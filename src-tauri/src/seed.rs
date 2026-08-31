use crate::model::*;
use chrono::Local;

// Real, runnable example tasks (live data, no extra deps — Node's global fetch).
// Installed once via migrations; not fabricated mock output.

const WEATHER_SCRIPT: &str = r#"(async()=>{const r=await (await fetch('https://api.open-meteo.com/v1/forecast?latitude=39.9&longitude=116.4&current=temperature_2m,wind_speed_10m,relative_humidity_2m')).json();const c=r.current||{};const m=[{label:'温度',value:(c.temperature_2m??'-')+'°C'},{label:'湿度',value:(c.relative_humidity_2m??'-')+'%'},{label:'风速',value:(c.wind_speed_10m??'-')+' km/h'}];console.log(JSON.stringify({summary:{headline:'北京天气'},card:{type:'metric',title:'天气 · 北京',metrics:m},items:[{title:'天气更新',at:new Date().toISOString()}]}));})().catch(e=>{console.error(e);process.exit(1);});"#;

const WEATHER_SCRIPT_EN: &str = r#"(async()=>{const r=await (await fetch('https://api.open-meteo.com/v1/forecast?latitude=40.7128&longitude=-74.0060&current=temperature_2m,wind_speed_10m,relative_humidity_2m')).json();const c=r.current||{};const m=[{label:'Temperature',value:(c.temperature_2m??'-')+'°C'},{label:'Humidity',value:(c.relative_humidity_2m??'-')+'%'},{label:'Wind',value:(c.wind_speed_10m??'-')+' km/h'}];console.log(JSON.stringify({summary:{headline:'New York weather'},card:{type:'metric',title:'Weather · New York',metrics:m},items:[{title:'Weather updated',at:new Date().toISOString()}]}));})().catch(e=>{console.error(e);process.exit(1);});"#;

fn use_chinese_defaults() -> bool {
    Local::now().offset().local_minus_utc() == 8 * 60 * 60
}

pub fn example_tasks_v1() -> Vec<Task> {
    Vec::new()
}

pub fn example_tasks_v2() -> Vec<Task> {
    let chinese = use_chinese_defaults();
    vec![node_task(
            "example-weather",
            if chinese { "北京天气" } else { "New York Weather" },
            if chinese { WEATHER_SCRIPT } else { WEATHER_SCRIPT_EN },
            Trigger::Interval { every_secs: 3600 },
            DisplayForm::Metric,
        )]
}

fn location(path: std::path::PathBuf) -> Option<(String, Option<String>)> {
    if !path.is_file() {
        return None;
    }
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    let mut command = path.to_string_lossy().into_owned();
    // `canonicalize` returns a verbatim local path on Windows. A normal DOS
    // path works with both CreateProcess and tools that inspect the saved task.
    #[cfg(windows)]
    if let Some(without_prefix) = command.strip_prefix(r"\\?\") {
        command = without_prefix.to_string();
    }
    let cwd = path.parent().map(|parent| {
        let mut value = parent.to_string_lossy().into_owned();
        #[cfg(windows)]
        if let Some(without_prefix) = value.strip_prefix(r"\\?\") {
            value = without_prefix.to_string();
        }
        value
    });
    Some((command, cwd))
}

#[cfg(windows)]
fn numeric_version(value: &str) -> Option<Vec<u64>> {
    value
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(windows)]
fn scoop_cli_proxy_location(profile: &str) -> Option<(String, Option<String>)> {
    let root = std::path::Path::new(profile)
        .join("scoop")
        .join("apps")
        .join("cliproxyapi");

    // Do not launch through Scoop's `current` junction. Installed applications
    // can run with Windows Redirection Guard, which rejects CreateProcess when
    // the executable path traverses that reparse point (OS error 448).
    let mut versions = std::fs::read_dir(&root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.eq_ignore_ascii_case("current") {
                return None;
            }
            let version = numeric_version(&name)?;
            let executable = entry.path().join("cli-proxy-api.exe");
            executable.is_file().then_some((version, executable))
        })
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| left.0.cmp(&right.0));
    if let Some((_, executable)) = versions.pop() {
        return location(executable);
    }

    // Non-Scoop/manual layouts can still use the junction when the OS permits
    // it; canonicalization turns it into the physical target before saving.
    location(root.join("current").join("cli-proxy-api.exe"))
}

fn cli_proxy_location() -> (String, Option<String>) {
    if let Ok(value) = std::env::var("CLIPROXYAPI_PATH") {
        let path = std::path::PathBuf::from(value.trim());
        if let Some(found) = location(path) {
            return found;
        }
    }
    #[cfg(windows)]
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if let Some(found) = scoop_cli_proxy_location(&profile) {
            return found;
        }
    }
    (
        if cfg!(windows) {
            "cli-proxy-api.exe".into()
        } else {
            "cli-proxy-api".into()
        },
        None,
    )
}

/// Keep the built-in CPA task on a physical executable path. This runs on every
/// Mosaic start so existing databases are repaired and Scoop upgrades are
/// picked up without overwriting a deliberately customized command.
pub fn repair_builtin_plugin_locations(tasks: &mut [Task]) -> bool {
    let Some(task) = tasks
        .iter_mut()
        .find(|task| task.id == "plugin-cliproxyapi")
    else {
        return false;
    };
    let normalized = task.command.to_ascii_lowercase().replace('/', "\\");
    let uses_scoop_junction = normalized.contains("\\scoop\\apps\\cliproxyapi\\current\\");
    let uses_default_name = normalized == "cli-proxy-api.exe" || normalized == "cli-proxy-api";
    if !uses_scoop_junction && !uses_default_name {
        return false;
    }
    let (command, cwd) = cli_proxy_location();
    if task.command == command && task.cwd == cwd {
        return false;
    }
    task.command = command;
    task.cwd = cwd;
    true
}

pub fn builtin_plugins_v3() -> Vec<Task> {
    let (command, cwd) = cli_proxy_location();
    vec![Task {
        id: "plugin-cliproxyapi".into(),
        nickname: "CLIProxyAPI".into(),
        command,
        kind: TaskKind::Plugin,
        args: vec![],
        cwd,
        trigger: Trigger::Manual,
        display: DisplayForm::Card,
        lifecycle: Lifecycle::Resident,
        active: true,
        enabled: false,
        timeout_secs: 0,
        push_channel: None,
        on_dashboard: false,
        order: 0,
        col_span: 1,
        row_span: 1,
        output_dir: None,
        stdin: None,
        community: None,
        created_at: Local::now().to_rfc3339(),
    }]
}

/// Identifies the original fabricated demo tasks so the migration can remove them.
pub fn is_legacy_mock(t: &Task) -> bool {
    matches!(t.nickname.as_str(), "系统快照" | "示例新闻" | "每日提示")
        && t.command == "node"
        && t.args.first().map(|a| a == "-e").unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn compares_scoop_versions_numerically() {
        assert!(super::numeric_version("7.10.2") > super::numeric_version("7.2.145"));
        assert_eq!(super::numeric_version("current"), None);
    }
}

fn node_task(id: &str, name: &str, script: &str, trigger: Trigger, display: DisplayForm) -> Task {
    Task {
        id: id.into(),
        nickname: name.into(),
        command: "node".into(),
        kind: TaskKind::Script,
        args: vec!["-e".into(), script.into()],
        cwd: None,
        trigger,
        display,
        lifecycle: Lifecycle::Ephemeral,
        active: true,
        enabled: true,
        timeout_secs: 60,
        push_channel: None,
        on_dashboard: true,
        order: 0,
        col_span: 1,
        row_span: 1,
        output_dir: None,
        stdin: None,
        community: None,
        created_at: Local::now().to_rfc3339(),
    }
}
