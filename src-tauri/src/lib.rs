mod config;
mod db;
mod deeplink;
mod models;
mod panel;
mod parser;
mod reminder;
mod store;
mod tray_util;

use config::AppConfig;
use models::TaskCounts;
use reminder::ReminderState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use store::TaskStore;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskSummary {
    id: String,
    title: String,
    priority: i32,
    is_completed: bool,
    is_marked_complete: bool,
    is_current: bool,
    is_in_today_queue: bool,
    due_at: Option<String>,
    reminder_at: Option<String>,
    estimated_minutes: Option<i32>,
    focus_remaining_seconds: Option<f64>,
    is_focus_running: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IslandSnapshot {
    focus_counts: TaskCounts,
    menu_bar_title: String,
    attention_task: Option<TaskSummary>,
    preview_tasks: Vec<TaskSummary>,
    expanded_height: u32,
    has_active_focus: bool,
    incomplete_count: usize,
}

fn task_summary_from(
    task: &models::TaskItem,
    store: &TaskStore,
    now: chrono::DateTime<chrono::Utc>,
    is_marked_complete: bool,
    default_focus_minutes: i32,
) -> TaskSummary {
    let is_focus_running = task.focus_started_at.is_some();
    let focus_remaining_seconds = if is_focus_running || task.focus_seconds() > 0.0 {
        Some(store.focus_remaining_seconds(task.id, now, default_focus_minutes))
    } else {
        None
    };

    TaskSummary {
        id: task.id.to_string(),
        title: task.title.clone(),
        priority: task.priority_raw_value.unwrap_or(1),
        is_completed: task.is_completed,
        is_marked_complete,
        is_current: task.is_current,
        is_in_today_queue: task.is_in_today_queue(),
        due_at: task.due_at.map(|d| d.to_rfc3339()),
        reminder_at: task.reminder_at.map(|d| d.to_rfc3339()),
        estimated_minutes: task.estimated_minutes,
        focus_remaining_seconds,
        is_focus_running,
    }
}

pub(crate) struct AppState {
    store: Mutex<TaskStore>,
    config: Mutex<AppConfig>,
    reminders: ReminderState,
    marked_complete: Mutex<HashSet<Uuid>>,
}

fn should_show_capsule(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .config
        .lock()
        .map(|config| config.show_capsule)
        .unwrap_or(true)
}

/// 前端 `island_ready` 已触发；用于避免启动兜底线程抢在首帧绘制前 show 岛。
static ISLAND_READY_RECEIVED: AtomicBool = AtomicBool::new(false);

/// 主面板后台预加载暂停截止（相对进程启动的单调毫秒），岛交互期间推迟以免阻塞 resize/invoke。
static PANEL_PRELOAD_PAUSE_DEADLINE_MS: AtomicU64 = AtomicU64::new(0);
/// 最近一次岛尺寸变更（resize_island_window），预加载前需冷却，避免与展开动画争主线程。
static LAST_ISLAND_RESIZE_MS: AtomicU64 = AtomicU64::new(0);
static APP_MONOTONIC_START: OnceLock<Instant> = OnceLock::new();

/// 岛 resize 结束后至少等待这么久再允许 panel 预加载（毫秒）
const ISLAND_RESIZE_COOLDOWN_MS: u64 = 800;

/// 快速新增 WebView 前端已挂载，可供快捷键首开时延迟聚焦。
static QUICKADD_FRONTEND_READY: AtomicBool = AtomicBool::new(false);
/// 快速新增后台预热只启动一次，避免重复排队创建 WebView。
static QUICKADD_PRELOAD_STARTED: AtomicBool = AtomicBool::new(false);

const QUICKADD_FOCUS_SCRIPT: &str = r#"
(function () {
  if (typeof window.__taskcapFocusQuickAddInput === "function") {
    window.__taskcapFocusQuickAddInput();
    return;
  }
  var el = document.querySelector(".quickadd-input");
  if (el) {
    el.focus();
    if (typeof el.select === "function") {
      el.select();
    }
  }
})();
"#;

fn monotonic_ms() -> u64 {
    let start = APP_MONOTONIC_START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u64
}

fn postpone_panel_preload_millis(ms: u64) {
    let deadline = monotonic_ms().saturating_add(ms);
    loop {
        let current = PANEL_PRELOAD_PAUSE_DEADLINE_MS.load(Ordering::Relaxed);
        if deadline <= current {
            break;
        }
        if PANEL_PRELOAD_PAUSE_DEADLINE_MS
            .compare_exchange_weak(current, deadline, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            break;
        }
    }
}

fn is_panel_preload_paused() -> bool {
    monotonic_ms() < PANEL_PRELOAD_PAUSE_DEADLINE_MS.load(Ordering::Relaxed)
}

fn mark_island_resize_activity() {
    LAST_ISLAND_RESIZE_MS.store(monotonic_ms(), Ordering::Relaxed);
    postpone_panel_preload_millis(12_000);
}

fn island_resize_cooldown_elapsed() -> bool {
    monotonic_ms().saturating_sub(LAST_ISLAND_RESIZE_MS.load(Ordering::Relaxed))
        >= ISLAND_RESIZE_COOLDOWN_MS
}

/// 岛无交互推迟且 resize 已冷却后再预加载 panel，兼顾「3 秒预加载」与急点岛不卡死。
fn wait_until_safe_for_panel_preload() {
    loop {
        while is_panel_preload_paused() {
            std::thread::sleep(Duration::from_millis(200));
        }
        if island_resize_cooldown_elapsed() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn refresh_tray_ui(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let config = state.config.lock().map_err(|e| e.to_string())?;
    tray_util::refresh_tray(app, &store, &config)
}

fn refresh_system_ui(app: &AppHandle) -> Result<(), String> {
    refresh_tray_ui(app)?;
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|e| e.to_string())?;
    tray_util::apply_capsule_visibility(app, &config)
}

/// 广播任务变更并刷新托盘/悬浮岛。调用前必须已释放 `store` 锁，否则会死锁。
fn notify_tasks_changed(app: &AppHandle) -> Result<(), String> {
    app.emit("tasks-changed", ())
        .map_err(|e| e.to_string())?;
    refresh_system_ui(app)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfigPatch {
    autostart: Option<bool>,
    show_capsule: Option<bool>,
    show_title_in_menu_bar: Option<bool>,
    island_offset_x: Option<i32>,
    island_offset_y: Option<i32>,
    capsule_transparency_percent: Option<u8>,
    quick_add_shortcut: Option<String>,
    display_mode: Option<String>,
    panel_background_preload: Option<bool>,
    panel_refresh_interval_secs: Option<u32>,
}

#[tauri::command]
fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config::sync_autostart_flag(&mut config);
    Ok(config.clone())
}

#[tauri::command]
fn save_app_config(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: AppConfigPatch,
) -> Result<AppConfig, String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    if let Some(autostart) = patch.autostart {
        config::apply_autostart(autostart)?;
        config.autostart = autostart;
    }
    // 仅在影响系统 UI 的字段变化时才刷新托盘/显隐/位置，
    // 避免拖动透明度滑块时高频调用 set_icon 导致托盘图标闪烁。
    let mut needs_system_ui = false;
    let mut needs_reposition = false;
    if let Some(show_capsule) = patch.show_capsule {
        config.show_capsule = show_capsule;
        needs_system_ui = true;
    }
    if let Some(show_title_in_menu_bar) = patch.show_title_in_menu_bar {
        config.show_title_in_menu_bar = show_title_in_menu_bar;
        needs_system_ui = true;
    }
    if let Some(island_offset_x) = patch.island_offset_x {
        config.island_offset_x = island_offset_x;
        needs_reposition = true;
    }
    if let Some(island_offset_y) = patch.island_offset_y {
        config.island_offset_y = island_offset_y.clamp(0, 120);
        needs_reposition = true;
    }
    if let Some(capsule_transparency_percent) = patch.capsule_transparency_percent {
        config.capsule_transparency_percent = capsule_transparency_percent
            .min(config::MAX_CAPSULE_TRANSPARENCY_PERCENT);
    }
    if let Some(quick_add_shortcut) = patch.quick_add_shortcut {
        config.quick_add_shortcut = quick_add_shortcut;
    }
    if let Some(display_mode) = patch.display_mode {
        config.display_mode = display_mode;
    }
    let mut trigger_panel_preload = false;
    if let Some(panel_background_preload) = patch.panel_background_preload {
        config.panel_background_preload = panel_background_preload;
        trigger_panel_preload = panel_background_preload;
    }
    if let Some(panel_refresh_interval_secs) = patch.panel_refresh_interval_secs {
        config.panel_refresh_interval_secs = panel_refresh_interval_secs.clamp(15, 600);
    }
    config::save_config(&config)?;
    let saved = config.clone();
    drop(config);
    app.emit("app-config-changed", &saved)
        .map_err(|e| e.to_string())?;
    if needs_system_ui {
        refresh_system_ui(&app)?;
    }
    if needs_reposition {
        reposition_island(&app)?;
    }
    if trigger_panel_preload {
        let app_for_preload = app.clone();
        let _ = app.run_on_main_thread(move || {
            preload_panel_background(&app_for_preload);
        });
    }
    Ok(saved)
}

#[tauri::command]
fn save_island_position(
    app: AppHandle,
    state: State<'_, AppState>,
    x: i32,
    y: i32,
    width: u32,
) -> Result<AppConfig, String> {
    let island = app
        .get_webview_window("island")
        .ok_or("island window not found")?;
    let monitor = island
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("primary monitor not found")?;
    let work = monitor.work_area();
    let center_x = work.position.x + (work.size.width as i32 - width as i32) / 2;

    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.island_offset_x = x - center_x;
    config.island_offset_y = (y - work.position.y).clamp(0, 120);
    config::save_config(&config)?;
    Ok(config.clone())
}

/// 持久化主面板位置到配置（忽略锁失败）。用 let-else 把 guard 绑定为普通局部变量，
/// 确保它先于 state 释放，避免 if-let 尾表达式临时值延长导致的借用生命周期错误。
fn persist_panel_position(app: &AppHandle, x: i32, y: i32) {
    let state = app.state::<AppState>();
    let Ok(mut config) = state.config.lock() else {
        return;
    };
    config.panel_pos_x = Some(x);
    config.panel_pos_y = Some(y);
    let _ = config::save_config(&config);
}

/// 持久化主面板位置：用户拖动结束后调用，使下次打开（含失焦隐藏后再开）记住位置。
#[tauri::command]
fn save_panel_position(state: State<'_, AppState>, x: i32, y: i32) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.panel_pos_x = Some(x);
    config.panel_pos_y = Some(y);
    config::save_config(&config)
}

#[tauri::command]
fn postpone_panel_preload(millis: Option<u64>) -> Result<(), String> {
    postpone_panel_preload_millis(millis.unwrap_or(10_000).clamp(1_000, 60_000));
    Ok(())
}

#[tauri::command]
fn resize_island_window(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    mark_island_resize_activity();
    let island = app
        .get_webview_window("island")
        .ok_or("island window not found")?;
    island
        .set_size(Size::Logical(LogicalSize::new(width, height)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 前端首屏渲染 + 初始尺寸就绪后调用：在窗口仍隐藏时一次性确定尺寸和位置，再显示。
/// 关键：先 set_size 让 outer_size 立即返回正确值（隐藏窗口的 outer_size 不可靠），
/// 再据此居中定位，最后 show，确保用户第一眼看到的就是已居中、尺寸正确的岛，零跳动。
#[tauri::command]
fn island_ready(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    ISLAND_READY_RECEIVED.store(true, Ordering::Relaxed);
    let island = app
        .get_webview_window("island")
        .ok_or("island window not found")?;
    if island.is_visible().unwrap_or(false) {
        return Ok(());
    }
    // 位置已在创建窗口时一次性设定（隐藏窗口的 set_position 会被系统丢弃），
    // 这里只设尺寸并显示，首帧即居中、零跳动。
    let _ = island.set_size(Size::Logical(LogicalSize::new(width, height)));
    if !should_show_capsule(&app) {
        return Ok(());
    }
    island.show().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn export_tasks_to_path(
    state: State<'_, AppState>,
    path: String,
    format: String,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.export_tasks(Path::new(&path), &format)
}

#[tauri::command]
fn import_tasks_from_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<usize, String> {
    let mut store = state.store.lock().map_err(|e| e.to_string())?;
    let count = store.import_tasks(Path::new(&path))?;
    drop(store);
    notify_tasks_changed(&app)?;
    Ok(count)
}

#[tauri::command]
fn get_task_counts(state: State<'_, AppState>) -> Result<TaskCounts, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(store.task_counts())
}

#[tauri::command]
fn list_incomplete_tasks(state: State<'_, AppState>, default_focus_minutes: i32) -> Result<Vec<TaskSummary>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let marked = state.marked_complete.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now();
    Ok(store
        .incomplete_tasks()
        .into_iter()
        .map(|t| task_summary_from(&t, &store, now, marked.contains(&t.id), default_focus_minutes))
        .collect())
}

#[tauri::command]
fn get_island_snapshot(state: State<'_, AppState>, default_focus_minutes: i32) -> Result<IslandSnapshot, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let marked = state.marked_complete.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now();
    Ok(IslandSnapshot {
        focus_counts: store.focus_priority_counts(),
        menu_bar_title: store.menu_bar_title(),
        attention_task: store
            .focus_attention_task()
            .as_ref()
            .map(|t| task_summary_from(t, &store, now, marked.contains(&t.id), default_focus_minutes)),
        preview_tasks: store
            .preview_tasks(3)
            .into_iter()
            .map(|t| task_summary_from(&t, &store, now, marked.contains(&t.id), default_focus_minutes))
            .collect(),
        expanded_height: store.expanded_island_height(),
        has_active_focus: store.active_focus_task().is_some(),
        incomplete_count: store.incomplete_count(),
    })
}

#[tauri::command]
fn quick_add_task(app: AppHandle, state: State<'_, AppState>, text: String) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store
            .add_task(&text, "", models::TaskPriority::Medium)?
            .ok_or_else(|| "任务标题不能为空".to_string())?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn get_menu_bar_title(state: State<'_, AppState>) -> Result<String, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(store.menu_bar_title())
}

#[tauri::command]
fn complete_task(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.complete(task_id, chrono::Utc::now())?;
    }
    state
        .marked_complete
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&task_id);
    reminder::clear_fired_task(&state.reminders, task_id);
    notify_tasks_changed(&app)
}

#[tauri::command]
fn reopen_task(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.reopen(task_id, chrono::Utc::now())?;
    }
    state
        .marked_complete
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&task_id);
    notify_tasks_changed(&app)
}

#[tauri::command]
fn set_current_task(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.set_current(task_id)?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn delete_task(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.delete(task_id)?;
    }
    state
        .marked_complete
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&task_id);
    reminder::clear_fired_task(&state.reminders, task_id);
    notify_tasks_changed(&app)
}

#[tauri::command]
fn toggle_task_mark(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        if !store.incomplete_tasks().iter().any(|task| task.id == task_id) {
            return Ok(());
        }
    }
    {
        let mut marked = state.marked_complete.lock().map_err(|e| e.to_string())?;
        if !marked.insert(task_id) {
            marked.remove(&task_id);
        }
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn get_panel_snapshot(state: State<'_, AppState>) -> Result<panel::PanelSnapshot, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let marked = state.marked_complete.lock().map_err(|e| e.to_string())?;
    Ok(panel::build_panel_snapshot(&store, &marked))
}

#[tauri::command]
fn search_tasks(state: State<'_, AppState>, query: String) -> Result<panel::SearchTasksResult, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let marked = state.marked_complete.lock().map_err(|e| e.to_string())?;
    Ok(panel::search_tasks(&store, &query, &marked))
}

#[tauri::command]
fn query_history(
    state: State<'_, AppState>,
    query: Option<String>,
    start_at: Option<String>,
    end_at: Option<String>,
) -> Result<Vec<panel::TaskDetail>, String> {
    let start = parse_opt_datetime(start_at)?;
    let end = parse_opt_datetime(end_at)?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(panel::query_history(
        &store,
        query.as_deref().unwrap_or(""),
        start,
        end,
    ))
}

#[tauri::command]
fn update_task_title(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.update_title(task_id, &title)?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn update_task_notes(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    notes: String,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.update_notes(task_id, &notes)?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn set_task_priority(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    priority: i32,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.set_priority(task_id, panel::priority_from_raw(priority))?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn toggle_today_queue(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        let in_queue = store
            .incomplete_tasks()
            .into_iter()
            .find(|t| t.id == task_id)
            .map(|t| t.is_in_today_queue())
            .ok_or_else(|| "task not found".to_string())?;
        store.set_today_queue(task_id, !in_queue)?;
    }
    notify_tasks_changed(&app)
}

/// 把可选的 RFC3339 字符串解析为 UTC 时间（空/None 视为清除）
fn parse_opt_datetime(value: Option<String>) -> Result<Option<chrono::DateTime<chrono::Utc>>, String> {
    match value {
        Some(s) if !s.trim().is_empty() => {
            let dt = chrono::DateTime::parse_from_rfc3339(s.trim()).map_err(|e| e.to_string())?;
            Ok(Some(dt.with_timezone(&chrono::Utc)))
        }
        _ => Ok(None),
    }
}

#[tauri::command]
fn set_task_project(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    project_name: Option<String>,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.set_project_name(task_id, project_name)?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn set_task_tags(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.set_tags(task_id, tags)?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn set_task_estimated_minutes(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    minutes: Option<i32>,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.set_estimated_minutes(task_id, minutes)?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn set_task_repeat_rule(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    rule: Option<String>,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let parsed = match rule {
        Some(s) if !s.trim().is_empty() => {
            Some(models::TaskRepeatRule::from_str(&s).ok_or("invalid repeat rule")?)
        }
        _ => None,
    };
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.set_repeat_rule(task_id, parsed)?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn set_task_due_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    due_at: Option<String>,
    reminder_at: Option<String>,
) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let due = parse_opt_datetime(due_at)?;
    let rem = parse_opt_datetime(reminder_at)?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.set_due_reminder(task_id, due, rem)?;
    }
    reminder::clear_fired_task(&state.reminders, task_id);
    notify_tasks_changed(&app)
}

#[tauri::command]
fn start_focus(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.start_focus(task_id, chrono::Utc::now())?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn pause_focus(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.pause_focus(task_id, chrono::Utc::now())?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn stop_focus(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.stop_focus(task_id, chrono::Utc::now())?;
    }
    notify_tasks_changed(&app)
}

#[tauri::command]
fn close_focus(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let task_id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store.close_focus(task_id, chrono::Utc::now())?;
    }
    notify_tasks_changed(&app)
}

/// 取已存在的 panel 窗口；不存在则按需创建（隐藏态 + 透明背景）。
/// 启动时不在 tauri.conf 预建 panel/quickadd，避免它们的 WebView 初始化
/// 在屏幕上闪出透明空壳（启动「一闪一闪」的根因之一）。仅首次打开时才创建，之后复用。
/// 必须在主线程调用：懒创建 panel WebView。
fn ensure_panel_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(win) = app.get_webview_window("panel") {
        return Ok(win);
    }
    let win = WebviewWindowBuilder::new(app, "panel", WebviewUrl::App("index.html".into()))
        .title("TaskCap")
        .inner_size(430.0, 590.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    let _ = win.set_background_color(Some(tauri::webview::Color(0, 0, 0, 0)));
    Ok(win)
}

/// 取已存在的 quickadd 窗口；不存在则按需创建（隐藏态 + 透明背景）。
/// 必须在主线程调用：懒创建 quickadd WebView。
fn ensure_quickadd_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(win) = app.get_webview_window("quickadd") {
        return Ok(win);
    }
    let win = WebviewWindowBuilder::new(app, "quickadd", WebviewUrl::App("index.html".into()))
        .title("快速新增")
        .inner_size(440.0, 320.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    let _ = win.set_background_color(Some(tauri::webview::Color(0, 0, 0, 0)));
    // 宽屏显示模式下快速新增窗口更宽，创建时按当前配置落地，避免首开尺寸不对
    let wide = app
        .state::<AppState>()
        .config
        .lock()
        .map(|c| c.display_mode == "wide")
        .unwrap_or(false);
    let width = if wide { 500.0 } else { 440.0 };
    let _ = win.set_size(Size::Logical(LogicalSize::new(width, 320.0)));
    Ok(win)
}

/// 后台预创建主面板（隐藏），挂载前端后自动拉数并按时刷新。
fn preload_panel_background(app: &AppHandle) {
    if is_panel_preload_paused() {
        return;
    }
    let enabled = app
        .state::<AppState>()
        .config
        .lock()
        .map(|c| c.panel_background_preload)
        .unwrap_or(true);
    if !enabled {
        return;
    }
    if ensure_panel_window(app).is_ok() {
        let app_for_event = app.clone();
        std::thread::spawn(move || {
            // 给隐藏 WebView 留出挂载监听的时间；前端仍有 3 秒兜底刷新。
            std::thread::sleep(Duration::from_millis(800));
            let _ = app_for_event.emit_to("panel", "panel-refresh-requested", ());
        });
    }
}

/// 后台预创建快速新增窗口，避免用户第一次从灵动岛点「+」时现场创建 WebView 卡住岛。
fn preload_quickadd_background(app: &AppHandle) {
    if QUICKADD_FRONTEND_READY.load(Ordering::Relaxed) {
        return;
    }
    let _ = ensure_quickadd_window(app);
}

fn start_quickadd_background_preload(app: AppHandle) {
    if QUICKADD_PRELOAD_STARTED.swap(true, Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(move || {
        // 等岛首帧就绪后再预热 quickadd，避免启动首屏与岛抢主线程。
        for _ in 0..100 {
            if ISLAND_READY_RECEIVED.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // 留出短暂可交互窗口；用户急点岛时 postpone 会继续顺延。
        std::thread::sleep(Duration::from_millis(1200));
        wait_until_safe_for_panel_preload();
        let app_handle = app.clone();
        let app_for_preload = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            preload_quickadd_background(&app_for_preload);
        });
    });
}

fn start_panel_background_preload(app: AppHandle) {
    std::thread::spawn(move || {
        // 等灵动岛首帧就绪后再预加载，避免 WebView 创建阻塞岛的 invoke/resize。
        for _ in 0..100 {
            if ISLAND_READY_RECEIVED.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // 岛就绪后 3 秒：空闲用户此时后台预加载；急点岛则 postpone + resize 冷却顺延
        std::thread::sleep(Duration::from_secs(3));
        wait_until_safe_for_panel_preload();
        let app_handle = app.clone();
        let app_for_preload = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            preload_panel_background(&app_for_preload);
        });
    });
}

fn toggle_panel_window_on_main(app: &AppHandle) -> Result<(), String> {
    let panel = ensure_panel_window(app)?;
    if panel.is_visible().map_err(|e| e.to_string())? {
        // 关闭前记一次当前位置，双保险（前端拖动已实时保存）
        if let Ok(pos) = panel.outer_position() {
            persist_panel_position(app, pos.x, pos.y);
        }
        panel.hide().map_err(|e| e.to_string())?;
    } else {
        position_panel_restore_or_default(app, &panel)?;
        panel.show().map_err(|e| e.to_string())?;
        panel.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn toggle_panel_window(app: AppHandle) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = toggle_panel_window_on_main(&app_for_main);
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.recv()
        .map_err(|_| "panel toggle channel closed".to_string())?
}

fn schedule_quickadd_input_focus(app: &AppHandle, quickadd: &WebviewWindow) {
    let delays_ms = [0_u64, 60, 140, 280, 520, 900, 1500, 2400];
    for delay_ms in delays_ms {
        let app_for_thread = app.clone();
        let label = quickadd.label().to_string();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(delay_ms));
            let app_for_main = app_for_thread.clone();
            let label_for_main = label.clone();
            let _ = app_for_thread.run_on_main_thread(move || {
                if let Some(win) = app_for_main.get_webview_window(&label_for_main) {
                    let _ = win.set_focus();
                    let _ = win.eval(QUICKADD_FOCUS_SCRIPT);
                }
            });
        });
    }
}

#[tauri::command]
fn quickadd_frontend_ready(app: AppHandle) -> Result<(), String> {
    QUICKADD_FRONTEND_READY.store(true, Ordering::Relaxed);
    let _ = app.emit_to("quickadd", "quickadd-focus-input", ());
    Ok(())
}

fn show_quickadd_window_on_main(app: &AppHandle) -> Result<(), String> {
    postpone_panel_preload_millis(30_000);
    let quickadd = ensure_quickadd_window(app)?;
    quickadd.center().map_err(|e| e.to_string())?;
    quickadd.show().map_err(|e| e.to_string())?;
    quickadd.set_focus().map_err(|e| e.to_string())?;
    let _ = app.emit_to("quickadd", "quickadd-opened", ());
    schedule_quickadd_input_focus(app, &quickadd);
    Ok(())
}

/// 快速新增：与 toggle_panel 一样同步等待主线程完成，避免 fire-and-forget 排在 panel 预加载后面无响应。
fn show_quickadd_window(app: AppHandle) -> Result<(), String> {
    postpone_panel_preload_millis(30_000);
    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
    let app_for_main = app.clone();
    app.run_on_main_thread(move || {
        let result = show_quickadd_window_on_main(&app_for_main);
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    rx.recv_timeout(Duration::from_secs(20))
        .map_err(|_| "quickadd open timed out".to_string())?
}

#[tauri::command]
fn toggle_panel(app: AppHandle) -> Result<(), String> {
    toggle_panel_window(app)
}

#[tauri::command]
fn show_quickadd(app: AppHandle) -> Result<(), String> {
    show_quickadd_window(app)
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn apply_display_mode(app: AppHandle, mode: String) -> Result<(), String> {
    let quickadd_w = if mode == "wide" { 500_f64 } else { 440_f64 };
    if let Some(win) = app.get_webview_window("quickadd") {
        let _ = win.set_size(tauri::LogicalSize::new(quickadd_w, 320_f64));
    }
    Ok(())
}

fn position_island(window: &WebviewWindow, config: &AppConfig) -> Result<(), String> {
    let monitor = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("primary monitor not found")?;
    let work = monitor.work_area();
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let center_x = work.position.x + ((work.size.width as i32 - size.width as i32) / 2);
    let x = center_x + config.island_offset_x;
    let y = work.position.y + config.island_offset_y;
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn reposition_island(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|e| e.to_string())?;
    if let Some(island) = app.get_webview_window("island") {
        position_island(&island, &config)?;
    }
    Ok(())
}

fn position_panel_bottom_right(window: &WebviewWindow) -> Result<(), String> {
    let monitor = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("primary monitor not found")?;
    let work = monitor.work_area();
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let scale = monitor.scale_factor();
    let margin = (20.0 * scale) as i32;
    let x = work.position.x + work.size.width as i32 - size.width as i32 - margin;
    let y = work.position.y + work.size.height as i32 - size.height as i32 - margin;
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 打开面板时定位：有记录位置则恢复（夹紧到工作区，防分辨率变化跑屏外），否则首次默认右下角。
fn position_panel_restore_or_default(
    app: &AppHandle,
    window: &WebviewWindow,
) -> Result<(), String> {
    let saved = {
        let state = app.state::<AppState>();
        let config = state.config.lock().map_err(|e| e.to_string())?;
        config.panel_pos_x.zip(config.panel_pos_y)
    };
    let Some((sx, sy)) = saved else {
        return position_panel_bottom_right(window);
    };
    let monitor = window
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("primary monitor not found")?;
    let work = monitor.work_area();
    let size = window.outer_size().map_err(|e| e.to_string())?;
    let max_x = work.position.x + work.size.width as i32 - size.width as i32;
    let max_y = work.position.y + work.size.height as i32 - size.height as i32;
    let x = sx.clamp(work.position.x, max_x.max(work.position.x));
    let y = sy.clamp(work.position.y, max_y.max(work.position.y));
    window
        .set_position(Position::Physical(PhysicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_shortcut_code(key: &str) -> Option<Code> {
    match key.to_lowercase().as_str() {
        "a" => Some(Code::KeyA), "b" => Some(Code::KeyB), "c" => Some(Code::KeyC),
        "d" => Some(Code::KeyD), "e" => Some(Code::KeyE), "f" => Some(Code::KeyF),
        "g" => Some(Code::KeyG), "h" => Some(Code::KeyH), "i" => Some(Code::KeyI),
        "j" => Some(Code::KeyJ), "k" => Some(Code::KeyK), "l" => Some(Code::KeyL),
        "m" => Some(Code::KeyM), "n" => Some(Code::KeyN), "o" => Some(Code::KeyO),
        "p" => Some(Code::KeyP), "q" => Some(Code::KeyQ), "r" => Some(Code::KeyR),
        "s" => Some(Code::KeyS), "t" => Some(Code::KeyT), "u" => Some(Code::KeyU),
        "v" => Some(Code::KeyV), "w" => Some(Code::KeyW), "x" => Some(Code::KeyX),
        "y" => Some(Code::KeyY), "z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0), "1" => Some(Code::Digit1), "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3), "4" => Some(Code::Digit4), "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6), "7" => Some(Code::Digit7), "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        _ => None,
    }
}

pub fn parse_shortcut_string(s: &str) -> Result<Shortcut, String> {
    let mut mods = Modifiers::empty();
    let mut key_code: Option<Code> = None;
    for part in s.split('+').map(|p| p.trim()) {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "alt" => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            k => { key_code = parse_shortcut_code(k); }
        }
    }
    key_code
        .map(|c| Shortcut::new(if mods.is_empty() { None } else { Some(mods) }, c))
        .ok_or_else(|| format!("无法解析快捷键: {s}"))
}

fn register_quick_add_shortcut(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let shortcut_str = state
        .config
        .lock()
        .map_err(|e| e.to_string())?
        .quick_add_shortcut
        .clone();
    let shortcut = parse_shortcut_string(&shortcut_str)?;
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                let _ = show_quickadd_window(handle.clone());
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn update_shortcut(app: AppHandle, state: State<'_, AppState>, shortcut: String) -> Result<(), String> {
    let new_shortcut = parse_shortcut_string(&shortcut)?;
    app.global_shortcut().unregister_all().map_err(|e| e.to_string())?;
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(new_shortcut, move |_app, _sc, event| {
            if event.state == ShortcutState::Pressed {
                let _ = show_quickadd_window(handle.clone());
            }
        })
        .map_err(|e| e.to_string())?;
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.quick_add_shortcut = shortcut;
    config::save_config(&config)
}

fn handle_startup_deep_links(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    for url in deeplink::collect_startup_urls() {
        deeplink::handle_deep_link(
            app,
            &state.store,
            &url,
            toggle_panel_window,
            show_quickadd_window,
        )?;
        let _ = notify_tasks_changed(app);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let store = TaskStore::new_file().expect("failed to init task store");
    let mut app_config = config::load_config();
    config::sync_autostart_flag(&mut app_config);

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let state = app.state::<AppState>();
            for arg in args {
                if arg.starts_with("taskcap://") {
                    let _ = deeplink::handle_deep_link(
                        app,
                        &state.store,
                        &arg,
                        toggle_panel_window,
                        show_quickadd_window,
                    );
                    let _ = notify_tasks_changed(app);
                }
            }
            if should_show_capsule(&app) {
                if let Some(island) = app.get_webview_window("island") {
                    let _ = island.show();
                }
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            store: Mutex::new(store),
            config: Mutex::new(app_config),
            reminders: ReminderState::new(),
            marked_complete: Mutex::new(HashSet::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_app_config,
            save_app_config,
            get_task_counts,
            get_menu_bar_title,
            get_island_snapshot,
            get_panel_snapshot,
            search_tasks,
            query_history,
            list_incomplete_tasks,
            quick_add_task,
            complete_task,
            reopen_task,
            set_current_task,
            delete_task,
            toggle_task_mark,
            update_task_title,
            update_task_notes,
            set_task_priority,
            set_task_project,
            set_task_tags,
            set_task_estimated_minutes,
            set_task_repeat_rule,
            set_task_due_reminder,
            toggle_today_queue,
            start_focus,
            pause_focus,
            stop_focus,
            close_focus,
            toggle_panel,
            show_quickadd,
            quickadd_frontend_ready,
            save_island_position,
            save_panel_position,
            resize_island_window,
            postpone_panel_preload,
            island_ready,
            export_tasks_to_path,
            import_tasks_from_path,
            quit_app,
            update_shortcut,
            apply_display_mode
        ])
        .setup(|app| {
            // release 版由 NSIS installer 写注册表；dev 模式本地注册方便测试
            #[cfg(debug_assertions)]
            let _ = deeplink::register_protocol();
            let _ = app.handle().notification().request_permission();

            let show_panel = MenuItem::with_id(app, "show_panel", "显示任务面板", true, None::<&str>)?;
            let quick_add = MenuItem::with_id(app, "quick_add", "快速新增", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_panel, &quick_add, &quit])?;

            let _tray = TrayIconBuilder::with_id(tray_util::TRAY_ID)
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("TaskCap")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show_panel" => {
                        let _ = toggle_panel_window(app.clone());
                    }
                    "quick_add" => {
                        let _ = show_quickadd_window(app.clone());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        let _ = toggle_panel_window(app.clone());
                    }
                })
                .build(app)?;

            // 动态创建灵动岛：不在 tauri.conf 预建。隐藏 + 刚创建的窗口 set_position
            // 会被系统丢弃（Win32 实测：岛会停在默认级联位置导致偏左），因此必须在
            // 创建窗口的那一刻就带上居中位置（WebviewWindowBuilder.position），实现零跳居中。
            {
                let (offset_x, offset_y) = {
                    let state = app.state::<AppState>();
                    let cfg = state.config.lock().unwrap();
                    (cfg.island_offset_x, cfg.island_offset_y)
                };
                let (ix, iy) = match app.primary_monitor() {
                    Ok(Some(monitor)) => {
                        let work = monitor.work_area();
                        let scale = monitor.scale_factor();
                        let phys_w = (172.0 * scale).round() as i32;
                        let px = work.position.x + (work.size.width as i32 - phys_w) / 2 + offset_x;
                        let py = work.position.y + offset_y;
                        (px as f64 / scale, py as f64 / scale)
                    }
                    _ => (700.0, 8.0),
                };
                let built = WebviewWindowBuilder::new(
                    app,
                    "island",
                    WebviewUrl::App("index.html".into()),
                )
                .title("TaskCap")
                .inner_size(172.0, 30.0)
                .position(ix, iy)
                .resizable(true)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible(false)
                .build();
                // 把 WebView2 默认背景设为全透明，消除首次 show 时 controller 层露白
                if let Ok(island) = built {
                    let _ = island.set_background_color(Some(tauri::webview::Color(0, 0, 0, 0)));
                }
            }

            // 兜底：仅当前端迟迟未触发 island_ready 时才强制 show，避免打包版抢在首帧前闪透明边框。
            let app_for_fallback = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(5000));
                if ISLAND_READY_RECEIVED.load(Ordering::Relaxed) {
                    return;
                }
                if should_show_capsule(&app_for_fallback) {
                    if let Some(island) = app_for_fallback.get_webview_window("island") {
                        if !island.is_visible().unwrap_or(false) {
                            let _ = island.set_size(Size::Logical(LogicalSize::new(172.0, 30.0)));
                            let _ = island.show();
                        }
                    }
                }
            });

            register_quick_add_shortcut(app.handle())?;
            start_quickadd_background_preload(app.handle().clone());
            start_panel_background_preload(app.handle().clone());
            // 启动只刷新托盘；灵动岛首次显示交给前端 island_ready，避免 WebView 未绘制就 show。
            let _ = refresh_tray_ui(app.handle());
            let _ = handle_startup_deep_links(app.handle());
            reminder::start_reminder_loop(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
