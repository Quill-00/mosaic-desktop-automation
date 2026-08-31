use crate::model::*;
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Lock a mutex, recovering from poisoning instead of cascading panics. One
/// panicked thread should not take the whole engine down.
pub fn lk<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A live, platform-managed child process. Keyed by task id in [`Inner::running`]
/// (one concurrent run per task). The `kill` flag is the cross-thread signal the
/// run loop watches to terminate cooperatively, then forcefully.
pub struct RunHandle {
    pub exec_id: Id,
    pub task_id: Id,
    pub nickname: String,
    pub pid: u32,
    pub started_at: DateTime<Local>,
    pub lifecycle: Lifecycle,
    pub command: String,
    pub kill: Arc<AtomicBool>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningInfo {
    pub exec_id: Id,
    pub task_id: Id,
    pub nickname: String,
    pub pid: u32,
    pub started_at: String,
    pub lifecycle: Lifecycle,
    pub command: String,
    pub uptime_secs: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BotConnectionStatus {
    Stopped,
    Connecting,
    Online,
    Error,
}

#[derive(Debug, Clone)]
pub struct BotConnectionState {
    pub status: BotConnectionStatus,
    pub detail: String,
}

/// Handle for one QQ WebSocket worker. `kill` prevents reconnects and asks the
/// active socket loop to close; `done` lets switch-off/delete wait until the
/// worker has actually exited instead of merely changing a UI flag.
pub struct BotConnectionHandle {
    pub kill: Arc<AtomicBool>,
    pub done: Arc<AtomicBool>,
    pub state: Arc<Mutex<BotConnectionState>>,
}

impl RunHandle {
    pub fn info(&self) -> RunningInfo {
        let uptime = (Local::now() - self.started_at).num_seconds();
        RunningInfo {
            exec_id: self.exec_id.clone(),
            task_id: self.task_id.clone(),
            nickname: self.nickname.clone(),
            pid: self.pid,
            started_at: self.started_at.to_rfc3339(),
            lifecycle: self.lifecycle,
            command: self.command.clone(),
            uptime_secs: uptime,
        }
    }
}

/// Shared application state. Everything the engine touches lives behind a mutex
/// here; threads hold an `Arc<Inner>` ([`Shared`]).
pub struct Inner {
    pub db: Mutex<Db>,
    pub path: PathBuf,
    pub running: Mutex<HashMap<Id, RunHandle>>,
    pub last_runs: Mutex<HashMap<Id, DateTime<Local>>>,
    pub watchers: Mutex<HashMap<Id, notify::RecommendedWatcher>>,
    pub bot_connections: Mutex<HashMap<Id, BotConnectionHandle>>,
    dirty: AtomicBool,
}

pub type Shared = Arc<Inner>;

impl Inner {
    pub fn new(db: Db, path: PathBuf) -> Self {
        Inner {
            db: Mutex::new(db),
            path,
            running: Mutex::new(HashMap::new()),
            last_runs: Mutex::new(HashMap::new()),
            watchers: Mutex::new(HashMap::new()),
            bot_connections: Mutex::new(HashMap::new()),
            dirty: AtomicBool::new(false),
        }
    }

    /// Mark the database dirty. A background flusher writes it out atomically;
    /// this keeps the hot path (every run / notification) off the disk.
    pub fn save(&self) {
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Write to disk now if dirty. Called by the flusher loop and on shutdown.
    pub fn flush(&self) {
        if self.dirty.swap(false, Ordering::Relaxed) {
            let db = lk(&self.db);
            crate::store::save(&self.path, &db);
        }
    }

    /// Seed in-memory last-run times from persisted executions so a restart does
    /// not re-fire every interval task at once or double-fire daily tasks.
    pub fn seed_last_runs(&self) {
        let db = lk(&self.db);
        let mut lr = lk(&self.last_runs);
        for ex in db.executions.iter() {
            if let Ok(t) = DateTime::parse_from_rfc3339(&ex.started_at) {
                let local = t.with_timezone(&Local);
                lr.entry(ex.task_id.clone())
                    .and_modify(|cur| {
                        if local > *cur {
                            *cur = local;
                        }
                    })
                    .or_insert(local);
            }
        }
    }
}
