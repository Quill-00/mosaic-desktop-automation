mod channels;
mod commands;
mod community;
mod model;
mod popo;
mod runner;
mod scanner;
mod scheduler;
mod seed;
mod state;
mod store;
mod update;
mod vault;
mod watcher;

use state::{Inner, Shared};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tauri::{
    utils::{config::WindowEffectsConfig, WindowEffect},
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow,
};

static ENGINE: OnceLock<Shared> = OnceLock::new();
// Match The Tower's proven edge affordance: a near-invisible 12 px rail with a
// short internal rule, instead of a second floating panel competing for space.
const WIDGET_COLLAPSED: (f64, f64) = (12.0, 132.0);
const WIDGET_EXPANDED: (f64, f64) = (372.0, 596.0);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if update::try_launch_pending_at_startup() {
        return;
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            std::fs::create_dir_all(&data_dir).ok();
            let path = store::db_path(&data_dir);

            let mut db = store::load(&path);
            if !db.migrated_v1 {
                // One-time: drop the old fabricated demo tasks, install real examples.
                db.tasks.retain(|t| !seed::is_legacy_mock(t));
                for ex in seed::example_tasks_v1() {
                    if !db.tasks.iter().any(|t| t.id == ex.id) {
                        db.tasks.push(ex);
                    }
                }
                db.migrated_v1 = true;
            }
            if !db.migrated_v2 {
                for ex in seed::example_tasks_v2() {
                    if !db.tasks.iter().any(|t| t.id == ex.id) {
                        db.tasks.push(ex);
                    }
                }
                db.migrated_v2 = true;
            }
            if !db.migrated_v3 {
                for plugin in seed::builtin_plugins_v3() {
                    if !db.tasks.iter().any(|task| task.id == plugin.id) {
                        db.tasks.push(plugin);
                    }
                }
                db.migrated_v3 = true;
            }
            if !db.migrated_v4 {
                // Previously `enabled` served both as library membership and
                // as the runtime switch. Preserve running items in the active
                // workspace, tuck stopped legacy items into the shelf, and keep
                // the built-in CPA plugin visible with its switch initially off.
                for task in &mut db.tasks {
                    task.active = task.enabled || task.id == "plugin-cliproxyapi";
                    if !task.active {
                        task.enabled = false;
                        task.on_dashboard = false;
                    }
                }
                db.migrated_v4 = true;
            }
            if !db.migrated_v5 {
                // The early robot prototype stored Incoming Webhook URLs and
                // called them desktop bots. Preserve those records so the rest
                // of db.json remains readable, but never auto-start them.
                for bot in &mut db.bot_channels {
                    if bot.platform != model::BotPlatform::Qq {
                        bot.enabled = false;
                    }
                }
                db.migrated_v5 = true;
            }
            if !db.migrated_v6 {
                if db.community.sources.is_empty() {
                    db.community
                        .sources
                        .push(model::OFFICIAL_COMMUNITY_SOURCE.into());
                }
                db.migrated_v6 = true;
            }
            seed::repair_builtin_plugin_locations(&mut db.tasks);

            let inner = Arc::new(Inner::new(db, path));
            app.manage(inner.clone());
            let _ = ENGINE.set(inner.clone());

            inner.seed_last_runs();
            inner.save();
            inner.flush();

            {
                let s = inner.clone();
                let a = app.handle().clone();
                std::thread::spawn(move || scheduler::run_loop(s, a));
            }
            {
                let s = inner.clone();
                let a = app.handle().clone();
                watcher::start_all(s, a);
            }
            // Enabled resident plugins are live services: restore them whenever
            // Mosaic starts, just like toggling their switch on in the task list.
            let resident_tasks: Vec<_> = state::lk(&inner.db)
                .tasks
                .iter()
                .filter(|task| {
                    task.active && task.enabled && task.lifecycle == model::Lifecycle::Resident
                })
                .cloned()
                .collect();
            for task in resident_tasks {
                runner::run_task(
                    inner.clone(),
                    app.handle().clone(),
                    task,
                    "随 Mosaic 启动".into(),
                );
            }
            // Desktop QQ bots are resident WebSocket connections. Restore only
            // the channels whose own switch is enabled.
            channels::start_enabled(inner.clone(), app.handle().clone());
            update::schedule_check(inner.clone(), app.handle().clone());
            // Background flusher: keeps the hot path off the disk; writes when dirty.
            {
                let s = inner.clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(Duration::from_millis(1500));
                    s.flush();
                });
            }

            if state::lk(&inner.db).window.widget {
                let _ = ensure_widget(&app.handle().clone(), true);
            }

            build_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::exec_items,
            commands::open_path,
            commands::create_task,
            commands::delete_task,
            commands::set_active,
            commands::set_enabled,
            commands::run_now,
            commands::set_dashboard,
            commands::set_module_span,
            commands::terminate,
            commands::terminate_all,
            commands::mark_read,
            commands::mark_all_read,
            commands::delete_notification,
            commands::clear_notifications,
            commands::scan_script,
            commands::scan_task_source,
            commands::inspect_target,
            commands::import_local,
            commands::list_channels,
            commands::daily_brief,
            commands::save_popo_config,
            commands::send_to_popo,
            commands::popo_scan,
            commands::save_bot_channel,
            commands::set_bot_channel_enabled,
            commands::test_bot_channel,
            commands::delete_bot_channel,
            commands::save_window_config,
            community::save_community_sources,
            community::community_catalog,
            community::install_community_package,
            community::uninstall_community_package,
            update::check_for_updates,
            set_widget_expanded,
            snap_widget_to_edge,
            show_main_window
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let to_tray = ENGINE
                        .get()
                        .map(|e| state::lk(&e.db).window.minimize_to_tray)
                        .unwrap_or(false);
                    if to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(engine) = ENGINE.get() {
                runner::terminate_all(engine);
                channels::stop_all(engine);
                engine.flush();
            }
        }
    });
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItem::with_id(app, "show", "显示主面板", true, None::<&str>)?;
    let widget = MenuItem::with_id(app, "widget", "显示小组件", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &widget, &quit])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "widget" => {
                // Restore the widget from the tray: re-enable it, (re)create if needed,
                // and unhide it (the widget's "x" only hides it to the tray).
                if let Some(engine) = ENGINE.get() {
                    {
                        let mut db = state::lk(&engine.db);
                        db.window.widget = true;
                    }
                    engine.save();
                    engine.flush();
                }
                let _ = ensure_widget(app, true);
                if let Some(w) = app.get_webview_window("widget") {
                    let _ = w.show();
                    let _ = position_widget(app, true, false);
                    let _ = w.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

/// Open or close the frameless always-on-top desktop widget window.
pub(crate) fn ensure_widget(app: &tauri::AppHandle, on: bool) -> Result<(), String> {
    if on {
        if app.get_webview_window("widget").is_none() {
            let w = tauri::WebviewWindowBuilder::new(
                app,
                "widget",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("Mosaic Quick Control")
            .inner_size(WIDGET_COLLAPSED.0, WIDGET_COLLAPSED.1)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .effects(WindowEffectsConfig {
                effects: vec![WindowEffect::Acrylic],
                state: None,
                radius: None,
                color: None,
            })
            .build()
            .map_err(|e| format!("创建小组件窗口失败: {}", e))?;
            round_corners(&w);
            position_widget(app, false, true)?;
            let _ = w.show();
        }
    } else {
        if let Some(w) = app.get_webview_window("widget") {
            let _ = w.close();
        }
    }
    Ok(())
}

#[tauri::command]
fn set_widget_expanded(app: AppHandle, expanded: bool) -> Result<(), String> {
    position_widget(&app, expanded, false)
}

#[tauri::command]
fn snap_widget_to_edge(
    app: AppHandle,
    preferred_edge: Option<model::WidgetEdge>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "悬浮窗不存在".to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| "找不到显示器".to_string())?;
    let scale = monitor.scale_factor();
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let monitor_x = monitor.position().x as f64 / scale;
    let monitor_y = monitor.position().y as f64 / scale;
    let monitor_width = monitor.size().width as f64 / scale;
    let monitor_height = monitor.size().height as f64 / scale;
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    let current_x = position.x as f64 / scale;
    let current_y = position.y as f64 / scale;
    let edge = preferred_edge.unwrap_or_else(|| {
        if current_x + width / 2.0 < monitor_x + monitor_width / 2.0 {
            model::WidgetEdge::Left
        } else {
            model::WidgetEdge::Right
        }
    });
    let x = match edge {
        model::WidgetEdge::Left => monitor_x,
        model::WidgetEdge::Right => monitor_x + monitor_width - width,
    };
    let y = current_y.clamp(
        monitor_y,
        (monitor_y + monitor_height - height).max(monitor_y),
    );
    if (current_x - x).abs() > 0.5 || (current_y - y).abs() > 0.5 {
        window
            .set_position(LogicalPosition::new(x, y))
            .map_err(|error| error.to_string())?;
    }
    let engine = ENGINE
        .get()
        .ok_or_else(|| "Mosaic 引擎尚未就绪".to_string())?;
    {
        let mut db = state::lk(&engine.db);
        db.window.widget_edge = Some(edge);
        db.window.widget_y = Some(y);
    }
    engine.save();
    Ok(())
}

#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "主窗口不存在".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

/// Resize the shortcut around its snapped edge. Right-snapped windows grow to
/// the left and left-snapped windows grow to the right, so the activation strip
/// never jumps away from the pointer during an expand/collapse transition.
fn position_widget(app: &AppHandle, expanded: bool, initial: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("widget")
        .ok_or_else(|| "悬浮窗不存在".to_string())?;
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| "找不到显示器".to_string())?;
    let scale = monitor.scale_factor();
    let (width, height) = if expanded {
        (WIDGET_EXPANDED.0, WIDGET_EXPANDED.1)
    } else {
        (WIDGET_COLLAPSED.0, WIDGET_COLLAPSED.1)
    };
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let monitor_x = monitor_position.x as f64 / scale;
    let monitor_y = monitor_position.y as f64 / scale;
    let monitor_width = monitor_size.width as f64 / scale;
    let monitor_height = monitor_size.height as f64 / scale;

    let stored = ENGINE.get().map(|engine| {
        let db = state::lk(&engine.db);
        (db.window.widget_edge, db.window.widget_y)
    });
    let current_y = (!initial)
        .then(|| window.outer_position().ok())
        .flatten()
        .map(|point| point.y as f64 / scale);
    // Default left avoids The Tower's right-edge strip. The user's first drag
    // chooses the persistent edge from then on.
    let edge = stored
        .as_ref()
        .and_then(|(edge, _)| *edge)
        .unwrap_or(model::WidgetEdge::Left);
    let requested_y = current_y
        .or_else(|| stored.and_then(|(_, y)| y))
        .unwrap_or(monitor_y + 128.0);
    let x = match edge {
        model::WidgetEdge::Left => monitor_x,
        model::WidgetEdge::Right => monitor_x + monitor_width - width,
    };
    let y = requested_y.clamp(
        monitor_y,
        (monitor_y + monitor_height - height).max(monitor_y),
    );
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    window
        .set_position(LogicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())
}

#[cfg(windows)]
fn round_corners(window: &WebviewWindow) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    };
    let Ok(handle) = window.hwnd() else { return };
    let preference = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            handle,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::addr_of!(preference).cast(),
            std::mem::size_of_val(&preference) as u32,
        );
    }
}

#[cfg(not(windows))]
fn round_corners(_window: &WebviewWindow) {}
