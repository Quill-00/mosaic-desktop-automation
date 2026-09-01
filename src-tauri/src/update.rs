use crate::model::Notification;
use crate::state::{lk, Shared};
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use url::Url;

const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/Quill-00/mosaic-desktop-automation/releases/latest";
const MAX_INSTALLER_BYTES: u64 = 256 * 1024 * 1024;
const INSTALLER_ARGS: &[&str] = &[
    "/VERYSILENT",
    "/SUPPRESSMSGBOXES",
    "/NOCANCEL",
    "/NORESTART",
    "/CLOSEAPPLICATIONS",
    "/RESTARTAPPLICATIONS",
    "/AUTOUPDATE",
];

static CHECKING: AtomicBool = AtomicBool::new(false);
static STATUS: OnceLock<Mutex<UpdateStatus>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current_version: String,
    pub state: String,
    pub latest_version: Option<String>,
    pub message: String,
    pub checked_at: Option<String>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            current_version: env!("CARGO_PKG_VERSION").into(),
            state: "idle".into(),
            latest_version: None,
            message: crate::locale::text(
                "Mosaic checks automatically after launch; installers are distributed by GitHub Releases.",
                "启动后自动检查；安装包由 GitHub Releases 分发。",
            ),
            checked_at: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug)]
struct UpdateOffer {
    version: String,
    download_url: String,
    sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingUpdate {
    version: String,
    installer_path: String,
    sha256: String,
    ready_at: String,
}

pub fn status() -> UpdateStatus {
    STATUS
        .get_or_init(|| Mutex::new(UpdateStatus::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn set_status(next: UpdateStatus, app: &AppHandle) {
    *STATUS
        .get_or_init(|| Mutex::new(UpdateStatus::default()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = next;
    let _ = app.emit("mosaic:changed", ());
}

/// This gate runs before Tauri creates a window or starts any resident task.
/// A staged installer is launched only on a later process start.
pub fn try_launch_pending_at_startup() -> bool {
    #[cfg(not(windows))]
    {
        false
    }
    #[cfg(windows)]
    {
        let Some(pending) = read_pending() else {
            return false;
        };
        if compare_versions(env!("CARGO_PKG_VERSION"), &pending.version) >= 0 {
            let _ = fs::remove_file(pending_manifest_path());
            return false;
        }
        if sha256_file(Path::new(&pending.installer_path)).as_deref() != Some(&pending.sha256) {
            let _ = fs::remove_file(&pending.installer_path);
            let _ = fs::remove_file(pending_manifest_path());
            return false;
        }
        Command::new(&pending.installer_path)
            .args(INSTALLER_ARGS)
            .current_dir(
                Path::new(&pending.installer_path)
                    .parent()
                    .unwrap_or_else(|| Path::new(".")),
            )
            .spawn()
            .is_ok()
    }
}

pub fn schedule_check(shared: Shared, app: AppHandle) {
    if CHECKING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    set_status(
        UpdateStatus {
            state: "checking".into(),
            message: crate::locale::text(
                "Checking GitHub for a new installer…",
                "正在检查 GitHub 上的新版安装包…",
            ),
            ..status()
        },
        &app,
    );
    std::thread::spawn(move || {
        let result = check_and_stage();
        let checked_at = Local::now().to_rfc3339();
        match result {
            Ok(Some(offer)) => {
                upsert_notification(
                    &shared,
                    format!("update-ready-{}", offer.version),
                    "success",
                    if crate::locale::is_chinese() {
                        format!("Mosaic {} 已准备好", offer.version)
                    } else {
                        format!("Mosaic {} is ready", offer.version)
                    },
                    Some(crate::locale::text(
                        "The installer was downloaded from GitHub and passed SHA-256 verification. It will install automatically the next time Mosaic starts.",
                        "安装包已从 GitHub 下载并通过 SHA-256 校验，将在下次启动 Mosaic 时自动安装。",
                    )),
                );
                set_status(
                    UpdateStatus {
                        current_version: env!("CARGO_PKG_VERSION").into(),
                        state: "ready".into(),
                        latest_version: Some(offer.version),
                        message: crate::locale::text(
                            "The verified update is ready and will install on the next launch.",
                            "更新已安全下载，将在下次启动时安装。",
                        ),
                        checked_at: Some(checked_at),
                    },
                    &app,
                );
            }
            Ok(None) => set_status(
                UpdateStatus {
                    current_version: env!("CARGO_PKG_VERSION").into(),
                    state: "upToDate".into(),
                    latest_version: None,
                    message: crate::locale::text(
                        "You are using the latest version.",
                        "当前已是最新版本。",
                    ),
                    checked_at: Some(checked_at),
                },
                &app,
            ),
            Err(error) => {
                let github_failure = error.contains("GitHub");
                let title = if github_failure {
                    crate::locale::text(
                        "Unable to connect to GitHub for updates",
                        "无法连接到 GitHub 更新",
                    )
                } else {
                    crate::locale::text("Automatic update check failed", "自动更新检查失败")
                };
                let body = if github_failure {
                    crate::locale::text(
                        "Check your network and retry from Settings. You can keep using this version; incomplete downloads will not be installed.",
                        "请检查网络后在设置中重试。当前版本可以继续使用，不会安装未完成的下载。",
                    )
                } else {
                    crate::locale::text(
                        "Version information is temporarily unavailable. Retry from Settings later; you can keep using this version.",
                        "暂时无法获取版本信息，请稍后在设置中重试。当前版本可以继续使用。",
                    )
                };
                upsert_notification(
                    &shared,
                    "update-error".into(),
                    "warning",
                    title.clone(),
                    Some(body),
                );
                set_status(
                    UpdateStatus {
                        current_version: env!("CARGO_PKG_VERSION").into(),
                        state: "error".into(),
                        latest_version: None,
                        message: format!("{}: {}", title, error),
                        checked_at: Some(checked_at),
                    },
                    &app,
                );
            }
        }
        shared.save();
        CHECKING.store(false, Ordering::SeqCst);
        let _ = app.emit("mosaic:changed", ());
    });
}

#[tauri::command]
pub fn check_for_updates(state: tauri::State<'_, Shared>, app: AppHandle) -> UpdateStatus {
    schedule_check(state.inner().clone(), app);
    status()
}

fn check_and_stage() -> Result<Option<UpdateOffer>, String> {
    let offer = fetch_offer()?;
    let Some(offer) = offer else {
        return Ok(None);
    };
    download_and_stage(&offer)?;
    Ok(Some(offer))
}

fn fetch_offer() -> Result<Option<UpdateOffer>, String> {
    let agent = crate::network::download_agent_builder()
        .https_only(true)
        .redirects(0)
        .timeout(Duration::from_secs(12))
        .build();
    let response = agent
        .get(LATEST_RELEASE_API)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", concat!("Mosaic/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|error| format!("GitHub 版本检查失败 ({})", error))?;
    let release: GithubRelease = response
        .into_json()
        .map_err(|error| format!("GitHub Release 响应无效 ({})", error))?;
    let release_tag = release.tag_name.trim().to_string();
    let version = release_tag
        .trim()
        .trim_start_matches(['v', 'V'])
        .to_string();
    if version.is_empty() {
        return Err("GitHub Release 缺少版本号".into());
    }
    if compare_versions(env!("CARGO_PKG_VERSION"), &version) >= 0 {
        return Ok(None);
    }

    let installer_name = format!("Mosaic-Setup-{version}.exe");
    let checksum_name = format!("{installer_name}.sha256");
    let download_url = release
        .assets
        .iter()
        .find(|asset| asset.name == installer_name)
        .map(|asset| asset.browser_download_url.clone())
        .ok_or_else(|| format!("GitHub Release 缺少 {installer_name}"))?;
    let checksum_url = release
        .assets
        .iter()
        .find(|asset| asset.name == checksum_name)
        .map(|asset| asset.browser_download_url.clone())
        .ok_or_else(|| format!("GitHub Release 缺少 {checksum_name}"))?;
    validate_github_release_asset_url(&download_url, &release_tag, &installer_name)?;
    validate_github_release_asset_url(&checksum_url, &release_tag, &checksum_name)?;
    let sha256 = fetch_release_checksum(&checksum_url, &installer_name)?;
    Ok(Some(UpdateOffer {
        version,
        download_url,
        sha256,
    }))
}

fn fetch_release_checksum(url: &str, installer_name: &str) -> Result<String, String> {
    let response = crate::network::download_agent_builder()
        .https_only(true)
        .redirects(5)
        .timeout(Duration::from_secs(12))
        .build()
        .get(url)
        .set("User-Agent", concat!("Mosaic/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|error| format!("GitHub 校验文件下载失败 ({error})"))?;
    let final_url =
        Url::parse(response.get_url()).map_err(|_| "GitHub 校验文件重定向地址无效".to_string())?;
    if final_url.scheme() != "https" {
        return Err("GitHub 校验文件被重定向到非 HTTPS 地址".into());
    }
    let mut body = Vec::new();
    response
        .into_reader()
        .take(4097)
        .read_to_end(&mut body)
        .map_err(|error| format!("GitHub 校验文件读取失败 ({error})"))?;
    if body.len() > 4096 {
        return Err("GitHub 校验文件过大".into());
    }
    let text = String::from_utf8(body).map_err(|_| "GitHub 校验文件不是 UTF-8".to_string())?;
    parse_release_checksum(&text, installer_name)
}

fn parse_release_checksum(text: &str, installer_name: &str) -> Result<String, String> {
    let mut parts = text.split_whitespace();
    let checksum = parts.next().unwrap_or_default().to_ascii_lowercase();
    let file_name = parts.next().unwrap_or_default().trim_start_matches('*');
    if !is_sha256(&checksum) || file_name != installer_name || parts.next().is_some() {
        return Err("GitHub Release 的 SHA-256 文件格式无效".into());
    }
    Ok(checksum)
}

fn download_and_stage(offer: &UpdateOffer) -> Result<PathBuf, String> {
    let parsed = validate_github_release_url(&offer.download_url)?;
    let file_name = parsed
        .path_segments()
        .and_then(|segments| segments.last())
        .filter(|name| safe_file_component(name) && name.to_ascii_lowercase().ends_with(".exe"))
        .ok_or_else(|| "GitHub 更新文件名无效".to_string())?;
    let version_dir = update_root().join(safe_version_component(&offer.version));
    fs::create_dir_all(&version_dir).map_err(|error| format!("无法创建更新目录 ({})", error))?;
    let final_path = version_dir.join(file_name);
    let partial_path = final_path.with_extension("exe.partial");
    let _ = fs::remove_file(&partial_path);

    let agent = crate::network::download_agent_builder()
        .https_only(true)
        .redirects(5)
        .timeout_connect(Duration::from_secs(12))
        .timeout_read(Duration::from_secs(30))
        .build();
    let response = agent
        .get(parsed.as_str())
        .set("User-Agent", concat!("Mosaic/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|error| format!("GitHub 下载失败 ({})", error))?;
    let final_url =
        Url::parse(response.get_url()).map_err(|_| "GitHub 重定向地址无效".to_string())?;
    if final_url.scheme() != "https" {
        return Err("GitHub 更新被重定向到非 HTTPS 地址".into());
    }
    let content_length = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());
    if content_length.is_some_and(|size| size > MAX_INSTALLER_BYTES) {
        return Err("GitHub 安装包超过 256 MB 安全上限".into());
    }

    let mut input = response.into_reader();
    let result = (|| -> Result<(), String> {
        let mut output = File::create(&partial_path)
            .map_err(|error| format!("无法创建更新临时文件 ({})", error))?;
        let mut hash = Sha256::new();
        let mut received = 0_u64;
        let mut buffer = [0_u8; 128 * 1024];
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|error| format!("GitHub 下载中断 ({})", error))?;
            if read == 0 {
                break;
            }
            received += read as u64;
            if received > MAX_INSTALLER_BYTES {
                return Err("GitHub 安装包超过 256 MB 安全上限".into());
            }
            output
                .write_all(&buffer[..read])
                .map_err(|error| format!("写入更新临时文件失败 ({})", error))?;
            hash.update(&buffer[..read]);
        }
        output
            .sync_all()
            .map_err(|error| format!("保存更新临时文件失败 ({})", error))?;
        if content_length.is_some_and(|expected| expected != received) {
            return Err("GitHub 安装包下载不完整".into());
        }
        let actual = format!("{:x}", hash.finalize());
        if actual != offer.sha256 {
            return Err("GitHub 安装包 SHA-256 校验失败".into());
        }
        if !is_windows_executable(&partial_path) {
            return Err("GitHub 下载内容不是有效的 Windows 安装程序".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&partial_path);
        return Err(error);
    }
    let _ = fs::remove_file(&final_path);
    fs::rename(&partial_path, &final_path)
        .map_err(|error| format!("无法保存已校验安装包 ({})", error))?;
    write_pending(PendingUpdate {
        version: offer.version.clone(),
        installer_path: final_path.to_string_lossy().into_owned(),
        sha256: offer.sha256.clone(),
        ready_at: Local::now().to_rfc3339(),
    })?;
    Ok(final_path)
}

fn upsert_notification(
    shared: &Shared,
    id: String,
    level: &str,
    title: String,
    body: Option<String>,
) {
    let mut db = lk(&shared.db);
    let next = Notification {
        id: id.clone(),
        level: level.into(),
        title,
        body,
        at: Local::now().to_rfc3339(),
        read: false,
        task_id: None,
    };
    if let Some(existing) = db.notifications.iter_mut().find(|item| item.id == id) {
        *existing = next;
    } else {
        db.notifications.insert(0, next);
    }
}

fn update_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Mosaic")
        .join("Updates")
}

fn pending_manifest_path() -> PathBuf {
    update_root().join("pending-update.json")
}

fn write_pending(pending: PendingUpdate) -> Result<(), String> {
    let root = update_root();
    fs::create_dir_all(&root).map_err(|error| format!("无法创建更新状态目录 ({})", error))?;
    let temporary = root.join("pending-update.json.tmp");
    let body = serde_json::to_vec_pretty(&pending).map_err(|error| error.to_string())?;
    fs::write(&temporary, body).map_err(|error| format!("无法写入更新状态 ({})", error))?;
    let _ = fs::remove_file(pending_manifest_path());
    fs::rename(&temporary, pending_manifest_path())
        .map_err(|error| format!("无法提交更新状态 ({})", error))
}

fn read_pending() -> Option<PendingUpdate> {
    let path = pending_manifest_path();
    let bytes = fs::read(&path).ok()?;
    let pending: PendingUpdate = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => {
            let _ = fs::remove_file(path);
            return None;
        }
    };
    let root = fs::canonicalize(update_root()).ok()?;
    let installer = fs::canonicalize(&pending.installer_path).ok()?;
    if !installer.starts_with(&root)
        || !is_windows_executable(&installer)
        || !is_sha256(&pending.sha256)
    {
        let _ = fs::remove_file(path);
        return None;
    }
    Some(PendingUpdate {
        installer_path: installer.to_string_lossy().into_owned(),
        ..pending
    })
}

fn validate_github_release_url(value: &str) -> Result<Url, String> {
    let parsed = Url::parse(value).map_err(|_| "GitHub 下载地址格式无效".to_string())?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("github.com")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("更新只接受无凭据的 GitHub Releases HTTPS 地址".into());
    }
    let prefix = "/Quill-00/mosaic-desktop-automation/releases/download/";
    if !parsed.path().starts_with(prefix) || !parsed.path().to_ascii_lowercase().ends_with(".exe") {
        return Err("更新地址不属于 Mosaic 的 GitHub Release".into());
    }
    Ok(parsed)
}

fn validate_github_release_asset_url(
    value: &str,
    tag: &str,
    file_name: &str,
) -> Result<Url, String> {
    let parsed = validate_github_release_url(value)?;
    if !safe_file_component(tag) || !safe_file_component(file_name) {
        return Err("GitHub Release 标签或文件名无效".into());
    }
    let expected =
        format!("/Quill-00/mosaic-desktop-automation/releases/download/{tag}/{file_name}");
    if parsed.path() != expected {
        return Err("GitHub Release 资产与版本不匹配".into());
    }
    Ok(parsed)
}

fn safe_file_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && !value.contains(['/', '\\', ':'])
        && value != "."
        && value != ".."
}

fn safe_version_component(value: &str) -> String {
    let safe: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        .take(64)
        .collect();
    if safe.is_empty() {
        "unknown".into()
    } else {
        safe
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_file(path: &Path) -> Option<String> {
    let mut input = File::open(path).ok()?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = input.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Some(format!("{:x}", hash.finalize()))
}

fn is_windows_executable(path: &Path) -> bool {
    let Ok(mut input) = File::open(path) else {
        return false;
    };
    let Ok(length) = input.metadata().map(|value| value.len()) else {
        return false;
    };
    if length < 68 {
        return false;
    }
    let mut header = [0_u8; 64];
    if input.read_exact(&mut header).is_err() || &header[..2] != b"MZ" {
        return false;
    }
    let offset = u32::from_le_bytes(header[0x3c..0x40].try_into().unwrap()) as u64;
    if offset < 64 || offset > length.saturating_sub(4) {
        return false;
    }
    use std::io::Seek;
    if input.seek(std::io::SeekFrom::Start(offset)).is_err() {
        return false;
    }
    let mut signature = [0_u8; 4];
    input.read_exact(&mut signature).is_ok() && signature == *b"PE\0\0"
}

fn compare_versions(left: &str, right: &str) -> i8 {
    fn parts(value: &str) -> Vec<u64> {
        value
            .trim()
            .trim_start_matches(['v', 'V'])
            .split_once('-')
            .map(|(stable, _)| stable)
            .unwrap_or(value.trim().trim_start_matches(['v', 'V']))
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    }
    let left = parts(left);
    let right = parts(right);
    for index in 0..left.len().max(right.len()) {
        match left
            .get(index)
            .copied()
            .unwrap_or(0)
            .cmp(&right.get(index).copied().unwrap_or(0))
        {
            std::cmp::Ordering::Less => return -1,
            std::cmp::Ordering::Greater => return 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_is_loose_and_never_downgrades() {
        assert_eq!(compare_versions("1.0", "1.0.0"), 0);
        assert_eq!(compare_versions("0.3.0", "0.3.1"), -1);
        assert_eq!(compare_versions("2.0.0", "1.9.9"), 1);
    }

    #[test]
    fn only_the_official_github_release_path_is_accepted() {
        assert!(validate_github_release_url(
            "https://github.com/Quill-00/mosaic-desktop-automation/releases/download/v0.3.0/Mosaic-Setup-0.3.0.exe"
        )
        .is_ok());
        assert!(validate_github_release_url(
            "http://github.com/Quill-00/mosaic-desktop-automation/releases/download/v1/a.exe"
        )
        .is_err());
        assert!(validate_github_release_url("https://example.com/Mosaic.exe").is_err());
        assert!(validate_github_release_url(
            "https://github.com/other/repo/releases/download/v1/a.exe"
        )
        .is_err());
    }

    #[test]
    fn checksum_must_be_complete_hex() {
        assert!(is_sha256(&"a".repeat(64)));
        assert!(!is_sha256(&"g".repeat(64)));
        assert!(!is_sha256(&"a".repeat(63)));
    }

    #[test]
    fn release_checksum_requires_the_exact_installer_name() {
        let hash = "a".repeat(64);
        assert_eq!(
            parse_release_checksum(
                &format!("{hash}  Mosaic-Setup-0.3.1.exe\n"),
                "Mosaic-Setup-0.3.1.exe"
            )
            .unwrap(),
            hash
        );
        assert!(parse_release_checksum(
            &format!("{}  another.exe\n", "a".repeat(64)),
            "Mosaic-Setup-0.3.1.exe"
        )
        .is_err());
    }

    #[test]
    fn release_assets_must_match_the_exact_tag_and_name() {
        assert!(validate_github_release_asset_url(
            "https://github.com/Quill-00/mosaic-desktop-automation/releases/download/v0.3.1/Mosaic-Setup-0.3.1.exe",
            "v0.3.1",
            "Mosaic-Setup-0.3.1.exe"
        )
        .is_ok());
        assert!(validate_github_release_asset_url(
            "https://github.com/Quill-00/mosaic-desktop-automation/releases/download/v0.3.0/Mosaic-Setup-0.3.1.exe",
            "v0.3.1",
            "Mosaic-Setup-0.3.1.exe"
        )
        .is_err());
    }
}
