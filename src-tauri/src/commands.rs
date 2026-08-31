use crate::channels::{self, BotChannelView, ChannelInfo};
use crate::model::*;
use crate::runner;
use crate::scanner::{self, CapabilityProfile};
use crate::state::{lk, RunningInfo, Shared};
use crate::{popo, vault, watcher};
use chrono::Local;
use serde::Serialize;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    tasks: Vec<Task>,
    results: HashMap<Id, TaskResultState>,
    executions: Vec<Execution>,
    notifications: Vec<Notification>,
    running: Vec<RunningInfo>,
    brief: Brief,
    popo: PopoConfig,
    bot_channels: Vec<BotChannelView>,
    window: WindowConfig,
    community: CommunityConfig,
    update: crate::update::UpdateStatus,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BriefSection {
    icon: String,
    title: String,
    detail: String,
    level: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Brief {
    headline: String,
    sections: Vec<BriefSection>,
}

fn emit(app: &AppHandle) {
    let _ = app.emit("mosaic:changed", ());
}

#[tauri::command]
pub fn set_locale(locale: String) -> Result<(), String> {
    crate::locale::set(&locale)
}

fn clear_last_run(state: &Shared, id: &str) {
    lk(&state.last_runs).remove(id);
}

fn compose_brief(db: &Db) -> Brief {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let runs_today = db
        .executions
        .iter()
        .filter(|e| e.started_at.get(0..10).map(|d| d == today).unwrap_or(false))
        .count();
    let fails = db
        .executions
        .iter()
        .filter(|e| matches!(e.status, ExecStatus::Failed | ExecStatus::TimedOut))
        .count();
    let enabled = db.tasks.iter().filter(|t| t.enabled).count();

    let mut sections = vec![BriefSection {
        icon: "list-checks".into(),
        title: "任务".into(),
        detail: format!("{} 个启用 · 今日已运行 {} 次", enabled, runs_today),
        level: "info".into(),
    }];

    // Stable order: walk tasks (not the HashMap) and surface the first few summaries.
    for task in db.tasks.iter().filter(|t| t.enabled) {
        if sections.len() >= 4 {
            break;
        }
        if let Some(r) = db.results.get(&task.id) {
            if let Some(s) = &r.summary {
                sections.push(BriefSection {
                    icon: "layout-grid".into(),
                    title: s.headline.clone(),
                    detail: format!(
                        "{} · {}",
                        task.nickname,
                        r.updated_at.clone().unwrap_or_default()
                    ),
                    level: "info".into(),
                });
            }
        }
    }

    if fails > 0 {
        sections.push(BriefSection {
            icon: "alert-triangle".into(),
            title: "需要注意".into(),
            detail: format!("有 {} 次运行失败", fails),
            level: "danger".into(),
        });
    }

    let headline = if fails > 0 {
        format!(
            "早安。今天有 {} 个任务在跑，有 {} 次失败需要你看一眼。",
            enabled, fails
        )
    } else {
        format!("早安。一切平稳，{} 个任务在为你运行。", enabled)
    };

    Brief { headline, sections }
}

#[tauri::command]
pub fn snapshot(state: State<'_, Shared>) -> Snapshot {
    let db = lk(&state.db);
    let running: Vec<RunningInfo> = lk(&state.running).values().map(|h| h.info()).collect();
    Snapshot {
        tasks: db.tasks.clone(),
        results: db.results.clone(),
        executions: db
            .executions
            .iter()
            .take(60)
            .map(|e| {
                // Strip per-run products from the snapshot — fetched on demand via
                // `exec_items` so the (polled) snapshot stays small.
                let mut e = e.clone();
                e.items = Vec::new();
                e
            })
            .collect(),
        notifications: db.notifications.clone(),
        running,
        brief: compose_brief(&db),
        popo: db.popo.clone(),
        bot_channels: channels::bot_views(state.inner(), &db.bot_channels),
        window: db.window.clone(),
        community: db.community.clone(),
        update: crate::update::status(),
    }
}

/// Products of a single run (fetched on demand when the user opens that round in
/// the detail view — kept out of the snapshot to keep it light).
#[tauri::command]
pub fn exec_items(state: State<'_, Shared>, exec_id: String) -> Vec<DetailItem> {
    let db = lk(&state.db);
    db.executions
        .iter()
        .find(|e| e.id == exec_id)
        .map(|e| e.items.clone())
        .unwrap_or_default()
}

/// Open a file or directory with the OS default handler (used for the "open
/// output folder" button and for opening individual file products).
#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_task(state: State<'_, Shared>, app: AppHandle, mut task: Task) -> Task {
    if task.id.is_empty() {
        task.id = uuid::Uuid::new_v4().to_string();
    }
    if task.created_at.is_empty() {
        task.created_at = Local::now().to_rfc3339();
    }
    if !task.active {
        task.enabled = false;
        task.on_dashboard = false;
    }
    // Editing a task: stop the old running instance and reset its schedule.
    runner::terminate(state.inner(), &task.id);
    clear_last_run(state.inner(), &task.id);
    {
        let mut db = lk(&state.db);
        db.tasks.retain(|t| t.id != task.id);
        db.tasks.push(task.clone());
    }
    state.save();
    state.flush();
    let sh = state.inner().clone();
    watcher::restart_all(&sh, &app);
    if task.active && task.enabled && task.lifecycle == Lifecycle::Resident {
        runner::run_task(
            state.inner().clone(),
            app.clone(),
            task.clone(),
            "保存后恢复".into(),
        );
    }
    emit(&app);
    task
}

#[tauri::command]
pub fn delete_task(
    state: State<'_, Shared>,
    app: AppHandle,
    id: Id,
    delete_products: bool,
) -> Result<(), String> {
    if lk(&state.db)
        .tasks
        .iter()
        .find(|task| task.id == id)
        .and_then(|task| task.community.as_ref())
        .is_some()
    {
        return Err("社区安装的项目请在“插件中心”卸载，以便同时清理包文件。".into());
    }
    runner::terminate(state.inner(), &id);
    clear_last_run(state.inner(), &id);
    {
        let mut db = lk(&state.db);
        db.tasks.retain(|t| t.id != id);
        // Keep the collected products (timeline/results) unless explicitly asked.
        if delete_products {
            db.results.remove(&id);
        }
    }
    state.save();
    state.flush();
    let sh = state.inner().clone();
    watcher::restart_all(&sh, &app);
    emit(&app);
    Ok(())
}

#[tauri::command]
pub fn set_active(state: State<'_, Shared>, app: AppHandle, id: Id, active: bool) {
    {
        let mut db = lk(&state.db);
        if let Some(task) = db.tasks.iter_mut().find(|task| task.id == id) {
            task.active = active;
            if !active {
                task.enabled = false;
                task.on_dashboard = false;
            }
        }
    }
    if !active {
        runner::terminate(state.inner(), &id);
        clear_last_run(state.inner(), &id);
    }
    state.save();
    state.flush();
    let shared = state.inner().clone();
    watcher::restart_all(&shared, &app);
    emit(&app);
}

#[tauri::command]
pub fn set_enabled(state: State<'_, Shared>, app: AppHandle, id: Id, enabled: bool) {
    let task = {
        let mut db = lk(&state.db);
        if let Some(t) = db.tasks.iter_mut().find(|t| t.id == id && t.active) {
            t.enabled = enabled;
            Some(t.clone())
        } else {
            None
        }
    };
    if !enabled {
        runner::terminate(state.inner(), &id);
    } else if let Some(task) = task.filter(|task| task.lifecycle == Lifecycle::Resident) {
        runner::run_task(state.inner().clone(), app.clone(), task, "开关开启".into());
    }
    state.save();
    state.flush();
    let sh = state.inner().clone();
    watcher::restart_all(&sh, &app);
    emit(&app);
}

#[tauri::command]
pub fn run_now(state: State<'_, Shared>, app: AppHandle, id: Id) {
    let task = {
        lk(&state.db)
            .tasks
            .iter()
            .find(|task| task.id == id && task.active && task.enabled)
            .cloned()
    };
    if let Some(t) = task {
        let sh = state.inner().clone();
        runner::run_task(sh, app.clone(), t, "手动".into());
    }
}

/// Set the dashboard layout in one call: tasks in `ordered_ids` are shown in that
/// order; every other task is taken off the dashboard.
#[tauri::command]
pub fn set_dashboard(state: State<'_, Shared>, app: AppHandle, ordered_ids: Vec<Id>) {
    {
        let mut db = lk(&state.db);
        for t in db.tasks.iter_mut() {
            match t
                .active
                .then(|| ordered_ids.iter().position(|id| id == &t.id))
                .flatten()
            {
                Some(pos) => {
                    t.on_dashboard = true;
                    t.order = pos as i32;
                }
                None => {
                    t.on_dashboard = false;
                }
            }
        }
    }
    state.save();
    state.flush();
    emit(&app);
}

/// Set a dashboard module's size in grid units (snapped; never free-pixel).
#[tauri::command]
pub fn set_module_span(state: State<'_, Shared>, app: AppHandle, id: Id, col: u8, row: u8) {
    {
        let mut db = lk(&state.db);
        if let Some(t) = db.tasks.iter_mut().find(|t| t.id == id) {
            t.col_span = col.clamp(1, 3);
            t.row_span = row.clamp(1, 2);
        }
    }
    state.save();
    state.flush();
    emit(&app);
}

#[tauri::command]
pub fn terminate(state: State<'_, Shared>, app: AppHandle, id: Id) {
    runner::terminate(state.inner(), &id);
    emit(&app);
}

#[tauri::command]
pub fn terminate_all(state: State<'_, Shared>, app: AppHandle) {
    runner::terminate_all(state.inner());
    emit(&app);
}

#[tauri::command]
pub fn mark_read(state: State<'_, Shared>, app: AppHandle, id: Id) {
    {
        let mut db = lk(&state.db);
        if let Some(n) = db.notifications.iter_mut().find(|n| n.id == id) {
            n.read = true;
        }
    }
    state.save();
    emit(&app);
}

#[tauri::command]
pub fn mark_all_read(state: State<'_, Shared>, app: AppHandle) {
    {
        let mut db = lk(&state.db);
        for n in db.notifications.iter_mut() {
            n.read = true;
        }
    }
    state.save();
    emit(&app);
}

#[tauri::command]
pub fn delete_notification(state: State<'_, Shared>, app: AppHandle, id: Id) {
    {
        let mut db = lk(&state.db);
        db.notifications
            .retain(|notification| notification.id != id);
    }
    state.save();
    state.flush();
    emit(&app);
}

#[tauri::command]
pub fn clear_notifications(state: State<'_, Shared>, app: AppHandle, read_only: bool) {
    {
        let mut db = lk(&state.db);
        if read_only {
            db.notifications.retain(|notification| !notification.read);
        } else {
            db.notifications.clear();
        }
    }
    state.save();
    state.flush();
    emit(&app);
}

/// Static-scan untrusted script text. Takes the source directly — it never reads
/// an arbitrary path from the frontend (that would be an arbitrary-file-read).
#[tauri::command]
pub fn scan_script(source: String) -> CapabilityProfile {
    scanner::scan(&source)
}

/// Inspect the actual local entry/project used by a task. If Mosaic cannot
/// obtain readable text source the result is explicitly `unknown`.
#[tauri::command]
pub fn scan_task_source(
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
) -> CapabilityProfile {
    scanner::scan_task(&command, &args, cwd.as_deref())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryGuess {
    kind: String,
    command: String,
    args: Vec<String>,
    cwd: String,
    nickname: String,
    note: String,
    output_dir: String,
}

fn detect_output_dir(dir: &std::path::Path) -> String {
    for cand in ["downloads", "output", "out", "results", "data"] {
        let p = dir.join(cand);
        if p.is_dir() {
            return p.display().to_string();
        }
    }
    String::new()
}

/// Find a PowerShell / batch / pyw entry script in a directory. With
/// `launcher_only`, returns one only if its name looks like a launcher
/// (fetch/run/main/start/...); otherwise also falls back to the first script.
fn find_script_entry(
    dir: &std::path::Path,
    launcher_only: bool,
) -> Option<(String, Vec<String>, String)> {
    let mut ps1: Vec<std::path::PathBuf> = vec![];
    let mut bat: Vec<std::path::PathBuf> = vec![];
    let mut pyw: Vec<std::path::PathBuf> = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let path = e.path();
            if !path.is_file() {
                continue;
            }
            match path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .as_deref()
            {
                Some("ps1") => ps1.push(path),
                Some("bat") | Some("cmd") => bat.push(path),
                Some("pyw") => pyw.push(path),
                _ => {}
            }
        }
    }
    fn pick(v: &[std::path::PathBuf], launcher_only: bool) -> Option<std::path::PathBuf> {
        const KW: [&str; 6] = ["fetch", "run", "main", "start", "collect", "采集"];
        for f in v {
            let stem = f
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if KW.iter().any(|k| stem.contains(k)) {
                return Some(f.clone());
            }
        }
        if launcher_only {
            None
        } else {
            v.first().cloned()
        }
    }
    if let Some(f) = pick(&ps1, launcher_only) {
        return Some((
            "powershell".into(),
            vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-File".into(),
                f.display().to_string(),
            ],
            "PowerShell 脚本".into(),
        ));
    }
    if let Some(f) = pick(&bat, launcher_only) {
        return Some((f.display().to_string(), vec![], "批处理脚本".into()));
    }
    if let Some(f) = pick(&pyw, launcher_only) {
        return Some((
            "pythonw".into(),
            vec![f.display().to_string()],
            "Python GUI".into(),
        ));
    }
    None
}

/// Inspect a local project directory or script file and guess how to run it, so
/// the user points at a path instead of hand-typing command + args. Read-only.
#[tauri::command]
pub fn inspect_target(path: String) -> Result<EntryGuess, String> {
    use std::path::Path;
    let p = Path::new(&path);
    if !p.exists() {
        return Err("路径不存在".into());
    }

    if p.is_dir() {
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("task")
            .to_string();
        let cwd = p.display().to_string();
        let output_dir = detect_output_dir(p);

        // A hand-written launcher (start.ps1 / run.bat / fetch.ps1) is the author's
        // intended entry — prefer it over language auto-detection.
        if let Some((command, args, note)) = find_script_entry(p, true) {
            return Ok(EntryGuess {
                kind: "script-project".into(),
                command,
                args,
                cwd,
                nickname: name,
                note,
                output_dir,
            });
        }

        if p.join("package.json").exists() {
            let pm = if p.join("pnpm-lock.yaml").exists() {
                "pnpm"
            } else if p.join("yarn.lock").exists() {
                "yarn"
            } else {
                "npm"
            };
            let has_start = std::fs::read_to_string(p.join("package.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .map(|v| v.pointer("/scripts/start").is_some())
                .unwrap_or(false);
            let args = vec![
                "run".into(),
                if has_start {
                    "start".into()
                } else {
                    "dev".into()
                },
            ];
            return Ok(EntryGuess {
                kind: "node-project".into(),
                command: pm.into(),
                args,
                cwd,
                nickname: name,
                note: format!("Node 项目（{}）", pm),
                output_dir,
            });
        }

        let py_entry = if p.join("main.py").exists() {
            Some("main.py")
        } else if p.join("app.py").exists() {
            Some("app.py")
        } else {
            None
        };
        if py_entry.is_some() || p.join("pyproject.toml").exists() {
            let py = if p.join(".venv/Scripts/python.exe").exists() {
                p.join(".venv/Scripts/python.exe").display().to_string()
            } else {
                "python".into()
            };
            let args = py_entry.map(|e| vec![e.to_string()]).unwrap_or_default();
            return Ok(EntryGuess {
                kind: "python-project".into(),
                command: py,
                args,
                cwd,
                nickname: name,
                note: "Python 项目".into(),
                output_dir,
            });
        }

        if p.join("Cargo.toml").exists() {
            return Ok(EntryGuess {
                kind: "rust-project".into(),
                command: "cargo".into(),
                args: vec!["run".into()],
                cwd,
                nickname: name,
                note: "Rust 项目".into(),
                output_dir,
            });
        }

        if let Some((command, args, note)) = find_script_entry(p, false) {
            return Ok(EntryGuess {
                kind: "script-project".into(),
                command,
                args,
                cwd,
                nickname: name,
                note,
                output_dir,
            });
        }

        return Err(
            "目录里没识别到入口（package.json / main.py / Cargo.toml / *.ps1 / *.bat 等）".into(),
        );
    }

    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let cwd = p
        .parent()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    let output_dir = p.parent().map(detect_output_dir).unwrap_or_default();
    let file = p.display().to_string();
    let name = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("task")
        .to_string();
    let (command, args): (String, Vec<String>) = match ext.as_str() {
        "py" => ("python".into(), vec![file.clone()]),
        "js" | "mjs" | "cjs" => ("node".into(), vec![file.clone()]),
        "ts" => ("npx".into(), vec!["tsx".into(), file.clone()]),
        "ps1" => (
            "powershell".into(),
            vec![
                "-NoProfile".into(),
                "-ExecutionPolicy".into(),
                "Bypass".into(),
                "-File".into(),
                file.clone(),
            ],
        ),
        "pyw" => ("pythonw".into(), vec![file.clone()]),
        "sh" => ("bash".into(), vec![file.clone()]),
        "rb" => ("ruby".into(), vec![file.clone()]),
        "exe" | "bat" | "cmd" => (file.clone(), vec![]),
        _ => return Err(format!("不认识的脚本类型：.{}", ext)),
    };
    Ok(EntryGuess {
        kind: "script".into(),
        command,
        args,
        cwd,
        nickname: name,
        note: format!(".{} 脚本", ext),
        output_dir,
    })
}

fn scripts_dir(state: &Shared) -> std::path::PathBuf {
    state
        .path
        .parent()
        .map(|p| p.join("scripts"))
        .unwrap_or_else(|| std::path::PathBuf::from("scripts"))
}

/// English-safe folder name from a script name (avoids CJK paths some tools choke on).
fn slugify(name: &str) -> String {
    let mut s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        format!("script-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
    } else {
        s
    }
}

/// Like `inspect_target`, but a single script file is first copied into our own
/// `scripts/<slug>/` (English path, self-contained). Project directories are
/// referenced in place — copying their deps/data would be wrong.
#[tauri::command]
pub fn import_local(state: State<'_, Shared>, path: String) -> Result<EntryGuess, String> {
    let guess = inspect_target(path.clone())?;
    let p = std::path::Path::new(&path);
    if p.is_file() {
        let slug = slugify(&guess.nickname);
        let dir = scripts_dir(state.inner()).join(&slug);
        std::fs::create_dir_all(&dir).map_err(|e| format!("建脚本目录失败: {}", e))?;
        let fname = p.file_name().and_then(|s| s.to_str()).ok_or("文件名无效")?;
        let dest = dir.join(fname);
        std::fs::copy(p, &dest).map_err(|e| format!("复制脚本失败: {}", e))?;
        // Re-detect on the copy so command/args/cwd point at our managed copy.
        return inspect_target(dest.display().to_string());
    }
    Ok(guess)
}

#[tauri::command]
pub fn list_channels(state: State<'_, Shared>) -> Vec<ChannelInfo> {
    let db = lk(&state.db);
    channels::list(&db.popo, &db.bot_channels)
}

#[tauri::command]
pub fn save_bot_channel(
    state: State<'_, Shared>,
    app: AppHandle,
    mut channel: BotChannel,
    secret: String,
) -> Result<BotChannelView, String> {
    let secret = secret.trim().to_string();
    let existing = if channel.id.is_empty() {
        None
    } else {
        lk(&state.db)
            .bot_channels
            .iter()
            .find(|item| item.id == channel.id)
            .cloned()
    };
    if channel.id.is_empty() {
        channel.id = uuid::Uuid::new_v4().to_string();
        channel.enabled = false;
        channel.created_at = Local::now().to_rfc3339();
    } else if let Some(previous) = &existing {
        channel.enabled = previous.enabled;
        channel.created_at = previous.created_at.clone();
        if previous.platform != channel.platform && secret.is_empty() {
            return Err("更换平台时必须重新填写机器人凭据".into());
        }
    } else {
        return Err("找不到要编辑的机器人".into());
    }
    let new_secret = (!secret.is_empty()).then_some(secret.as_str());
    if new_secret.is_none() && !vault::contains(&vault::bot_channel_key(&channel.id)) {
        return Err("请填写 QQ AppSecret".into());
    }
    channels::validate_channel(&mut channel, new_secret)?;
    if existing.as_ref().map(|item| item.enabled).unwrap_or(false) {
        channels::stop_and_wait(state.inner(), &channel.id)?;
    }
    if let Some(value) = new_secret {
        vault::set(&vault::bot_channel_key(&channel.id), value)?;
    }
    {
        let mut db = lk(&state.db);
        db.bot_channels.retain(|item| item.id != channel.id);
        db.bot_channels.push(channel.clone());
    }
    state.save();
    state.flush();
    if channel.enabled {
        if let Err(error) = channels::start(state.inner().clone(), app.clone(), channel.clone()) {
            if let Some(saved) = lk(&state.db)
                .bot_channels
                .iter_mut()
                .find(|item| item.id == channel.id)
            {
                saved.enabled = false;
            }
            state.save();
            state.flush();
            return Err(error);
        }
    }
    emit(&app);
    channels::bot_views(state.inner(), &[channel])
        .into_iter()
        .next()
        .ok_or_else(|| "保存机器人失败".to_string())
}

#[tauri::command]
pub fn set_bot_channel_enabled(
    state: State<'_, Shared>,
    app: AppHandle,
    id: Id,
    enabled: bool,
) -> Result<(), String> {
    if enabled && !vault::contains(&vault::bot_channel_key(&id)) {
        return Err("机器人凭据缺失，请先编辑并保存".into());
    }
    let channel = {
        let mut db = lk(&state.db);
        let channel = db
            .bot_channels
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| "找不到这个机器人".to_string())?;
        channel.enabled = enabled;
        channel.clone()
    };
    state.save();
    state.flush();
    emit(&app);
    if enabled {
        if let Err(error) = channels::start(state.inner().clone(), app.clone(), channel) {
            if let Some(saved) = lk(&state.db)
                .bot_channels
                .iter_mut()
                .find(|item| item.id == id)
            {
                saved.enabled = false;
            }
            state.save();
            state.flush();
            emit(&app);
            return Err(error);
        }
    } else {
        channels::stop_and_wait(state.inner(), &id)?;
    }
    Ok(())
}

#[tauri::command]
pub fn test_bot_channel(state: State<'_, Shared>, id: Id) -> Result<String, String> {
    let channel = lk(&state.db)
        .bot_channels
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .ok_or_else(|| "找不到这个机器人".to_string())?;
    channels::probe(&channel)?;
    if channel.enabled && channels::is_online(state.inner(), &channel.id) {
        channels::send(state.inner(), &channel, "来自 Mosaic 的测试消息")?;
        Ok("WebSocket 在线，测试消息已发送".into())
    } else {
        Ok("凭据有效，已取得官方 WebSocket 网关；启用后将建立长连接".into())
    }
}

#[tauri::command]
pub fn delete_bot_channel(state: State<'_, Shared>, app: AppHandle, id: Id) -> Result<(), String> {
    let channel_id = format!("bot:{}", id);
    channels::stop_and_wait(state.inner(), &id)?;
    vault::delete(&vault::bot_channel_key(&id))?;
    let existed = {
        let mut db = lk(&state.db);
        let before = db.bot_channels.len();
        db.bot_channels.retain(|item| item.id != id);
        for task in &mut db.tasks {
            if task.push_channel.as_deref() == Some(channel_id.as_str()) {
                task.push_channel = None;
            }
        }
        db.bot_channels.len() != before
    };
    if !existed {
        return Err("找不到这个机器人".into());
    }
    state.save();
    state.flush();
    emit(&app);
    Ok(())
}

#[tauri::command]
pub fn save_popo_config(state: State<'_, Shared>, app: AppHandle, config: PopoConfig) {
    {
        let mut db = lk(&state.db);
        db.popo = config;
    }
    state.save();
    state.flush();
    emit(&app);
}

/// Ensure a stable fingerprint exists (generated on first use), returning the
/// (alias, fingerprint) to advertise. Caller persists.
fn ensure_fingerprint(state: &Shared) -> (String, String) {
    let mut db = lk(&state.db);
    if db.popo.fingerprint.is_empty() {
        db.popo.fingerprint = uuid::Uuid::new_v4().to_string();
    }
    (db.popo.alias.clone(), db.popo.fingerprint.clone())
}

#[tauri::command]
pub fn popo_scan(state: State<'_, Shared>) -> Vec<PopoPeer> {
    let (alias, fp) = ensure_fingerprint(state.inner());
    state.save();
    state.flush();
    popo::scan(&alias, &fp)
}

#[tauri::command]
pub fn save_window_config(
    state: State<'_, Shared>,
    app: AppHandle,
    config: WindowConfig,
) -> Result<(), String> {
    {
        let mut db = lk(&state.db);
        db.window = config.clone();
    }
    state.save();
    state.flush();
    crate::ensure_widget(&app, config.widget)?;
    emit(&app);
    Ok(())
}

#[tauri::command]
pub fn send_to_popo(state: State<'_, Shared>, text: String) -> Result<(), String> {
    let (alias, fp) = ensure_fingerprint(state.inner());
    state.save();
    state.flush();
    let target = lk(&state.db)
        .popo
        .target
        .clone()
        .ok_or("还没有选择 PoPo 目标设备")?;
    let tmp = std::env::temp_dir().join(format!("mosaic-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&tmp, text.as_bytes()).map_err(|e| format!("写临时文件失败: {}", e))?;
    let r = popo::send_file(&target, &alias, &fp, &tmp.to_string_lossy());
    let _ = std::fs::remove_file(&tmp);
    r
}

#[tauri::command]
pub fn daily_brief(state: State<'_, Shared>) -> Brief {
    compose_brief(&lk(&state.db))
}
