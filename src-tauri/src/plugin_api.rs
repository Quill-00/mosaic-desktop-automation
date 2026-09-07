use crate::model::{Lifecycle, Task, TaskKind};
use crate::runner;
use crate::state::{lk, Shared};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSurface {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub drawer: Option<PluginDrawer>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDrawer {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub drop: Option<PluginDrop>,
    #[serde(default)]
    pub actions: Vec<PluginAction>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDrop {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAction {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifest {
    schema_version: u32,
    id: String,
    #[serde(default)]
    ui: Option<PluginSurface>,
}

fn manifest_path(task: &Task) -> Option<PathBuf> {
    let cwd = Path::new(task.cwd.as_deref()?);
    ["mosaic-plugin.json", "mosaic-package.json"]
        .into_iter()
        .map(|name| cwd.join(name))
        .find(|path| path.is_file())
}

fn manifest(task: &Task) -> Option<PluginManifest> {
    let path = manifest_path(task)?;
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 256 * 1024 {
        return None;
    }
    let parsed: PluginManifest = serde_json::from_slice(&bytes).ok()?;
    if parsed.schema_version != 1 {
        return None;
    }
    let matches_task = parsed.id == task.id
        || task
            .community
            .as_ref()
            .is_some_and(|source| source.package_id == parsed.id);
    matches_task.then_some(parsed)
}

pub fn surfaces(tasks: &[Task]) -> HashMap<String, PluginSurface> {
    tasks
        .iter()
        .filter(|task| task.kind == TaskKind::Plugin)
        .filter_map(|task| manifest(task)?.ui.map(|ui| (task.id.clone(), ui)))
        .collect()
}

fn runnable_task(state: &Shared, id: &str) -> Result<Task, String> {
    lk(&state.db)
        .tasks
        .iter()
        .find(|task| task.id == id && task.kind == TaskKind::Plugin && task.active)
        .cloned()
        .ok_or_else(|| "插件不存在、未启用或已移除。".to_string())
}

#[tauri::command]
pub fn run_plugin_with_files(
    state: State<'_, Shared>,
    app: AppHandle,
    id: String,
    paths: Vec<String>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有收到文件路径。".into());
    }
    let mut task = runnable_task(state.inner(), &id)?;
    let drop = manifest(&task)
        .and_then(|manifest| manifest.ui)
        .and_then(|ui| ui.drawer)
        .and_then(|drawer| drawer.drop)
        .ok_or_else(|| "该插件没有声明文件拖放能力。".to_string())?;
    let accepted = paths
        .into_iter()
        .filter(|value| {
            let path = Path::new(value);
            path.is_dir()
                || (path.is_file()
                    && (drop.extensions.is_empty()
                        || path.extension().and_then(|value| value.to_str()).is_some_and(|ext| {
                            drop.extensions.iter().any(|allowed| {
                                allowed.trim_start_matches('.').eq_ignore_ascii_case(ext)
                            })
                        })))
        })
        .collect::<Vec<_>>();
    if accepted.is_empty() {
        return Err("拖入内容不符合插件声明的文件类型。".into());
    }
    task.args.extend(accepted);
    task.lifecycle = Lifecycle::Ephemeral;
    runner::run_task(state.inner().clone(), app, task, "插件文件拖放".into());
    Ok(())
}

#[tauri::command]
pub fn run_image_metadata_cleaner(
    state: State<'_, Shared>,
    app: AppHandle,
    paths: Vec<String>,
    mode: String,
    value: String,
) -> Result<(), String> {
    if paths.is_empty() {
        return Err("请先添加图片。".into());
    }
    if !matches!(mode.as_str(), "prefix" | "folder" | "overwrite") {
        return Err("不支持的副本保存方式。".into());
    }
    if value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err("名称设置无效或过长。".into());
    }

    let mut task = runnable_task(state.inner(), "plugin-image-metadata-cleaner")?;
    let extensions = ["jpg", "jpeg", "jfif", "png", "webp"];
    let accepted = paths
        .into_iter()
        .filter(|value| {
            let path = Path::new(value);
            path.is_dir()
                || (path.is_file()
                    && path.extension().and_then(|value| value.to_str()).is_some_and(|ext| {
                        extensions.iter().any(|allowed| allowed.eq_ignore_ascii_case(ext))
                    }))
        })
        .collect::<Vec<_>>();
    if accepted.is_empty() {
        return Err("没有可处理的 JPG、PNG 或 WebP 图片。".into());
    }

    task.args.extend(["-Mode".into(), mode, "-Value".into(), value]);
    task.args.extend(accepted);
    task.lifecycle = Lifecycle::Ephemeral;
    runner::run_task(state.inner().clone(), app, task, "图片元数据清洗".into());
    Ok(())
}

#[tauri::command]
pub fn run_plugin_action(
    state: State<'_, Shared>,
    app: AppHandle,
    id: String,
    action_id: String,
) -> Result<(), String> {
    let mut task = runnable_task(state.inner(), &id)?;
    let action = manifest(&task)
        .and_then(|manifest| manifest.ui)
        .and_then(|ui| ui.drawer)
        .and_then(|drawer| drawer.actions.into_iter().find(|action| action.id == action_id))
        .ok_or_else(|| "插件动作不存在。".to_string())?;
    task.args.extend(action.args);
    task.lifecycle = Lifecycle::Ephemeral;
    runner::run_task(state.inner().clone(), app, task, format!("插件动作：{}", action.label));
    Ok(())
}
