//! 飞书多维表格同步。
//!
//! 数据落在飞书「★每日工作日程表 / TaskCap任务」：
//! 1. 拉取：飞书里把「状态」改成「已完成」→ 本地任务标记完成；
//! 2. 推送：本地新增/修改的任务 → 按「本地ID」去重后批量写入飞书。
//!
//! 凭证和同步状态都写在 `%APPDATA%\taskcap\` 下，不进入仓库。

use crate::models::TaskItem;
use crate::AppState;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

const API: &str = "https://open.feishu.cn/open-apis";
/// 同步目标表统一用这个名字，建表时优先复用同名表
const TABLE_NAME: &str = "TaskCap任务";
const DEFAULT_AUTO_SYNC_MINUTES: u32 = 15;
/// 飞书批量接口单次上限 100 条
const MAX_RECORD_CHUNK: usize = 100;

fn default_auto_sync_minutes() -> u32 {
    DEFAULT_AUTO_SYNC_MINUTES
}

/// 飞书同步配置（feishu.json），含应用凭证，只存在本机。
///
/// 不预置任何租户信息：不填 = 不启用，完全离线可用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub app_secret: String,
    /// 多维表格 base token，用户在飞书里建好表格后自己填
    #[serde(default)]
    pub app_token: String,
    /// 数据表 ID，可用「创建任务表」一键生成
    #[serde(default)]
    pub table_id: String,
    #[serde(default = "default_auto_sync_minutes")]
    pub auto_sync_minutes: u32,
    #[serde(default)]
    pub last_sync_at: Option<String>,
    #[serde(default)]
    pub last_sync_summary: Option<String>,
}

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            app_id: String::new(),
            app_secret: String::new(),
            app_token: String::new(),
            table_id: String::new(),
            auto_sync_minutes: default_auto_sync_minutes(),
            last_sync_at: None,
            last_sync_summary: None,
        }
    }
}

/// 前端可读的同步状态（不返回 app_secret）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuState {
    pub enabled: bool,
    pub app_id: String,
    pub app_token: String,
    pub table_id: String,
    pub auto_sync_minutes: u32,
    pub has_secret: bool,
    pub last_sync_at: Option<String>,
    pub last_sync_summary: Option<String>,
    pub syncing: bool,
    pub last_error: Option<String>,
}

/// 设置页提交的增量配置；appSecret 留空表示沿用旧值
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfigPatch {
    pub enabled: Option<bool>,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
    pub app_token: Option<String>,
    pub table_id: Option<String>,
    pub auto_sync_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuSyncReport {
    pub total: usize,
    pub created: usize,
    pub updated: usize,
    pub pulled: usize,
    pub skipped: usize,
    pub duration_ms: u64,
}

impl FeishuSyncReport {
    fn summary(&self) -> String {
        format!(
            "本地 {} 条，新增 {}，更新 {}，回写完成 {}，跳过 {}，耗时 {}ms",
            self.total, self.created, self.updated, self.pulled, self.skipped, self.duration_ms
        )
    }
}

/// 运行期状态：是否正在同步、最近一次错误
pub struct FeishuRuntime {
    pub syncing: AtomicBool,
    pub last_error: Mutex<Option<String>>,
}

impl FeishuRuntime {
    pub fn new() -> Self {
        Self {
            syncing: AtomicBool::new(false),
            last_error: Mutex::new(None),
        }
    }
}

impl Default for FeishuRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// 推送去重用的本地状态：本地任务 ID → 飞书 record_id + 上次推送的 updated_at
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncEntry {
    record_id: String,
    pushed_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncState {
    #[serde(default = "sync_state_version")]
    version: u32,
    #[serde(default)]
    records: HashMap<String, SyncEntry>,
}

fn sync_state_version() -> u32 {
    1
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            version: sync_state_version(),
            records: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------- 文件位置

fn data_dir() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA").ok_or("APPDATA is not available")?;
    Ok(PathBuf::from(appdata).join("taskcap"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("feishu.json"))
}

fn sync_state_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("feishu-sync.json"))
}

fn log_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("feishu.log"))
}

pub fn load_config() -> FeishuConfig {
    let Ok(path) = config_path() else {
        return FeishuConfig::default();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return FeishuConfig::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_config(config: &FeishuConfig) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

fn load_sync_state() -> SyncState {
    let Ok(path) = sync_state_path() else {
        return SyncState::default();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_sync_state(state: &SyncState) {
    let Ok(path) = sync_state_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(raw) = serde_json::to_string_pretty(state) {
        let _ = fs::write(path, raw);
    }
}

/// 同步日志：写入 %APPDATA%\taskcap\feishu.log，便于排查
fn log_line(message: &str) {
    let Ok(path) = log_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), message);
    }
}

// ---------------------------------------------------------------- 对外命令实现

pub fn state(app: &AppHandle) -> FeishuState {
    let config = load_config();
    let runtime = app.state::<AppState>();
    let syncing = runtime.feishu.syncing.load(Ordering::SeqCst);
    let last_error = runtime
        .feishu
        .last_error
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    FeishuState {
        enabled: config.enabled,
        app_id: config.app_id,
        app_token: config.app_token,
        table_id: config.table_id,
        auto_sync_minutes: config.auto_sync_minutes,
        has_secret: !config.app_secret.trim().is_empty(),
        last_sync_at: config.last_sync_at,
        last_sync_summary: config.last_sync_summary,
        syncing,
        last_error,
    }
}

/// 保存设置页提交的增量配置；返回最新配置供调用方组装状态
pub fn save_patch(patch: FeishuConfigPatch) -> Result<FeishuConfig, String> {
    let mut config = load_config();
    if let Some(enabled) = patch.enabled {
        config.enabled = enabled;
    }
    if let Some(app_id) = patch.app_id {
        config.app_id = app_id.trim().to_string();
    }
    if let Some(secret) = patch.app_secret {
        // 空字符串表示「不改」，避免前端回填时把密钥清掉
        if !secret.trim().is_empty() {
            config.app_secret = secret.trim().to_string();
        }
    }
    if let Some(app_token) = patch.app_token {
        config.app_token = app_token.trim().to_string();
    }
    if let Some(table_id) = patch.table_id {
        config.table_id = table_id.trim().to_string();
    }
    if let Some(minutes) = patch.auto_sync_minutes {
        config.auto_sync_minutes = minutes.clamp(5, 240);
    }
    save_config(&config)?;
    log_line(&format!(
        "config saved enabled={} table={} interval={}m",
        config.enabled, config.table_id, config.auto_sync_minutes
    ));
    Ok(config)
}

/// 创建/复用任务表的结果：状态 + 一句给人看的说明
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuTableOutcome {
    pub state: FeishuState,
    pub message: String,
}

/// 连接自检结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConnectionInfo {
    pub app_token: String,
    pub table_id: String,
    pub table_name: Option<String>,
    pub field_count: usize,
    pub message: String,
}

/// 在指定多维表格里创建「TaskCap任务」数据表；已有同名表则直接复用。
///
/// 用户只填 App Token 就能一键建表，不用手抄 Table ID。
/// 复用同名表时会把真实 Table ID 回写配置，所以粘贴错的 Table ID 也会被纠正。
pub fn create_table(app: &AppHandle, app_token: &str) -> Result<FeishuTableOutcome, String> {
    let mut config = load_config();
    if !app_token.trim().is_empty() {
        config.app_token = app_token.trim().to_string();
    }
    if config.app_id.trim().is_empty() || config.app_secret.trim().is_empty() {
        return Err("请先填写 App ID 与 App Secret".to_string());
    }
    if config.app_token.trim().is_empty() {
        return Err("请先填写 App Token（链接 /base/ 或 /wiki/ 后面那段）".to_string());
    }

    let token = tenant_token(&config)?;
    // 知识库链接（/wiki/）里的 token 是节点 token，换成底层表格的 app_token
    let resolved = resolve_app_token(&config.app_token, &token);
    if resolved != config.app_token {
        log_line(&format!(
            "app_token resolved {} -> {}",
            config.app_token, resolved
        ));
        config.app_token = resolved;
    }

    // 先查同名表，避免重复创建
    let list_path = format!("/bitable/v1/apps/{}/tables?page_size=100", config.app_token);
    let existing = api(&token, "GET", &list_path, None)?;
    if let Some(items) = existing.pointer("/data/items").and_then(|v| v.as_array()) {
        if let Some(table_id) = items
            .iter()
            .find(|item| item.get("name").and_then(|name| name.as_str()) == Some(TABLE_NAME))
            .and_then(|item| item.get("table_id"))
            .and_then(|value| value.as_str())
        {
            let replaced = !config.table_id.trim().is_empty() && config.table_id != table_id;
            config.table_id = table_id.to_string();
            save_config(&config)?;
            log_line(&format!("table reused {table_id}"));
            let message = if replaced {
                format!("已复用同名表 {table_id}，并覆盖了原来填错的 Table ID")
            } else {
                format!("已复用同名表 {table_id}")
            };
            return Ok(FeishuTableOutcome {
                state: state(app),
                message,
            });
        }
    }

    let body = json!({
        "table": {
            "name": TABLE_NAME,
            "default_view_name": "全部任务",
            "fields": table_fields_spec(),
        }
    });
    let create_path = format!("/bitable/v1/apps/{}/tables", config.app_token);
    let created = api(&token, "POST", &create_path, Some(body))?;
    let table_id = created
        .pointer("/data/table_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "飞书没有返回 table_id".to_string())?
        .to_string();
    config.table_id = table_id.clone();
    save_config(&config)?;
    log_line(&format!("table created {table_id}"));
    Ok(FeishuTableOutcome {
        state: state(app),
        message: format!("已创建新表 {table_id}"),
    })
}

/// 连接自检：验证凭证、解析 token、确认表存在并读字段。
///
/// 不写任何数据，只回答「现在这个配置能不能用」。
pub fn test_connection() -> Result<FeishuConnectionInfo, String> {
    let config = load_config();
    if config.app_id.trim().is_empty() || config.app_secret.trim().is_empty() {
        return Err("还没填 App ID / App Secret".to_string());
    }
    if config.app_token.trim().is_empty() {
        return Err("还没填 App Token".to_string());
    }

    let token = tenant_token(&config)?;
    let resolved = resolve_app_token(&config.app_token, &token);
    let list_path = format!("/bitable/v1/apps/{resolved}/tables?page_size=100");
    let value = api(&token, "GET", &list_path, None)?;
    let items = value
        .pointer("/data/items")
        .and_then(|items| items.as_array())
        .cloned()
        .unwrap_or_default();

    let wanted = config.table_id.trim();
    let mut note = String::new();
    let matched = items
        .iter()
        .find(|item| {
            !wanted.is_empty() && item.get("table_id").and_then(|v| v.as_str()) == Some(wanted)
        })
        .cloned()
        .or_else(|| {
            items
                .iter()
                .find(|item| item.get("name").and_then(|v| v.as_str()) == Some(TABLE_NAME))
                .cloned()
                .map(|item| {
                    note = if wanted.is_empty() {
                        "还没有 Table ID，找到同名表".to_string()
                    } else {
                        format!("Table ID {wanted} 不在这个表格里，已找到同名表")
                    };
                    item
                })
        })
        .ok_or_else(|| {
            if wanted.is_empty() {
                "这个多维表格里还没有 TaskCap任务 表，点「创建任务表」".to_string()
            } else {
                format!("Table ID {wanted} 不在这个多维表格里，点「创建任务表」会新建或复用")
            }
        })?;

    let table_id = matched
        .get("table_id")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let table_name = matched
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    let fields_path = format!("/bitable/v1/apps/{resolved}/tables/{table_id}/fields?page_size=100");
    let fields = api(&token, "GET", &fields_path, None)?;
    let field_count = fields
        .pointer("/data/items")
        .and_then(|items| items.as_array())
        .map(|items| items.len())
        .unwrap_or(0);

    let message = if note.is_empty() {
        format!("连接正常：{}（{field_count} 个字段）", table_name.as_deref().unwrap_or(TABLE_NAME))
    } else {
        format!("{note}：{table_id}（{field_count} 个字段）")
    };

    Ok(FeishuConnectionInfo {
        app_token: resolved,
        table_id,
        table_name,
        field_count,
        message,
    })
}
/// 建表用的字段定义，与 scripts/feishu/create_table.py 保持一致
fn table_fields_spec() -> Value {
    json!([
        {"field_name": "任务标题", "type": 1},
        {"field_name": "本地ID", "type": 1},
        {
            "field_name": "状态",
            "type": 3,
            "property": {"options": [
                {"name": "待办", "color": 0},
                {"name": "已完成", "color": 2}
            ]}
        },
        {
            "field_name": "优先级",
            "type": 3,
            "property": {"options": [
                {"name": "高", "color": 1},
                {"name": "中", "color": 3},
                {"name": "低", "color": 5}
            ]}
        },
        {"field_name": "项目", "type": 1},
        {"field_name": "标签", "type": 1},
        {"field_name": "截止日期", "type": 5, "property": {"date_formatter": "yyyy/MM/dd", "auto_fill": false}},
        {"field_name": "提醒时间", "type": 5, "property": {"date_formatter": "yyyy/MM/dd HH:mm", "auto_fill": false}},
        {"field_name": "完成时间", "type": 5, "property": {"date_formatter": "yyyy/MM/dd HH:mm", "auto_fill": false}},
        {"field_name": "创建时间", "type": 5, "property": {"date_formatter": "yyyy/MM/dd HH:mm", "auto_fill": false}},
        {"field_name": "更新时间", "type": 5, "property": {"date_formatter": "yyyy/MM/dd HH:mm", "auto_fill": false}},
        {"field_name": "预计分钟", "type": 2},
        {"field_name": "专注分钟", "type": 2},
        {"field_name": "推迟次数", "type": 2},
        {"field_name": "子任务", "type": 1},
        {"field_name": "备注", "type": 1},
        {"field_name": "重复规则", "type": 1},
        {"field_name": "今日清单", "type": 7},
        {"field_name": "当前任务", "type": 7},
        {"field_name": "同步时间", "type": 5, "property": {"date_formatter": "yyyy/MM/dd HH:mm", "auto_fill": false}}
    ])
}

pub fn sync_now(app: &AppHandle) -> Result<FeishuSyncReport, String> {
    let runtime = app.state::<AppState>();
    if runtime.feishu.syncing.swap(true, Ordering::SeqCst) {
        return Err("正在同步中，请稍候".to_string());
    }

    let result = sync_inner(app);
    runtime.feishu.syncing.store(false, Ordering::SeqCst);

    match &result {
        Ok(report) => {
            if let Ok(mut slot) = runtime.feishu.last_error.lock() {
                *slot = None;
            }
            let mut config = load_config();
            config.last_sync_at = Some(Utc::now().to_rfc3339());
            config.last_sync_summary = Some(report.summary());
            let _ = save_config(&config);
            log_line(&format!("sync ok {}", report.summary()));
        }
        Err(error) => {
            if let Ok(mut slot) = runtime.feishu.last_error.lock() {
                *slot = Some(error.clone());
            }
            log_line(&format!("sync fail {error}"));
        }
    }

    result
}

/// 配置是否齐全：缺任何一项都不发起网络请求
pub fn is_configured(config: &FeishuConfig) -> bool {
    !config.app_id.trim().is_empty()
        && !config.app_secret.trim().is_empty()
        && !config.app_token.trim().is_empty()
        && !config.table_id.trim().is_empty()
}

/// 本地任务有改动，希望尽快同步（由 notify_tasks_changed 触发）
static DIRTY: AtomicBool = AtomicBool::new(false);
/// 有改动时，距离上次同步至少间隔这么久才推送，避免频繁请求
const DIRTY_SYNC_COOLDOWN_SECS: u64 = 30;

/// 标记「有本地改动」，后台线程会在冷却后补一次同步
pub fn request_sync_soon() {
    DIRTY.store(true, Ordering::SeqCst);
}

/// 是否该跑一次同步：到期按间隔跑；有改动则在冷却后补跑
fn should_run_sync(config: &FeishuConfig, dirty: bool) -> bool {
    if !config.enabled || !is_configured(config) {
        return false;
    }
    if is_sync_due(config) {
        return true;
    }
    dirty && elapsed_since_last_sync(config) >= Duration::from_secs(DIRTY_SYNC_COOLDOWN_SECS)
}

fn elapsed_since_last_sync(config: &FeishuConfig) -> Duration {
    let Some(raw) = config.last_sync_at.as_deref() else {
        return Duration::from_secs(u64::MAX);
    };
    match DateTime::parse_from_rfc3339(raw) {
        Ok(dt) => {
            let seconds = Utc::now()
                .signed_duration_since(dt.with_timezone(&Utc))
                .num_seconds();
            Duration::from_secs(seconds.max(0) as u64)
        }
        Err(_) => Duration::from_secs(u64::MAX),
    }
}

/// 后台定时同步：启用且配置齐全时才联网；有改动会在冷却后尽快补同步
pub fn start_auto_sync(app: AppHandle) {
    std::thread::spawn(move || {
        // 首次多等一会儿，避开启动期灵动岛和主面板的抢资源
        std::thread::sleep(Duration::from_secs(10));
        loop {
            let config = load_config();
            let dirty = DIRTY.load(Ordering::SeqCst);
            let runtime = app.state::<AppState>();
            let busy = runtime.feishu.syncing.load(Ordering::SeqCst);
            if !busy && should_run_sync(&config, dirty) {
                DIRTY.store(false, Ordering::SeqCst);
                let _ = sync_now(&app);
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

fn is_sync_due(config: &FeishuConfig) -> bool {
    let Some(last) = config.last_sync_at.as_deref() else {
        return true;
    };
    match DateTime::parse_from_rfc3339(last) {
        Ok(dt) => {
            let elapsed = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
            elapsed >= ChronoDuration::minutes(config.auto_sync_minutes as i64)
        }
        Err(_) => true,
    }
}

// ---------------------------------------------------------------- 同步主体

fn sync_inner(app: &AppHandle) -> Result<FeishuSyncReport, String> {
    let started = Instant::now();
    let config = load_config();
    if config.app_id.trim().is_empty() || config.app_secret.trim().is_empty() {
        return Err("飞书未配置：请填写 App ID 与 App Secret".to_string());
    }
    if config.app_token.trim().is_empty() || config.table_id.trim().is_empty() {
        return Err("飞书未配置：缺少 App Token 或 Table ID".to_string());
    }

    let token = tenant_token(&config)?;
    let mut config = config;
    // 知识库链接（/wiki/）里的 token 是节点 token，换成底层表格的 app_token
    let resolved = resolve_app_token(&config.app_token, &token);
    if resolved != config.app_token {
        log_line(&format!(
            "app_token resolved {} -> {}",
            config.app_token, resolved
        ));
        config.app_token = resolved;
    }
    let remote = list_records(&config, &token)?;

    // 本地快照：网络请求期间不持有 store 锁
    let tasks = snapshot_tasks(app)?;

    // 本地ID → record_id
    let mut remote_map: HashMap<String, String> = HashMap::new();
    for record in &remote {
        let Some(fields) = record.get("fields") else {
            continue;
        };
        let local_id = text_value(fields.get("本地ID").unwrap_or(&Value::Null));
        let record_id = record
            .get("record_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if !local_id.trim().is_empty() && !record_id.is_empty() {
            remote_map.insert(local_id.trim().to_string(), record_id.to_string());
        }
    }

    // 1) 拉取：飞书「已完成」→ 本地完成（本地完成的不反向重开，避免误伤历史）
    let mut pulled_ids: Vec<Uuid> = Vec::new();
    for record in &remote {
        let Some(fields) = record.get("fields") else {
            continue;
        };
        let status = text_value(fields.get("状态").unwrap_or(&Value::Null));
        if status != "已完成" {
            continue;
        }
        let local_id = text_value(fields.get("本地ID").unwrap_or(&Value::Null));
        let Ok(task_id) = Uuid::parse_str(local_id.trim()) else {
            continue;
        };
        if let Some(task) = tasks.iter().find(|task| task.id == task_id) {
            if !task.is_completed {
                pulled_ids.push(task_id);
            }
        }
    }

    let mut pulled = 0usize;
    if !pulled_ids.is_empty() {
        let runtime = app.state::<AppState>();
        let mut store = runtime.store.lock().map_err(|e| e.to_string())?;
        for task_id in &pulled_ids {
            if store.complete(*task_id, Utc::now()).is_ok() {
                pulled += 1;
            }
        }
    }

    // 拉取后重新取快照，保证推给飞书的状态是最新的
    let tasks = snapshot_tasks(app)?;

    let mut sync_state = load_sync_state();
    let mut creates: Vec<(Value, String, String)> = Vec::new();
    let mut updates: Vec<(String, Value, String, String)> = Vec::new();
    let mut skipped = 0usize;

    for task in &tasks {
        let local_id = task.id.to_string();
        let updated_at = task.updated_at.to_rfc3339();
        match remote_map.get(&local_id) {
            Some(record_id) => {
                let unchanged = sync_state
                    .records
                    .get(&local_id)
                    .map(|entry| {
                        entry.record_id == *record_id && entry.pushed_updated_at == updated_at
                    })
                    .unwrap_or(false);
                if unchanged {
                    skipped += 1;
                    continue;
                }
                updates.push((
                    record_id.clone(),
                    task_fields(task, true),
                    local_id,
                    updated_at,
                ));
            }
            None => creates.push((task_fields(task, false), local_id, updated_at)),
        }
    }

    // 2) 推送新增
    let mut created = 0usize;
    for chunk in creates.chunks(MAX_RECORD_CHUNK) {
        let records: Vec<Value> = chunk
            .iter()
            .map(|(fields, _, _)| json!({ "fields": fields }))
            .collect();
        let response = api(
            &token,
            "POST",
            &records_path(&config, "batch_create"),
            Some(json!({ "records": records })),
        )?;
        let items = response
            .pointer("/data/records")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        for (index, (_, local_id, updated_at)) in chunk.iter().enumerate() {
            let Some(record_id) = items
                .get(index)
                .and_then(|record| record.get("record_id"))
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            sync_state.records.insert(
                local_id.clone(),
                SyncEntry {
                    record_id: record_id.to_string(),
                    pushed_updated_at: updated_at.clone(),
                },
            );
            created += 1;
        }
    }

    // 3) 推送更新
    let mut updated = 0usize;
    for chunk in updates.chunks(MAX_RECORD_CHUNK) {
        let records: Vec<Value> = chunk
            .iter()
            .map(|(record_id, fields, _, _)| json!({ "record_id": record_id, "fields": fields }))
            .collect();
        api(
            &token,
            "POST",
            &records_path(&config, "batch_update"),
            Some(json!({ "records": records })),
        )?;
        for (record_id, _, local_id, updated_at) in chunk {
            sync_state.records.insert(
                local_id.clone(),
                SyncEntry {
                    record_id: record_id.clone(),
                    pushed_updated_at: updated_at.clone(),
                },
            );
            updated += 1;
        }
    }

    save_sync_state(&sync_state);

    if pulled > 0 || created > 0 || updated > 0 {
        let _ = crate::notify_tasks_changed(app);
    }

    Ok(FeishuSyncReport {
        total: tasks.len(),
        created,
        updated,
        pulled,
        skipped,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn snapshot_tasks(app: &AppHandle) -> Result<Vec<TaskItem>, String> {
    let runtime = app.state::<AppState>();
    let store = runtime.store.lock().map_err(|e| e.to_string())?;
    Ok(store.tasks.clone())
}

// ---------------------------------------------------------------- 字段映射

/// 本地任务 → 飞书字段。
/// `for_update = true` 时空值写成 null，用来清掉飞书里的旧值；
/// 新建时省略空值，避免日期/数字字段收到 null 报错。
fn task_fields(task: &TaskItem, for_update: bool) -> Value {
    let mut fields = Map::new();
    fields.insert("任务标题".into(), json!(task.title));
    fields.insert("本地ID".into(), json!(task.id.to_string()));
    fields.insert(
        "状态".into(),
        json!(if task.is_completed { "已完成" } else { "待办" }),
    );
    fields.insert("优先级".into(), json!(task.priority().short_title()));
    fields.insert("备注".into(), json!(task.notes));
    fields.insert("标签".into(), json!(task.tags().join(",")));
    fields.insert("项目".into(), json!(task.project_name.clone().unwrap_or_default()));
    fields.insert(
        "重复规则".into(),
        json!(task.repeat_rule().map(|rule| rule.title()).unwrap_or("")),
    );

    let subtask_total = task.subtasks().len();
    fields.insert(
        "子任务".into(),
        json!(format!(
            "{}/{}",
            task.completed_subtask_count(),
            subtask_total
        )),
    );

    fields.insert("今日清单".into(), json!(task.is_in_today_queue()));
    fields.insert("当前任务".into(), json!(task.is_current));
    fields.insert("推迟次数".into(), json!(task.postpone_count()));
    fields.insert(
        "专注分钟".into(),
        json!((task.focus_seconds() / 60.0).round() as i64),
    );

    put_opt_number(&mut fields, "预计分钟", task.estimated_minutes.map(i64::from), for_update);

    put_opt_date(&mut fields, "截止日期", task.due_at, for_update);
    put_opt_date(&mut fields, "提醒时间", task.reminder_at, for_update);
    put_opt_date(&mut fields, "完成时间", task.completed_at, for_update);
    put_opt_date(&mut fields, "创建时间", Some(task.created_at), for_update);
    put_opt_date(&mut fields, "更新时间", Some(task.updated_at), for_update);
    put_opt_date(&mut fields, "同步时间", Some(Utc::now()), for_update);

    Value::Object(fields)
}

fn put_opt_date(
    fields: &mut Map<String, Value>,
    key: &str,
    value: Option<DateTime<Utc>>,
    for_update: bool,
) {
    match value {
        Some(dt) => {
            fields.insert(key.to_string(), json!(dt.timestamp_millis()));
        }
        None => {
            if for_update {
                fields.insert(key.to_string(), Value::Null);
            }
        }
    }
}

fn put_opt_number(fields: &mut Map<String, Value>, key: &str, value: Option<i64>, for_update: bool) {
    match value {
        Some(number) => {
            fields.insert(key.to_string(), json!(number));
        }
        None => {
            if for_update {
                fields.insert(key.to_string(), Value::Null);
            }
        }
    }
}

/// 把飞书返回的字段值压平成字符串（文本/单选/数字/嵌套文本通吃）
fn text_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Array(items) => items
            .iter()
            .map(text_value)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(map) => map
            .get("text")
            .or_else(|| map.get("name"))
            .map(text_value)
            .unwrap_or_default(),
    }
}

// ---------------------------------------------------------------- HTTP

fn api(
    token: &str,
    method: &str,
    path: &str,
    payload: Option<Value>,
) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(40))
        .build();
    let url = format!("{API}{path}");
    let request = match method {
        "GET" => agent.get(&url),
        "POST" => agent.post(&url),
        other => return Err(format!("不支持的请求方法：{other}")),
    };
    let mut request = request.set("Content-Type", "application/json; charset=utf-8");
    if !token.is_empty() {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    let response = match payload {
        Some(body) => request.send_json(body),
        None => request.call(),
    };
    let value = match response {
        Ok(resp) => resp
            .into_json::<Value>()
            .map_err(|e| format!("解析飞书响应失败：{e}"))?,
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            return Err(format!("飞书接口 HTTP {code}：{}", shorten(&body, 300)));
        }
        Err(error) => return Err(format!("飞书接口请求失败：{error}")),
    };
    let code = value.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        let message = value.get("msg").and_then(|m| m.as_str()).unwrap_or("");
        return Err(format!("飞书接口返回错误 {code}：{message}"));
    }
    Ok(value)
}

fn tenant_token(config: &FeishuConfig) -> Result<String, String> {
    let body = json!({ "app_id": config.app_id, "app_secret": config.app_secret });
    let value = api(
        "",
        "POST",
        "/auth/v3/tenant_access_token/internal",
        Some(body),
    )?;
    value
        .get("tenant_access_token")
        .and_then(|token| token.as_str())
        .map(str::to_string)
        .ok_or_else(|| "飞书没有返回 tenant_access_token".to_string())
}

/// 把知识库（wiki）链接里的节点 token 换成底层多维表格的 app_token。
///
/// 从飞书客户端分享出来的链接常是 `/wiki/<节点token>?table=...`，
/// 这个 token 不是表格自身的 app_token。这里做 best-effort 解析：
/// 解析失败（没开 wiki 权限、或本来就不是 wiki token）就原样返回。
fn resolve_app_token(app_token: &str, token: &str) -> String {
    let path = format!("/wiki/v2/spaces/get_node?token={app_token}&obj_type=wiki");
    let Ok(value) = api(token, "GET", &path, None) else {
        return app_token.to_string();
    };
    let node = value.pointer("/data/node");
    let obj_type = node
        .and_then(|node| node.get("obj_type"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let obj_token = node
        .and_then(|node| node.get("obj_token"))
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if obj_type == "bitable" && !obj_token.is_empty() {
        obj_token.to_string()
    } else {
        app_token.to_string()
    }
}

fn records_path(config: &FeishuConfig, action: &str) -> String {
    format!(
        "/bitable/v1/apps/{}/tables/{}/records/{action}",
        config.app_token, config.table_id
    )
}

fn list_records(config: &FeishuConfig, token: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut body = Map::new();
        body.insert("automatic_fields".into(), json!(false));
        if let Some(page_token) = &page_token {
            body.insert("page_token".into(), json!(page_token));
        }
        let path = format!(
            "/bitable/v1/apps/{}/tables/{}/records/search?page_size=500",
            config.app_token, config.table_id
        );
        let value = api(token, "POST", &path, Some(Value::Object(body)))?;
        if let Some(items) = value.pointer("/data/items").and_then(|v| v.as_array()) {
            out.extend(items.iter().cloned());
        }
        let has_more = value
            .pointer("/data/has_more")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !has_more {
            break;
        }
        page_token = value
            .pointer("/data/page_token")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        if page_token.is_none() {
            break;
        }
    }
    Ok(out)
}

fn shorten(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskPriority;
    use chrono::TimeZone;

    fn sample_task() -> TaskItem {
        let created = Utc.with_ymd_and_hms(2026, 11, 16, 8, 0, 0).unwrap();
        TaskItem {
            id: Uuid::parse_str("a3051932-e67f-4762-804a-aaf49af97b05").unwrap(),
            title: "整理本周会议纪要".into(),
            notes: "汇总后发到项目群".into(),
            is_completed: false,
            is_current: false,
            created_at: created,
            updated_at: created,
            completed_at: None,
            sort_index: 0,
            priority_raw_value: Some(TaskPriority::High as i32),
            due_at: None,
            reminder_at: None,
            repeat_rule_raw_value: None,
            tags_raw_value: Some("复盘,文档".into()),
            project_name: Some("示例项目".into()),
            estimated_minutes: None,
            today_sort_index: None,
            today_added_date: None,
            subtasks_raw_value: None,
            focus_started_at: None,
            focus_accumulated_seconds: Some(1200.0),
            postponed_at: None,
            postpone_count_raw_value: Some(2),
        }
    }

    #[test]
    fn create_fields_omit_empty_dates_and_numbers() {
        let fields = task_fields(&sample_task(), false);
        let object = fields.as_object().unwrap();
        assert!(!object.contains_key("截止日期"));
        assert!(!object.contains_key("预计分钟"));
        assert_eq!(object.get("状态").unwrap(), "待办");
        assert_eq!(object.get("优先级").unwrap(), "高");
        assert_eq!(object.get("标签").unwrap(), "复盘,文档");
        assert_eq!(object.get("专注分钟").unwrap(), 20);
        assert_eq!(object.get("推迟次数").unwrap(), 2);
        assert_eq!(object.get("当前任务").unwrap(), false);
    }

    #[test]
    fn update_fields_clear_empty_values_with_null() {
        let fields = task_fields(&sample_task(), true);
        let object = fields.as_object().unwrap();
        assert!(object.get("截止日期").unwrap().is_null());
        assert!(object.get("预计分钟").unwrap().is_null());
    }

    #[test]
    fn completed_task_pushes_finished_status() {
        let mut task = sample_task();
        task.is_completed = true;
        task.completed_at = Some(Utc.with_ymd_and_hms(2026, 11, 16, 9, 0, 0).unwrap());
        let fields = task_fields(&task, false);
        assert_eq!(fields.get("状态").unwrap(), "已完成");
        let expected = task.completed_at.unwrap().timestamp_millis();
        assert_eq!(fields.get("完成时间").unwrap().as_i64(), Some(expected));
    }

    #[test]
    fn text_value_flattens_nested_shapes() {
        assert_eq!(text_value(&json!("已完成")), "已完成");
        assert_eq!(text_value(&json!([{ "text": "已完成" }])), "已完成");
        assert_eq!(text_value(&json!(null)), "");
        assert_eq!(text_value(&json!({"text": "abc"})), "abc");
    }

    #[test]
    fn sync_due_respects_interval() {
        let mut config = FeishuConfig::default();
        assert!(is_sync_due(&config));
        config.last_sync_at = Some((Utc::now() - ChronoDuration::minutes(1)).to_rfc3339());
        assert!(!is_sync_due(&config));
        config.last_sync_at = Some((Utc::now() - ChronoDuration::minutes(30)).to_rfc3339());
        assert!(is_sync_due(&config));
    }

    #[test]
    fn dirty_sync_waits_for_cooldown_and_config() {
        let mut config = FeishuConfig {
            enabled: true,
            app_id: "cli_test".into(),
            app_secret: "secret".into(),
            app_token: "app".into(),
            table_id: "tbl".into(),
            ..Default::default()
        };
        config.last_sync_at = Some((Utc::now() - ChronoDuration::minutes(1)).to_rfc3339());
        assert!(!should_run_sync(&config, false));
        assert!(should_run_sync(&config, true));

        // 刚同步完，有改动也要等冷却
        config.last_sync_at = Some(Utc::now().to_rfc3339());
        assert!(!should_run_sync(&config, true));

        // 未启用 / 未配置时不跑
        assert!(!should_run_sync(&FeishuConfig::default(), true));
        let mut partial = config.clone();
        partial.app_secret = String::new();
        assert!(!should_run_sync(&partial, true));
    }
}
