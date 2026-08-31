use crate::model::*;
use crate::runner;
use crate::state::{lk, Shared};
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::AppHandle;

const DEBOUNCE: Duration = Duration::from_millis(800);

/// (Re)start file watchers for every enabled task with a Watch trigger. The
/// watcher handle is kept alive in [`crate::state::Inner::watchers`]; dropping it
/// stops the watch.
pub fn start_all(state: Shared, app: AppHandle) {
    let tasks: Vec<Task> = { lk(&state.db).tasks.clone() };
    for task in tasks {
        if !task.active || !task.enabled {
            continue;
        }
        if let Trigger::Watch {
            path,
            pattern,
            recursive,
        } = task.trigger.clone()
        {
            start_one(&state, &app, &task, path, pattern, recursive);
        }
    }
}

fn start_one(
    state: &Shared,
    app: &AppHandle,
    task: &Task,
    path: String,
    pattern: String,
    recursive: bool,
) {
    let st = state.clone();
    let ap = app.clone();
    let t = task.clone();
    let last = Arc::new(Mutex::new(Instant::now() - DEBOUNCE * 2));

    let mut watcher =
        match notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            let event = match res {
                Ok(e) => e,
                Err(_) => return,
            };
            if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                return;
            }
            let hit = event.paths.iter().any(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| matches(&pattern, n))
                    .unwrap_or(false)
            });
            if !hit {
                return;
            }
            {
                let mut l = last.lock().unwrap_or_else(|e| e.into_inner());
                if l.elapsed() < DEBOUNCE {
                    return;
                }
                *l = Instant::now();
            }
            runner::run_task(st.clone(), ap.clone(), t.clone(), "看守".into());
        }) {
            Ok(w) => w,
            Err(_) => return,
        };

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    if watcher.watch(Path::new(&path), mode).is_ok() {
        lk(&state.watchers).insert(task.id.clone(), watcher);
    }
}

pub fn restart_all(state: &Shared, app: &AppHandle) {
    lk(&state.watchers).clear();
    start_all(state.clone(), app.clone());
}

/// Glob-ish filename match: `*` (all), `*.ext` (suffix), comma/space separated
/// alternatives, or an exact name. Case-insensitive.
pub fn matches(pattern: &str, name: &str) -> bool {
    let pattern = pattern.trim().to_lowercase();
    let name = name.to_lowercase();
    if pattern.is_empty() || pattern == "*" {
        return true;
    }
    for part in pattern.split([',', ' ']).filter(|p| !p.is_empty()) {
        if part == "*" {
            return true;
        }
        if let Some(ext) = part.strip_prefix("*.") {
            if name.ends_with(&format!(".{}", ext)) {
                return true;
            }
        } else if part == name {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn star_matches_all() {
        assert!(matches("*", "anything.txt"));
        assert!(matches("", "anything.txt"));
    }

    #[test]
    fn ext_match() {
        assert!(matches("*.pdf", "report.PDF"));
        assert!(!matches("*.pdf", "report.txt"));
    }

    #[test]
    fn multi_pattern() {
        assert!(matches("*.png, *.jpg", "photo.jpg"));
        assert!(!matches("*.png, *.jpg", "doc.pdf"));
    }

    #[test]
    fn exact_name() {
        assert!(matches("config.json", "config.json"));
        assert!(!matches("config.json", "other.json"));
    }
}
