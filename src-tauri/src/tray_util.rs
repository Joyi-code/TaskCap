use crate::config::AppConfig;
use crate::store::TaskStore;
use tauri::{image::Image, AppHandle, Manager};

pub const TRAY_ID: &str = "main";

const TRAY_ICON_PENDING: &[u8] = include_bytes!("../icons/32x32.png");
const TRAY_ICON_DONE: &[u8] = include_bytes!("../icons/Square44x44Logo.png");

fn tray_icon(all_done: bool) -> Result<Image<'static>, String> {
    let bytes = if all_done {
        TRAY_ICON_DONE
    } else {
        TRAY_ICON_PENDING
    };
    Image::from_bytes(bytes).map_err(|e| e.to_string())
}

pub fn tray_tooltip(store: &TaskStore, config: &AppConfig) -> String {
    if config.show_title_in_menu_bar {
        return store.menu_bar_title();
    }
    let counts = store.task_counts();
    if counts.total == 0 {
        "TaskCap · 已完成".to_string()
    } else {
        format!("TaskCap · {} 项待办", counts.total)
    }
}

pub fn refresh_tray(app: &AppHandle, store: &TaskStore, config: &AppConfig) -> Result<(), String> {
    let tooltip = tray_tooltip(store, config);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let all_done = store.incomplete_count() == 0;
        tray.set_icon(Some(tray_icon(all_done)?))
            .map_err(|e| e.to_string())?;
        tray.set_tooltip(Some(tooltip))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn apply_capsule_visibility(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let Some(island) = app.get_webview_window("island") else {
        return Ok(());
    };
    if config.show_capsule {
        island.show().map_err(|e| e.to_string())?;
    } else {
        // 已隐藏时再次 hide 在部分 Win32 环境会报错，不应阻断配置保存
        let _ = island.hide();
    }
    Ok(())
}