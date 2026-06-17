use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const AUTOSTART_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const AUTOSTART_NAME: &str = "TaskCap";

/// 界面透明度上限：0=不透明，50=最透（与设置页滑块一致）
pub const MAX_CAPSULE_TRANSPARENCY_PERCENT: u8 = 50;

fn default_island_offset_y() -> i32 {
    8
}

fn default_capsule_transparency_percent() -> u8 {
    0
}

fn default_show_capsule() -> bool {
    true
}

fn default_quick_add_shortcut() -> String {
    "Ctrl+Alt+N".to_string()
}

fn default_display_mode() -> String {
    "standard".to_string()
}

fn default_panel_background_preload() -> bool {
    true
}

fn default_panel_refresh_interval_secs() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_show_capsule")]
    pub show_capsule: bool,
    #[serde(default)]
    pub show_title_in_menu_bar: bool,
    /// 相对屏幕水平中心的像素偏移（拖拽持久化）
    #[serde(default)]
    pub island_offset_x: i32,
    /// 相对工作区顶部的像素偏移（对齐 macOS capsuleYOffset）
    #[serde(default = "default_island_offset_y")]
    pub island_offset_y: i32,
    /// 主面板上次所在的物理 X 坐标（None=未记录，首次按右下角定位）
    #[serde(default)]
    pub panel_pos_x: Option<i32>,
    /// 主面板上次所在的物理 Y 坐标
    #[serde(default)]
    pub panel_pos_y: Option<i32>,
    /// 任务岛全局界面透明度 0–50：0=不透明，50=最透（悬浮岛+面板+快速新增）
    #[serde(default = "default_capsule_transparency_percent")]
    pub capsule_transparency_percent: u8,
    /// 全局快捷键，格式 "Ctrl+Alt+N"
    #[serde(default = "default_quick_add_shortcut")]
    pub quick_add_shortcut: String,
    /// 面板显示模式："standard"=440px（对齐悬浮岛），"wide"=560px
    #[serde(default = "default_display_mode")]
    pub display_mode: String,
    /// 启动后在后台预创建主面板 WebView 并拉取数据，打开时无需等待
    #[serde(default = "default_panel_background_preload")]
    pub panel_background_preload: bool,
    /// 主面板已挂载时的自动刷新间隔（秒）
    #[serde(default = "default_panel_refresh_interval_secs")]
    pub panel_refresh_interval_secs: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            autostart: false,
            show_capsule: true,
            show_title_in_menu_bar: false,
            island_offset_x: 0,
            island_offset_y: default_island_offset_y(),
            panel_pos_x: None,
            panel_pos_y: None,
            capsule_transparency_percent: default_capsule_transparency_percent(),
            quick_add_shortcut: default_quick_add_shortcut(),
            display_mode: default_display_mode(),
            panel_background_preload: default_panel_background_preload(),
            panel_refresh_interval_secs: default_panel_refresh_interval_secs(),
        }
    }
}

pub fn config_path() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA").ok_or("APPDATA is not available")?;
    Ok(PathBuf::from(appdata)
        .join("taskcap")
        .join("config.json"))
}

fn normalize_config(mut config: AppConfig) -> AppConfig {
    config.capsule_transparency_percent = config
        .capsule_transparency_percent
        .min(MAX_CAPSULE_TRANSPARENCY_PERCENT);
    config.panel_refresh_interval_secs = config.panel_refresh_interval_secs.clamp(15, 600);
    config
}

pub fn load_config() -> AppConfig {
    let path = match config_path() {
        Ok(path) => path,
        Err(_) => return AppConfig::default(),
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    normalize_config(serde_json::from_str(&raw).unwrap_or_default())
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

pub fn read_autostart_enabled() -> bool {
    let output = Command::new("reg")
        .args(["query", AUTOSTART_KEY, "/v", AUTOSTART_NAME])
        .output();
    matches!(output, Ok(result) if result.status.success())
}

pub fn apply_autostart(enabled: bool) -> Result<(), String> {
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = Command::new("reg")
            .args(["add", AUTOSTART_KEY, "/v", AUTOSTART_NAME, "/t", "REG_SZ", "/d"])
            .arg(exe.to_string_lossy().to_string())
            .args(["/f"])
            .status()
            .map_err(|e| format!("写入开机自启失败：{e}"))?;
        if !status.success() {
            return Err("写入开机自启失败".to_string());
        }
        return Ok(());
    }

    let _ = Command::new("reg")
        .args(["delete", AUTOSTART_KEY, "/v", AUTOSTART_NAME, "/f"])
        .status();
    Ok(())
}

pub fn sync_autostart_flag(config: &mut AppConfig) {
    config.autostart = read_autostart_enabled();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = AppConfig::default();
        assert!(!config.autostart);
        assert!(config.show_capsule);
        assert!(!config.show_title_in_menu_bar);
        assert_eq!(config.capsule_transparency_percent, 0);
    }

    #[test]
    fn roundtrip_json() {
        let config = AppConfig {
            autostart: true,
            show_capsule: false,
            show_title_in_menu_bar: true,
            island_offset_x: 12,
            island_offset_y: 24,
            capsule_transparency_percent: 35,
        };
        let raw = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&raw).unwrap();
        assert!(parsed.autostart);
        assert!(!parsed.show_capsule);
        assert!(parsed.show_title_in_menu_bar);
        assert_eq!(parsed.island_offset_x, 12);
        assert_eq!(parsed.island_offset_y, 24);
        assert_eq!(parsed.capsule_transparency_percent, 35);
    }

    #[test]
    fn clamps_legacy_transparency_above_max() {
        let raw = r#"{"capsuleTransparencyPercent":80}"#;
        let parsed: AppConfig = serde_json::from_str(raw).unwrap();
        let normalized = normalize_config(parsed);
        assert_eq!(
            normalized.capsule_transparency_percent,
            MAX_CAPSULE_TRANSPARENCY_PERCENT
        );
    }
}
