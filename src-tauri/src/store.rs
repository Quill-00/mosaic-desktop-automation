use crate::model::Db;
use std::path::{Path, PathBuf};

pub fn load(path: &Path) -> Db {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Db::default(),
    }
}

/// Write the database atomically: serialize to a sibling temp file, then rename
/// over the target. A crash mid-write can never leave a half-written db.json
/// (which `load` would silently discard).
pub fn save(path: &Path, db: &Db) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = match serde_json::to_string_pretty(db) {
        Ok(s) => s,
        Err(_) => return,
    };
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, &json).is_ok() {
        if std::fs::rename(&tmp, path).is_err() {
            // Fallback for platforms/filesystems where rename-over-existing fails.
            let _ = std::fs::write(path, &json);
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("db.json")
}

pub fn cleanup_backups(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return; };
    let mut backups = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() { return None; }
            let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
            let backup = name.contains("backup")
                || name.contains("checkpoint")
                || name.contains(".pre-")
                || name.contains("running-old");
            if !backup { return None; }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    backups.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in backups.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}
