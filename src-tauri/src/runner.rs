use crate::model::*;
use crate::state::{lk, Inner, RunHandle, Shared};
use chrono::{DateTime, Local};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const POLL_MS: u64 = 80;

/// Launch a task on a background thread. The platform is the sole launcher;
/// scripts never start themselves. Returns immediately. Skips if the task is
/// already running (one concurrent run per task).
pub fn run_task(state: Shared, app: AppHandle, task: Task, trigger_label: String) {
    {
        let running = lk(&state.running);
        if running.contains_key(&task.id) {
            return;
        }
    }
    std::thread::spawn(move || execute(state, app, task, trigger_label));
}

pub fn terminate(state: &Inner, task_id: &str) -> bool {
    let running = lk(&state.running);
    if let Some(h) = running.get(task_id) {
        h.kill.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

pub fn terminate_all(state: &Inner) {
    let running = lk(&state.running);
    for h in running.values() {
        h.kill.store(true, Ordering::Relaxed);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = Command::new("taskkill")
                .creation_flags(CREATE_NO_WINDOW)
                .args(["/PID", &h.pid.to_string(), "/T", "/F"])
                .output();
        }
    }
}

pub fn push_notification(
    state: &Inner,
    level: &str,
    title: &str,
    body: Option<&str>,
    task_id: Option<&str>,
) {
    let mut db = lk(&state.db);
    db.notifications.insert(
        0,
        Notification {
            id: uuid::Uuid::new_v4().to_string(),
            level: level.into(),
            title: title.into(),
            body: body.map(|s| s.to_string()),
            at: Local::now().to_rfc3339(),
            read: false,
            task_id: task_id.map(|s| s.to_string()),
        },
    );
    db.notifications.truncate(100);
}

/// After a successful run, push the task's summary to its configured channel
/// (currently PoPo). Best-effort: a delivery failure becomes a notification, not
/// a task failure.
fn maybe_push(state: &Inner, task: &Task, headline: Option<String>) {
    let channel_id = match task.push_channel.as_deref() {
        Some(value) => value,
        None => return,
    };
    let msg = format!(
        "[{}] {}",
        task.nickname,
        headline.unwrap_or_else(|| "已更新".into())
    );
    if let Some(bot_id) = channel_id.strip_prefix("bot:") {
        let channel = lk(&state.db)
            .bot_channels
            .iter()
            .find(|channel| channel.id == bot_id)
            .cloned();
        if let Some(channel) = channel {
            if let Err(error) = crate::channels::send(state, &channel, &msg) {
                push_notification(
                    state,
                    "warning",
                    &format!("{} 推送机器人失败", task.nickname),
                    Some(&error),
                    Some(&task.id),
                );
            }
        }
        return;
    }
    if channel_id != "popo" {
        return;
    }
    let cfg = { lk(&state.db).popo.clone() };
    if !cfg.enabled || cfg.fingerprint.is_empty() {
        return;
    }
    let target = match cfg.target {
        Some(t) => t,
        None => return,
    };
    let tmp = std::env::temp_dir().join(format!("mosaic-{}.txt", uuid::Uuid::new_v4()));
    if std::fs::write(&tmp, msg.as_bytes()).is_err() {
        return;
    }
    let r = crate::popo::send_file(
        &target,
        &cfg.alias,
        &cfg.fingerprint,
        &tmp.to_string_lossy(),
    );
    let _ = std::fs::remove_file(&tmp);
    if let Err(e) = r {
        push_notification(
            state,
            "warning",
            &format!("{} 推送 PoPo 失败", task.nickname),
            Some(&e),
            Some(&task.id),
        );
    }
}

fn emit_changed(app: &AppHandle) {
    let _ = app.emit("mosaic:changed", ());
}

fn trim_exec(db: &mut Db) {
    if db.executions.len() > 200 {
        db.executions.truncate(200);
    }
}

/// Kill the whole process tree. On Windows `taskkill /T` reaches grandchildren
/// (e.g. processes a node script itself spawned); `child.kill()` is the fallback
/// and the reaper.
fn kill_tree(pid: u32, child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = Command::new("taskkill")
            .creation_flags(CREATE_NO_WINDOW)
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn record_failed(
    state: &Inner,
    app: &AppHandle,
    task: &Task,
    exec_id: &str,
    started: &DateTime<Local>,
    trigger: &str,
    err: &str,
) {
    {
        let mut db = lk(&state.db);
        // For resident plugins the switch describes a live service. If the
        // process cannot even be spawned, converge it back to off instead of
        // leaving the UI stuck forever on "starting".
        if task.lifecycle == Lifecycle::Resident {
            if let Some(saved) = db.tasks.iter_mut().find(|saved| saved.id == task.id) {
                saved.enabled = false;
            }
        }
        db.executions.insert(
            0,
            Execution {
                id: exec_id.into(),
                task_id: task.id.clone(),
                nickname: task.nickname.clone(),
                started_at: started.to_rfc3339(),
                finished_at: Some(Local::now().to_rfc3339()),
                status: ExecStatus::Failed,
                exit_code: None,
                trigger: trigger.into(),
                error: Some(err.into()),
                item_count: 0,
                items: vec![],
            },
        );
        trim_exec(&mut db);
    }
    push_notification(
        state,
        "danger",
        &format!("{} 启动失败", task.nickname),
        Some(err),
        Some(&task.id),
    );
    state.save();
    emit_changed(app);
}

/// Merge one parsed output into a task's result state (summary / card / prepended
/// timeline). Returns the number of timeline items added.
fn apply_result(
    state: &Inner,
    task_id: &str,
    summary: Option<Summary>,
    card: Option<serde_json::Value>,
    items: Vec<DetailItem>,
    cursor: Option<String>,
) -> usize {
    let added = items.len();
    let mut db = lk(&state.db);
    let entry = db.results.entry(task_id.to_string()).or_default();
    if summary.is_some() {
        entry.summary = summary;
    }
    if cursor.is_some() {
        entry.cursor = cursor;
    }
    if card.is_some() {
        entry.card = card;
    }
    if !items.is_empty() {
        let mut tl = items;
        tl.extend(entry.timeline.clone());
        tl.truncate(200);
        entry.timeline = tl;
    }
    entry.updated_at = Some(Local::now().to_rfc3339());
    added
}

fn execute(state: Shared, app: AppHandle, task: Task, trigger_label: String) {
    let exec_id = uuid::Uuid::new_v4().to_string();
    let started = Local::now();

    let mut cmd = Command::new(&task.command);
    cmd.args(&task.args);
    if let Some(cwd) = &task.cwd {
        if !cwd.is_empty() {
            cmd.current_dir(cwd);
        }
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    // Feed canned answers to interactive CLIs; null stdin otherwise so a prompt
    // gets EOF instead of hanging forever.
    let feed = task.stdin.clone().filter(|s| !s.is_empty());
    cmd.stdin(if feed.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });

    // Incremental-crawl scaffolding: hand the script its last run time, last
    // cursor and a persistent state dir, so resume-from-checkpoint is easy.
    {
        let (prev_cursor, prev_run) = {
            let db = lk(&state.db);
            match db.results.get(&task.id) {
                Some(r) => (r.cursor.clone(), r.updated_at.clone()),
                None => (None, None),
            }
        };
        if let Some(app_dir) = state.path.parent() {
            let tdir = app_dir.join("state").join(&task.id);
            let _ = std::fs::create_dir_all(&tdir);
            cmd.env("MOSAIC_TASK_DIR", &tdir);
        }
        cmd.env("MOSAIC_TASK_ID", &task.id);
        if let Some(lr) = prev_run {
            cmd.env("MOSAIC_LAST_RUN", lr);
        }
        if let Some(c) = prev_cursor {
            cmd.env("MOSAIC_CURSOR", c);
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            record_failed(
                &state,
                &app,
                &task,
                &exec_id,
                &started,
                &trigger_label,
                &format!("无法启动: {}", e),
            );
            return;
        }
    };
    let pid = child.id();

    if let Some(input) = &feed {
        if let Some(mut si) = child.stdin.take() {
            let _ = si.write_all(input.as_bytes());
            if !input.ends_with('\n') {
                let _ = si.write_all(b"\n");
            }
            // si dropped here closes stdin, signalling EOF to the script.
        }
    }

    {
        let mut db = lk(&state.db);
        db.executions.insert(
            0,
            Execution {
                id: exec_id.clone(),
                task_id: task.id.clone(),
                nickname: task.nickname.clone(),
                started_at: started.to_rfc3339(),
                finished_at: None,
                status: ExecStatus::Running,
                exit_code: None,
                trigger: trigger_label.clone(),
                error: None,
                item_count: 0,
                items: vec![],
            },
        );
        trim_exec(&mut db);
    }

    let kill = Arc::new(AtomicBool::new(false));
    {
        let mut running = lk(&state.running);
        running.insert(
            task.id.clone(),
            RunHandle {
                exec_id: exec_id.clone(),
                task_id: task.id.clone(),
                nickname: task.nickname.clone(),
                pid,
                started_at: started,
                lifecycle: task.lifecycle,
                command: format!("{} {}", task.command, task.args.join(" ")),
                kill: kill.clone(),
            },
        );
    }
    state.save();
    emit_changed(&app);

    let resident = task.lifecycle == Lifecycle::Resident;

    // stderr is always drained on its own thread so a full pipe can't block.
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let se = stderr_buf.clone();
    let mut se_pipe = child.stderr.take();
    let se_reader = std::thread::spawn(move || {
        if let Some(mut err) = se_pipe.take() {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            *se.lock().unwrap_or_else(|e| e.into_inner()) = s;
        }
    });

    // Resident tasks stream their stdout line-by-line and update the dashboard
    // as they go; ephemeral tasks are read to completion and parsed once.
    // `round_items` accumulates this run's products so they can be stored on the
    // execution record for per-round browsing.
    let resident_count = Arc::new(AtomicUsize::new(0));
    let round_items = Arc::new(Mutex::new(Vec::<DetailItem>::new()));
    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let so_reader = {
        let mut so_pipe = child.stdout.take();
        if resident {
            let st = state.clone();
            let ap = app.clone();
            let tid = task.id.clone();
            let counter = resident_count.clone();
            let ri = round_items.clone();
            std::thread::spawn(move || {
                if let Some(out) = so_pipe.take() {
                    let reader = BufReader::new(out);
                    for line in reader.lines().map_while(Result::ok) {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let (summary, card, items) = parse_output(&line);
                        {
                            let mut g = ri.lock().unwrap_or_else(|e| e.into_inner());
                            g.extend(items.iter().cloned());
                            g.truncate(200);
                        }
                        let added = apply_result(&st, &tid, summary, card, items, None);
                        counter.fetch_add(added, Ordering::Relaxed);
                        st.save();
                        emit_changed(&ap);
                    }
                }
            })
        } else {
            let so = stdout_buf.clone();
            std::thread::spawn(move || {
                if let Some(mut out) = so_pipe.take() {
                    let mut s = String::new();
                    let _ = out.read_to_string(&mut s);
                    *so.lock().unwrap_or_else(|e| e.into_inner()) = s;
                }
            })
        }
    };

    let timeout = task.timeout_secs;
    let status;
    let mut exit_code: Option<i32> = None;
    loop {
        if kill.load(Ordering::Relaxed) {
            kill_tree(pid, &mut child);
            status = ExecStatus::Killed;
            break;
        }
        match child.try_wait() {
            Ok(Some(st)) => {
                exit_code = st.code();
                status = if st.success() {
                    ExecStatus::Ok
                } else {
                    ExecStatus::Failed
                };
                break;
            }
            Ok(None) => {
                if timeout > 0 && (Local::now() - started).num_seconds() as u64 >= timeout {
                    kill_tree(pid, &mut child);
                    status = ExecStatus::TimedOut;
                    break;
                }
                std::thread::sleep(Duration::from_millis(POLL_MS));
            }
            Err(_) => {
                status = ExecStatus::Failed;
                break;
            }
        }
    }
    let _ = so_reader.join();
    let _ = se_reader.join();
    let stdout = stdout_buf.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let stderr = stderr_buf.lock().unwrap_or_else(|e| e.into_inner()).clone();

    {
        let mut running = lk(&state.running);
        running.remove(&task.id);
    }
    if resident && status != ExecStatus::Killed {
        let mut db = lk(&state.db);
        if let Some(saved) = db.tasks.iter_mut().find(|saved| saved.id == task.id) {
            saved.enabled = false;
        }
    }

    let finished = Local::now();
    let mut item_count = resident_count.load(Ordering::Relaxed);

    if status == ExecStatus::Ok && !resident {
        // A task with an output dir produces files (e.g. a crawler's downloads);
        // show those as the product. Otherwise fall back to parsing stdout.
        let (summary, card, items) = match task.output_dir.as_deref() {
            Some(dir) if !dir.trim().is_empty() => {
                let items = dir_items(dir);
                let n = items.len();
                let summary = Some(Summary {
                    headline: format!("{} 个文件", n),
                    count: Some(n as i64),
                    note: None,
                });
                (summary, Some(list_card(None, &items)), items)
            }
            _ => parse_output(&stdout),
        };
        item_count = items.len();
        {
            let mut g = round_items.lock().unwrap_or_else(|e| e.into_inner());
            *g = items.iter().take(200).cloned().collect();
        }
        let cursor = parse_cursor(&stdout);
        let headline = summary.as_ref().map(|s| s.headline.clone());
        apply_result(&state, &task.id, summary, card, items, cursor);
        maybe_push(&state, &task, headline);
    } else if status != ExecStatus::Ok && status != ExecStatus::Killed {
        let msg = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("{:?}", status)
        };
        push_notification(
            &state,
            "danger",
            &format!("{} 运行失败", task.nickname),
            Some(&msg),
            Some(&task.id),
        );
    } else if resident && status == ExecStatus::Ok {
        push_notification(
            &state,
            "warning",
            &format!("{} 已停止", task.nickname),
            Some("常驻插件已经退出，开关已自动关闭。"),
            Some(&task.id),
        );
    }

    let collected = {
        round_items
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    };
    {
        let mut db = lk(&state.db);
        if let Some(ex) = db.executions.iter_mut().find(|e| e.id == exec_id) {
            ex.finished_at = Some(finished.to_rfc3339());
            ex.status = status;
            ex.exit_code = exit_code;
            ex.item_count = item_count;
            ex.items = collected;
            if status != ExecStatus::Ok && !stderr.trim().is_empty() {
                ex.error = Some(stderr.trim().to_string());
            }
        }
    }
    state.save();
    emit_changed(&app);
}

/// Turn raw stdout into (summary, card, timeline items). Zero-config: a fully
/// structured `TaskOutput` wins; a bare card object or JSON array is wrapped;
/// otherwise each non-empty line becomes a list item.
pub fn parse_output(stdout: &str) -> (Option<Summary>, Option<serde_json::Value>, Vec<DetailItem>) {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return (None, None, vec![]);
    }

    if let Ok(out) = serde_json::from_str::<TaskOutput>(trimmed) {
        if out.summary.is_some() || out.card.is_some() || !out.items.is_empty() {
            let card = out.card.clone().or_else(|| {
                if !out.items.is_empty() {
                    Some(list_card(None, &out.items))
                } else {
                    None
                }
            });
            return (out.summary, card, out.items);
        }
    }

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if v.is_object() && v.get("type").is_some() {
            let items = items_from_card(&v);
            return (None, Some(v), items);
        }
        if let Some(arr) = v.as_array() {
            let items: Vec<DetailItem> = arr
                .iter()
                .map(|x| DetailItem {
                    title: value_to_text(x),
                    ..Default::default()
                })
                .collect();
            return (None, Some(list_card(None, &items)), items);
        }
    }

    let items: Vec<DetailItem> = trimmed
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(100)
        .map(|l| DetailItem {
            title: l.trim().to_string(),
            ..Default::default()
        })
        .collect();
    (None, Some(list_card(None, &items)), items)
}

fn list_card(title: Option<String>, items: &[DetailItem]) -> serde_json::Value {
    serde_json::json!({
        "type": "list",
        "title": title,
        "items": items.iter().map(|i| serde_json::json!({"text": i.title, "subtitle": i.subtitle})).collect::<Vec<_>>()
    })
}

fn items_from_card(v: &serde_json::Value) -> Vec<DetailItem> {
    let mut out = vec![];
    if let Some(arr) = v.get("items").and_then(|x| x.as_array()) {
        for it in arr {
            let title = it
                .get("title")
                .and_then(|x| x.as_str())
                .or_else(|| it.get("text").and_then(|x| x.as_str()))
                .unwrap_or("")
                .to_string();
            let subtitle = it
                .get("subtitle")
                .and_then(|x| x.as_str())
                .or_else(|| it.get("source").and_then(|x| x.as_str()))
                .map(|s| s.to_string());
            out.push(DetailItem {
                title,
                subtitle,
                ..Default::default()
            });
        }
    }
    out
}

fn value_to_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Pull a `cursor` checkpoint out of structured stdout, if present.
fn parse_cursor(stdout: &str) -> Option<String> {
    let t = stdout.trim();
    if t.is_empty() {
        return None;
    }
    serde_json::from_str::<TaskOutput>(t)
        .ok()
        .and_then(|o| o.cursor)
}

/// List the most recent files under `root` (recursive, bounded) as timeline
/// items — used for tasks whose product is files on disk.
fn dir_items(root: &str) -> Vec<DetailItem> {
    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf, u64)> = Vec::new();
    collect_files(std::path::Path::new(root), 0, &mut files);
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.truncate(60);
    files
        .into_iter()
        .map(|(mtime, path, size)| {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("file")
                .to_string();
            let at = DateTime::<chrono::Utc>::from(mtime).with_timezone(&Local);
            DetailItem {
                title: name.clone(),
                subtitle: Some(human_size(size)),
                at: Some(at.to_rfc3339()),
                kind: Some(file_kind(&name).into()),
                path: Some(path.display().to_string()),
                id: None,
            }
        })
        .collect()
}

fn collect_files(
    dir: &std::path::Path,
    depth: u8,
    out: &mut Vec<(std::time::SystemTime, std::path::PathBuf, u64)>,
) {
    if depth > 4 || out.len() > 2000 {
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    for entry in rd.flatten() {
        match entry.file_type() {
            Ok(ft) if ft.is_dir() => collect_files(&entry.path(), depth + 1, out),
            Ok(ft) if ft.is_file() => {
                if let Ok(meta) = entry.metadata() {
                    let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    out.push((mtime, entry.path(), meta.len()));
                }
            }
            _ => {}
        }
    }
}

/// Classify a file by extension so the detail view can render it as the right
/// medium: image / video / audio / generic file.
fn file_kind(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    let ends = |exts: &[&str]| exts.iter().any(|e| lower.ends_with(e));
    if ends(&[
        ".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".avif", ".svg",
    ]) {
        "image"
    } else if ends(&[".mp4", ".webm", ".mkv", ".mov", ".avi", ".m4v"]) {
        "video"
    } else if ends(&[".mp3", ".wav", ".flac", ".m4a", ".aac", ".ogg", ".opus"]) {
        "audio"
    } else {
        "file"
    }
}

fn human_size(bytes: u64) -> String {
    const U: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, U[0])
    } else {
        format!("{:.1} {}", v, U[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_output() {
        let s = r#"{"summary":{"headline":"hi","count":2},"card":{"type":"metric"},"items":[{"title":"a"},{"title":"b"}]}"#;
        let (summary, card, items) = parse_output(s);
        assert_eq!(summary.unwrap().count, Some(2));
        assert!(card.is_some());
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn wraps_bare_card() {
        let s = r#"{"type":"list","items":[{"text":"x"},{"text":"y"}]}"#;
        let (summary, card, items) = parse_output(s);
        assert!(summary.is_none());
        assert_eq!(card.unwrap()["type"], "list");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn wraps_json_array() {
        let (_, card, items) = parse_output(r#"["one","two","three"]"#);
        assert_eq!(card.unwrap()["type"], "list");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "one");
    }

    #[test]
    fn falls_back_to_lines() {
        let (_, card, items) = parse_output("alpha\n\nbeta\n");
        assert_eq!(card.unwrap()["type"], "list");
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].title, "beta");
    }

    #[test]
    fn empty_is_empty() {
        let (s, c, i) = parse_output("   \n  ");
        assert!(s.is_none() && c.is_none() && i.is_empty());
    }
}
