use crate::model::*;
use crate::state::{lk, Shared};
use crate::{runner, scanner, watcher};
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use url::Url;

const MAX_REGISTRY_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PACKAGE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_FILES: usize = 512;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityCatalogItem {
    pub package: RegistryPackage,
    pub source_url: String,
    pub installed_version: Option<String>,
    pub installed_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunitySourceError {
    pub source_url: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityCatalog {
    pub items: Vec<CommunityCatalogItem>,
    pub errors: Vec<CommunitySourceError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageManifest {
    schema_version: u32,
    id: String,
    version: String,
    runtime: PackageRuntime,
    entry: String,
}

fn emit(app: &AppHandle) {
    let _ = app.emit("mosaic:changed", ());
}

fn safe_slug(value: &str, label: &str) -> Result<(), String> {
    if value.len() < 2 || value.len() > 64 {
        return Err(format!("{}长度必须在 2 到 64 个字符之间", label));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(format!(
            "{}只能包含英文字母、数字、点、下划线和连字符",
            label
        ));
    }
    Ok(())
}

fn safe_relative(value: &str, label: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("{}必须是包内的安全相对路径", label));
    }
    Ok(path.to_path_buf())
}

fn parse_source_url(value: &str) -> Result<Url, String> {
    let parsed = Url::parse(value.trim()).map_err(|_| "社区源 URL 无效".to_string())?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("社区源 URL 不能包含用户名或密码".into());
    }
    let loopback = parsed
        .host_str()
        .map(|host| host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1")
        .unwrap_or(false);
    if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
        return Err("社区源必须使用 HTTPS（本机测试可使用 localhost）".into());
    }
    Ok(parsed)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn agent() -> ureq::Agent {
    crate::network::download_agent_builder()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .redirects(0)
        .build()
}

fn fetch_limited(url: &Url, max_bytes: u64) -> Result<Vec<u8>, String> {
    let response = agent()
        .get(url.as_str())
        .set("User-Agent", concat!("Mosaic/", env!("CARGO_PKG_VERSION")))
        .call()
        .map_err(|error| format!("下载失败: {}", error))?;
    if let Some(length) = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
    {
        if length > max_bytes {
            return Err(format!("下载内容超过 {} MB 限制", max_bytes / 1024 / 1024));
        }
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取下载内容失败: {}", error))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("下载内容超过 {} MB 限制", max_bytes / 1024 / 1024));
    }
    Ok(bytes)
}

fn validate_package(package: &mut RegistryPackage, source: &Url) -> Result<(), String> {
    safe_slug(&package.id, "包 ID")?;
    safe_slug(&package.version, "版本")?;
    if package.name.trim().is_empty() || package.name.len() > 80 {
        return Err("包名称不能为空且不能超过 80 个字符".into());
    }
    if package.author.trim().is_empty() || package.author.len() > 80 {
        return Err("作者不能为空且不能超过 80 个字符".into());
    }
    safe_relative(&package.entry, "入口文件")?;
    if package.args.len() > 32 || package.args.iter().any(|arg| arg.len() > 1024) {
        return Err("启动参数数量或长度超过限制".into());
    }
    let hash = package.sha256.trim().to_ascii_lowercase();
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("包 SHA-256 必须是 64 位十六进制字符串".into());
    }
    package.sha256 = hash;
    let package_url = source
        .join(package.package_url.trim())
        .map_err(|_| "包下载 URL 无效".to_string())?;
    if !same_origin(source, &package_url) {
        return Err("包文件必须与社区注册表同源，禁止跨域下载".into());
    }
    package.package_url = package_url.to_string();
    Ok(())
}

fn fetch_registry(source_value: &str) -> Result<(Url, RegistryDocument), String> {
    let source = parse_source_url(source_value)?;
    let bytes = fetch_limited(&source, MAX_REGISTRY_BYTES)?;
    let mut document: RegistryDocument =
        serde_json::from_slice(&bytes).map_err(|error| format!("注册表 JSON 无效: {}", error))?;
    if document.schema_version != 1 {
        return Err(format!("不支持的注册表版本 {}", document.schema_version));
    }
    if document.packages.len() > 500 {
        return Err("单个社区源最多登记 500 个包".into());
    }
    let mut seen = HashSet::new();
    for package in &mut document.packages {
        validate_package(package, &source)?;
        if !seen.insert(package.id.clone()) {
            return Err(format!(
                "包 ID {} 重复登记；一个源只保留当前版本",
                package.id
            ));
        }
    }
    Ok((source, document))
}

fn load_catalog(state: &Shared) -> CommunityCatalog {
    let (sources, installed) = {
        let db = lk(&state.db);
        let installed = db
            .tasks
            .iter()
            .filter_map(|task| {
                task.community
                    .as_ref()
                    .map(|source| (source.clone(), task.id.clone()))
            })
            .collect::<Vec<_>>();
        (db.community.sources.clone(), installed)
    };
    let mut catalog = CommunityCatalog::default();
    for source_url in sources {
        match fetch_registry(&source_url) {
            Ok((source, document)) => {
                for package in document.packages {
                    let current = installed
                        .iter()
                        .find(|(item, _)| item.package_id == package.id)
                        .cloned();
                    catalog.items.push(CommunityCatalogItem {
                        package,
                        source_url: source.to_string(),
                        installed_version: current.as_ref().map(|(item, _)| item.version.clone()),
                        installed_task_id: current.map(|(_, task_id)| task_id),
                    });
                }
            }
            Err(message) => catalog.errors.push(CommunitySourceError {
                source_url,
                message,
            }),
        }
    }
    catalog.items.sort_by(|left, right| {
        left.package
            .name
            .to_lowercase()
            .cmp(&right.package.name.to_lowercase())
    });
    catalog
}

#[tauri::command]
pub fn save_community_sources(
    state: State<'_, Shared>,
    app: AppHandle,
    sources: Vec<String>,
) -> Result<(), String> {
    if sources.len() > 8 {
        return Err("最多添加 8 个社区源".into());
    }
    let mut normalized = Vec::new();
    for source in sources {
        let value = parse_source_url(&source)?.to_string();
        if !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    {
        let mut db = lk(&state.db);
        db.community.sources = normalized;
    }
    state.save();
    state.flush();
    emit(&app);
    Ok(())
}

#[tauri::command]
pub async fn community_catalog(state: State<'_, Shared>) -> Result<CommunityCatalog, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || load_catalog(&shared))
        .await
        .map_err(|error| format!("读取社区源失败: {}", error))
}

fn package_root(state: &Shared) -> Result<PathBuf, String> {
    let data_dir = state
        .path
        .parent()
        .ok_or_else(|| "找不到 Mosaic 数据目录".to_string())?;
    let root = data_dir.join("community-packages");
    std::fs::create_dir_all(&root).map_err(|error| format!("创建社区包目录失败: {}", error))?;
    Ok(root)
}

fn remove_inside(root: &Path, candidate: &Path) -> Result<(), String> {
    if !candidate.exists() {
        return Ok(());
    }
    let root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    let candidate = std::fs::canonicalize(candidate).map_err(|error| error.to_string())?;
    if candidate == root || !candidate.starts_with(&root) {
        return Err("拒绝删除社区包目录之外的路径".into());
    }
    std::fs::remove_dir_all(candidate).map_err(|error| format!("删除旧包失败: {}", error))
}

fn unpack_package(bytes: Vec<u8>, destination: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|error| format!("创建临时目录失败: {}", error))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("包不是有效的 ZIP: {}", error))?;
    if archive.len() > MAX_FILES {
        return Err(format!("包内文件超过 {} 个限制", MAX_FILES));
    }
    let mut unpacked = 0u64;
    let mut paths = HashSet::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| format!("读取 ZIP 条目失败: {}", error))?;
        if file
            .unix_mode()
            .map(|mode| mode & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            return Err("社区包不能包含符号链接".into());
        }
        unpacked = unpacked.saturating_add(file.size());
        if unpacked > MAX_UNPACKED_BYTES {
            return Err("解包后内容超过 128 MB 限制".into());
        }
        let relative = file
            .enclosed_name()
            .ok_or_else(|| "社区包包含越界路径".to_string())?
            .to_path_buf();
        let path_key = relative
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if !paths.insert(path_key) {
            return Err("社区包包含重复路径".into());
        }
        let output = destination.join(&relative);
        if file.is_dir() {
            std::fs::create_dir_all(&output).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut writer = std::fs::File::create(&output).map_err(|error| error.to_string())?;
        std::io::copy(&mut file, &mut writer).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn terminate_and_wait(state: &Shared, task_id: &str) {
    if !runner::terminate(state, task_id) {
        return;
    }
    for _ in 0..25 {
        if !lk(&state.running).contains_key(task_id) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn install_package(
    state: Shared,
    app: AppHandle,
    source_value: String,
    package_id: String,
    version: String,
) -> Result<Task, String> {
    let configured = {
        let db = lk(&state.db);
        db.community
            .sources
            .iter()
            .any(|value| value == &source_value)
    };
    if !configured {
        return Err("该社区源尚未加入 Mosaic".into());
    }
    let (source_url, document) = fetch_registry(&source_value)?;
    let package = document
        .packages
        .into_iter()
        .find(|package| package.id == package_id && package.version == version)
        .ok_or_else(|| "社区源中找不到这个版本".to_string())?;
    let package_url =
        Url::parse(&package.package_url).map_err(|_| "包下载 URL 无效".to_string())?;
    if !same_origin(&source_url, &package_url) {
        return Err("包文件与社区注册表不同源".into());
    }
    let bytes = fetch_limited(&package_url, MAX_PACKAGE_BYTES)?;
    let actual_hash = format!("{:x}", Sha256::digest(&bytes));
    if actual_hash != package.sha256 {
        return Err(format!(
            "SHA-256 校验失败：登记值 {}，实际值 {}",
            package.sha256, actual_hash
        ));
    }

    let root = package_root(&state)?;
    let target = root.join(&package.id).join(&package.version);
    let temporary = root.join(format!(".install-{}", uuid::Uuid::new_v4()));
    if !target.exists() {
        if let Err(error) = unpack_package(bytes, &temporary) {
            let _ = remove_inside(&root, &temporary);
            return Err(error);
        }
        let validate = || -> Result<(), String> {
            let manifest_path = temporary.join("mosaic-package.json");
            let manifest: PackageManifest = std::fs::read_to_string(&manifest_path)
                .map_err(|_| "包内缺少 mosaic-package.json".to_string())
                .and_then(|value| {
                    serde_json::from_str(&value).map_err(|error| format!("包清单无效: {}", error))
                })?;
            if manifest.schema_version != 1
                || manifest.id != package.id
                || manifest.version != package.version
                || manifest.runtime != package.runtime
                || manifest.entry != package.entry
            {
                return Err("包内清单与社区登记信息不一致".into());
            }
            let entry = temporary.join(safe_relative(&package.entry, "入口文件")?);
            if !entry.is_file() {
                return Err("登记的入口文件不存在".into());
            }
            Ok(())
        };
        if let Err(error) = validate() {
            let _ = remove_inside(&root, &temporary);
            return Err(error);
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        if let Err(error) = std::fs::rename(&temporary, &target) {
            let _ = remove_inside(&root, &temporary);
            return Err(format!("安装包失败: {}", error));
        }
    }

    let entry = target.join(safe_relative(&package.entry, "入口文件")?);
    let (command, mut args) = match package.runtime {
        PackageRuntime::Node => (
            "node".to_string(),
            vec![entry.to_string_lossy().into_owned()],
        ),
        PackageRuntime::Python => (
            "python".to_string(),
            vec![entry.to_string_lossy().into_owned()],
        ),
        PackageRuntime::PowerShell => (
            "powershell.exe".to_string(),
            vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-File".into(),
                entry.to_string_lossy().into_owned(),
            ],
        ),
        PackageRuntime::Executable => (entry.to_string_lossy().into_owned(), vec![]),
    };
    args.extend(package.args.clone());
    let risk = scanner::scan_task(&command, &args, Some(target.to_string_lossy().as_ref())).risk;

    let previous = {
        let db = lk(&state.db);
        db.tasks
            .iter()
            .find(|task| {
                task.community
                    .as_ref()
                    .map(|source| source.package_id == package.id)
                    .unwrap_or(false)
            })
            .cloned()
    };
    if let Some(old) = &previous {
        terminate_and_wait(&state, &old.id);
        lk(&state.last_runs).remove(&old.id);
    }
    let task_id = previous
        .as_ref()
        .map(|task| task.id.clone())
        .unwrap_or_else(|| format!("community:{}", package.id));
    let task = Task {
        id: task_id.clone(),
        nickname: package.name.clone(),
        command,
        kind: package.kind,
        args,
        cwd: Some(target.to_string_lossy().into_owned()),
        trigger: Trigger::Manual,
        display: DisplayForm::Card,
        lifecycle: if package.kind == TaskKind::Plugin {
            Lifecycle::Resident
        } else {
            package.lifecycle
        },
        active: false,
        enabled: false,
        timeout_secs: if package.kind == TaskKind::Plugin {
            0
        } else {
            60
        },
        push_channel: None,
        on_dashboard: false,
        order: 0,
        col_span: 1,
        row_span: 1,
        output_dir: None,
        stdin: None,
        community: Some(CommunityTaskSource {
            package_id: package.id.clone(),
            version: package.version.clone(),
            registry_url: source_url.to_string(),
            package_dir: target.to_string_lossy().into_owned(),
            sha256: package.sha256.clone(),
            author: package.author.clone(),
            runtime: package.runtime,
            risk,
        }),
        created_at: Local::now().to_rfc3339(),
    };
    {
        let mut db = lk(&state.db);
        db.tasks.retain(|item| item.id != task_id);
        db.tasks.push(task.clone());
    }
    state.save();
    state.flush();
    watcher::restart_all(&state, &app);
    if let Some(old_dir) = previous
        .and_then(|task| task.community)
        .map(|source| PathBuf::from(source.package_dir))
    {
        if old_dir != target {
            let _ = remove_inside(&root, &old_dir);
        }
    }
    runner::push_notification(
        &state,
        "info",
        &format!("{} 已安装", package.name),
        Some("第三方代码保持未启用；请阅读权限说明后再启用。"),
        Some(&task.id),
    );
    state.save();
    state.flush();
    emit(&app);
    Ok(task)
}

#[tauri::command]
pub async fn install_community_package(
    state: State<'_, Shared>,
    app: AppHandle,
    source_url: String,
    package_id: String,
    version: String,
) -> Result<Task, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        install_package(shared, app, source_url, package_id, version)
    })
    .await
    .map_err(|error| format!("安装任务异常: {}", error))?
}

fn uninstall_package(state: Shared, app: AppHandle, package_id: String) -> Result<(), String> {
    let task = {
        let db = lk(&state.db);
        db.tasks
            .iter()
            .find(|task| {
                task.community
                    .as_ref()
                    .map(|source| source.package_id == package_id)
                    .unwrap_or(false)
            })
            .cloned()
    }
    .ok_or_else(|| "这个社区包尚未安装".to_string())?;
    let source = task
        .community
        .clone()
        .ok_or_else(|| "社区包来源信息缺失".to_string())?;
    let root = package_root(&state)?;
    terminate_and_wait(&state, &task.id);
    remove_inside(&root, Path::new(&source.package_dir))?;
    lk(&state.last_runs).remove(&task.id);
    {
        let mut db = lk(&state.db);
        db.tasks.retain(|item| item.id != task.id);
    }
    state.save();
    state.flush();
    watcher::restart_all(&state, &app);
    emit(&app);
    Ok(())
}

#[tauri::command]
pub async fn uninstall_community_package(
    state: State<'_, Shared>,
    app: AppHandle,
    package_id: String,
) -> Result<(), String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || uninstall_package(shared, app, package_id))
        .await
        .map_err(|error| format!("卸载任务异常: {}", error))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_path_traversal() {
        assert!(safe_relative("../evil.exe", "入口").is_err());
        assert!(safe_relative("/absolute.exe", "入口").is_err());
        assert!(safe_relative("scripts/main.js", "入口").is_ok());
    }

    #[test]
    fn accepts_https_and_loopback_only() {
        assert!(parse_source_url("https://example.com/registry.json").is_ok());
        assert!(parse_source_url("http://127.0.0.1:8317/registry.json").is_ok());
        assert!(parse_source_url("http://example.com/registry.json").is_err());
        assert!(parse_source_url("file:///tmp/registry.json").is_err());
    }

    #[test]
    fn requires_same_origin_packages() {
        let source = Url::parse("https://plugins.example.com/registry.json").unwrap();
        let package = Url::parse("https://plugins.example.com/files/a.zip").unwrap();
        let foreign = Url::parse("https://cdn.example.com/a.zip").unwrap();
        assert!(same_origin(&source, &package));
        assert!(!same_origin(&source, &foreign));
    }

    #[test]
    fn rejects_zip_path_traversal() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut archive = zip::ZipWriter::new(&mut bytes);
            archive
                .start_file("../outside.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(b"nope").unwrap();
            archive.finish().unwrap();
        }
        let destination =
            std::env::temp_dir().join(format!("mosaic-community-test-{}", uuid::Uuid::new_v4()));
        assert!(unpack_package(bytes.into_inner(), &destination).is_err());
        let _ = std::fs::remove_dir_all(destination);
    }
}
