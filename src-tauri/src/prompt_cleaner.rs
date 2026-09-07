use crate::state::{lk, Shared};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use tauri::State;

const TASK_ID: &str = "plugin-prompt-cleaner";

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptRule {
    rule_type: String,
    find: String,
    replace: String,
    ignore_case: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCleanerConfig {
    root: String,
    recursive: bool,
    rules: Vec<PromptRule>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCleanerResult {
    scanned: usize,
    changed: usize,
    matched_rules: usize,
}

fn config_path(state: &Shared) -> Result<std::path::PathBuf, String> {
    let app_dir = state.path.parent().ok_or("无法确定 Mosaic 数据目录")?;
    let dir = app_dir.join("state").join(TASK_ID);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

fn validate(config: &PromptCleanerConfig) -> Result<(), String> {
    if config.root.trim().is_empty() {
        return Err("请填写根目录。".into());
    }
    if config.rules.len() > 10_000 {
        return Err("规则数量不能超过 10000 条。".into());
    }
    if !config.rules.iter().any(|rule| !rule.find.trim().is_empty()) {
        return Err("请至少添加一条包含查找内容的规则。".into());
    }
    if config.rules.iter().any(|rule| rule.rule_type != "delete" && rule.rule_type != "replace") {
        return Err("发现未知规则类型。".into());
    }
    Ok(())
}

fn save(state: &Shared, config: &PromptCleanerConfig) -> Result<(), String> {
    validate(config)?;
    let path = config_path(state)?;
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).or_else(|_| {
        std::fs::copy(&temp, &path)?;
        std::fs::remove_file(&temp)
    }).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn prompt_cleaner_load(state: State<'_, Shared>) -> Result<PromptCleanerConfig, String> {
    let path = config_path(state.inner())?;
    if !path.is_file() {
        return Ok(PromptCleanerConfig {
            root: String::new(),
            recursive: true,
            rules: vec![PromptRule {
                rule_type: "delete".into(),
                find: String::new(),
                replace: String::new(),
                ignore_case: false,
            }],
        });
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("清洗配置损坏: {e}"))
}

#[tauri::command]
pub fn prompt_cleaner_save(
    state: State<'_, Shared>,
    config: PromptCleanerConfig,
) -> Result<(), String> {
    save(state.inner(), &config)
}

#[tauri::command]
pub async fn prompt_cleaner_run(
    state: State<'_, Shared>,
    config: PromptCleanerConfig,
) -> Result<PromptCleanerResult, String> {
    save(state.inner(), &config)?;
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let task = lk(&shared.db)
            .tasks
            .iter()
            .find(|task| task.id == TASK_ID && task.active)
            .cloned()
            .ok_or("提示词清洗插件尚未安装或未启用。")?;
        let state_dir = config_path(&shared)?
            .parent()
            .ok_or("无法确定插件状态目录")?
            .to_path_buf();
        let mut command = Command::new(&task.command);
        command.args(&task.args).arg("-RunSaved");
        if let Some(cwd) = &task.cwd { command.current_dir(cwd); }
        command.env("MOSAIC_TASK_DIR", state_dir);
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let output = command.output().map_err(|e| format!("无法启动清洗插件: {e}"))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if message.is_empty() { "清洗插件运行失败。".into() } else { message });
        }
        serde_json::from_slice::<PromptCleanerResult>(&output.stdout)
            .map_err(|e| format!("无法读取插件处理结果: {e}"))
    }).await.map_err(|e| e.to_string())?
}
