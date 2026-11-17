//! 应用内更新：查 GitHub Releases → 下载安装包 → 静默覆盖安装 → 自动重启。
//!
//! 只替换程序文件，**不碰 `%APPDATA%\taskcap\` 下的任务库、配置和同步状态**，
//! 所以升级不会丢数据。检查更新可以关闭，关闭后应用完全不联网。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::config;
use crate::AppState;

/// 版本信息只从这里取：GitHub 最新 release。
/// 可用环境变量 `TASKCAP_UPDATE_API` 指向镜像地址（企业内网或用测试源时用）。
const RELEASE_API: &str = "https://api.github.com/repos/Joyi-code/TaskCap/releases/latest";
const USER_AGENT: &str = "TaskCap-updater";
/// 自动检查间隔
const AUTO_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub download_url: String,
    pub size: u64,
    /// 空字符串表示发布方没给摘要，跳过校验
    pub sha256: String,
    /// true = NSIS 安装包（静默安装），false = 便携 exe（直接替换）
    pub is_installer: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSnapshot {
    pub current_version: String,
    pub checking: bool,
    pub applying: bool,
    pub available: Option<UpdateInfo>,
    pub skipped_version: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub last_check_at: Option<String>,
    pub last_error: Option<String>,
    /// 上次更新是否成功（更新后由 helper 写文件，下次启动读出来）
    pub last_apply_ok: Option<bool>,
    pub last_apply_message: Option<String>,
}

/// 运行期状态
pub struct UpdaterRuntime {
    checking: AtomicBool,
    applying: AtomicBool,
    inner: Mutex<UpdaterInner>,
}

#[derive(Default)]
struct UpdaterInner {
    available: Option<UpdateInfo>,
    last_check_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    downloaded: u64,
    total: u64,
    /// 上次更新结果（启动时从 update-result.json 读入一次）
    apply_result: Option<ApplyResult>,
}

impl UpdaterRuntime {
    pub fn new() -> Self {
        Self {
            checking: AtomicBool::new(false),
            applying: AtomicBool::new(false),
            inner: Mutex::new(UpdaterInner::default()),
        }
    }
}

impl Default for UpdaterRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------- 版本比较

/// 解析 "v0.2.1" / "0.2.1" / "0.2.1-beta" 这类版本号为数字段
fn version_parts(raw: &str) -> Vec<u64> {
    raw.trim()
        .trim_start_matches(|c| c == 'v' || c == 'V')
        .split(|c| c == '-' || c == '+')
        .next()
        .unwrap_or("")
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

/// `candidate` 是否比 `current` 新
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let a = version_parts(candidate);
    let b = version_parts(current);
    let len = a.len().max(b.len());
    for index in 0..len {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

// ---------------------------------------------------------------- 发布信息解析

/// 从 GitHub release JSON 里挑出可用资产。
///
/// 优先便携版（名字含 portable），其次 NSIS 安装包（名字含 setup），
/// 都没有返回 None。
fn pick_asset(release: &Value) -> Option<UpdateInfo> {
    let version = release
        .get("tag_name")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim_start_matches(['v', 'V'])
        .to_string();
    if version.is_empty() {
        return None;
    }
    let notes = release
        .get("body")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let assets = release
        .get("assets")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let find = |needle: &str| {
        assets.iter().find(|asset| {
            asset
                .get("name")
                .and_then(|value| value.as_str())
                .map(|name| name.to_lowercase().contains(needle))
                .unwrap_or(false)
                && asset
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(|name| name.to_lowercase().ends_with(".exe"))
                    .unwrap_or(false)
        })
    };

    // 默认就用发版时 Tauri 产出的安装包 TaskCap_X.Y.Z_x64-setup.exe；
    // 只有找不到安装包时，才退回名字含 portable 的便携版。
    let (asset, is_installer) = match (find("setup"), find("portable")) {
        (Some(setup), _) => (setup, true),
        (None, Some(portable)) => (portable, false),
        (None, None) => return None,
    };

    let asset_name = asset.get("name")?.as_str()?.to_string();
    let download_url = asset.get("browser_download_url")?.as_str()?.to_string();
    let size = asset.get("size").and_then(|value| value.as_u64()).unwrap_or(0);
    let sha256 = asset
        .get("digest")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim_start_matches("sha256:")
        .to_string();

    Some(UpdateInfo {
        version,
        notes,
        asset_name,
        download_url,
        size,
        sha256,
        is_installer,
    })
}

// ---------------------------------------------------------------- 对外状态

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn result_path() -> Option<PathBuf> {
    config::config_path()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("update-result.json")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApplyResult {
    ok: bool,
    message: String,
    version: String,
}

fn load_apply_result() -> Option<ApplyResult> {
    let path = result_path()?;
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn snapshot(app: &AppHandle) -> UpdateSnapshot {
    let runtime = app.state::<AppState>();
    let config = config::load_config();
    let (available, last_check_at, last_error, downloaded, total, apply_result) =
        match runtime.updater.inner.lock() {
            Ok(inner) => (
                inner.available.clone(),
                inner.last_check_at,
                inner.last_error.clone(),
                inner.downloaded,
                inner.total,
                inner.apply_result.clone(),
            ),
            Err(_) => (None, None, None, 0, 0, None),
        };
    UpdateSnapshot {
        current_version: current_version(),
        checking: runtime.updater.checking.load(Ordering::SeqCst),
        applying: runtime.updater.applying.load(Ordering::SeqCst),
        available,
        skipped_version: config.skipped_update_version.clone(),
        downloaded_bytes: downloaded,
        total_bytes: total,
        last_check_at: last_check_at.map(|time| time.to_rfc3339()),
        last_error,
        last_apply_ok: apply_result.as_ref().map(|result| result.ok),
        last_apply_message: apply_result.map(|result| result.message),
    }
}

/// 启动时读取上次更新结果并清掉文件，避免设置页一直显示旧消息
pub fn init(app: &AppHandle) {
    let Some(result) = load_apply_result() else {
        return;
    };
    if let Some(path) = result_path() {
        let _ = fs::remove_file(path);
    }
    let runtime = app.state::<AppState>();
    let guard = runtime.updater.inner.lock();
    if let Ok(mut inner) = guard {
        inner.apply_result = Some(result);
    }
}

fn emit_state(app: &AppHandle) {
    let _ = app.emit("update-state", snapshot(app));
}

// ---------------------------------------------------------------- 检查

/// 查询最新 release；`force = false` 时 10 分钟内不重复请求
pub fn check(app: &AppHandle, force: bool) -> Result<UpdateSnapshot, String> {
    let runtime = app.state::<AppState>();
    if runtime.updater.checking.swap(true, Ordering::SeqCst) {
        return Ok(snapshot(app));
    }
    let result = check_inner(app, force);
    runtime.updater.checking.store(false, Ordering::SeqCst);
    emit_state(app);
    // 必须在清掉 checking 之后再取快照：check_inner 里取的快照还是「检查中」，
    // 直接返回会让前端把按钮永远停在「检查中…」。
    let final_snapshot = snapshot(app);
    result.map(|_| final_snapshot)
}

fn check_inner(app: &AppHandle, force: bool) -> Result<UpdateSnapshot, String> {
    let runtime = app.state::<AppState>();
    if !force {
        if let Ok(inner) = runtime.updater.inner.lock() {
            if let Some(last) = inner.last_check_at {
                if Utc::now().signed_duration_since(last).num_seconds() < 600 {
                    drop(inner);
                    return Ok(snapshot(app));
                }
            }
        }
    }

    let fetched = fetch_latest_release();
    let mut inner = runtime.updater.inner.lock().map_err(|e| e.to_string())?;
    inner.last_check_at = Some(Utc::now());
    match fetched {
        Ok(release) => {
            inner.last_error = None;
            inner.available = release
                .filter(|info| is_newer(&info.version, &current_version()));
            log_line(&format!(
                "check ok current={} newer={}",
                current_version(),
                inner
                    .available
                    .as_ref()
                    .map(|info| info.version.clone())
                    .unwrap_or_else(|| "none".to_string())
            ));
            inner.downloaded = 0;
            inner.total = 0;
        }
        Err(error) => {
            log_line(&format!("check fail {error}"));
            inner.last_error = Some(error);
        }
    }
    drop(inner);
    Ok(snapshot(app))
}

fn log_line(message: &str) {
    let Ok(dir) = config::config_path() else {
        return;
    };
    let Some(parent) = dir.parent() else {
        return;
    };
    let _ = fs::create_dir_all(parent);
    let path = parent.join("updater.log");
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "{} {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            message
        );
    }
}

fn fetch_latest_release() -> Result<Option<UpdateInfo>, String> {
    let endpoint = std::env::var("TASKCAP_UPDATE_API").unwrap_or_else(|_| RELEASE_API.to_string());
    log_line(&format!("check endpoint {endpoint}"));
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(12))
        .build();
    let mut request = agent
        .get(&endpoint)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json");
    // 可选：配置了 token 就走认证额度（每小时 5000 次），普通用户不需要
    if let Ok(token) = std::env::var("TASKCAP_UPDATE_TOKEN") {
        if !token.trim().is_empty() {
            request = request.set("Authorization", &format!("Bearer {}", token.trim()));
        }
    }
    let response = request.call();
    let value: Value = match response {
        Ok(resp) => resp
            .into_json()
            .map_err(|e| format!("解析版本信息失败：{e}"))?,
        Err(ureq::Error::Status(code, resp)) => {
            if code == 403 || code == 429 {
                // GitHub 未登录接口每小时 60 次，用 reset 时间告诉用户等多久
                let minutes = resp
                    .header("x-ratelimit-reset")
                    .and_then(|value| value.parse::<i64>().ok())
                    .map(|reset| ((reset - Utc::now().timestamp()).max(0) as f64 / 60.0).ceil() as i64)
                    .unwrap_or(0);
                return Err(if minutes > 0 {
                    format!("GitHub 接口限流（未登录每小时 60 次），约 {minutes} 分钟后自动重试")
                } else {
                    "GitHub 接口限流（未登录每小时 60 次），稍后自动重试".to_string()
                });
            }
            return Err(match code {
                404 => "没有找到发布版本（仓库还没有 release）".to_string(),
                _ => format!("查询版本失败：HTTP {code}"),
            });
        }
        Err(error) => return Err(format!("联网失败：{error}")),
    };
    Ok(pick_asset(&value).inspect(|info| {
        log_line(&format!(
            "release {} asset {} installer={}",
            info.version, info.asset_name, info.is_installer
        ));
    }))
}

// ---------------------------------------------------------------- 下载与安装

/// 开始下载并安装；立即返回，进度通过 `update-state` 事件推送
pub fn start_apply(app: &AppHandle) -> Result<(), String> {
    let runtime = app.state::<AppState>();
    if runtime.updater.applying.swap(true, Ordering::SeqCst) {
        return Err("正在更新，请稍候".to_string());
    }
    let target = {
        let inner = runtime.updater.inner.lock().map_err(|e| e.to_string())?;
        match inner.available.clone() {
            Some(info) => info,
            None => {
                runtime.updater.applying.store(false, Ordering::SeqCst);
                return Err("没有可用的新版本".to_string());
            }
        }
    };

    let app_handle = app.clone();
    std::thread::spawn(move || {
        let result = download_and_install(&app_handle, &target);
        let runtime = app_handle.state::<AppState>();
        if let Err(error) = &result {
            if let Ok(mut inner) = runtime.updater.inner.lock() {
                inner.last_error = Some(error.clone());
            }
        }
        runtime.updater.applying.store(false, Ordering::SeqCst);
        emit_state(&app_handle);
    });
    Ok(())
}

fn updates_dir() -> Result<PathBuf, String> {
    let dir = config::config_path()?
        .parent()
        .map(|parent| parent.join("updates"))
        .ok_or("找不到数据目录")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn download_and_install(app: &AppHandle, info: &UpdateInfo) -> Result<(), String> {
    let dir = updates_dir()?;
    let target = dir.join(&info.asset_name);
    let temp = dir.join(format!("{}.part", info.asset_name));

    // 清掉旧安装包，只留这次的
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path != target && path != temp && path.extension().and_then(|e| e.to_str()) == Some("exe") {
                let _ = fs::remove_file(path);
            }
        }
    }

    download_file(app, info, &temp)?;

    if !info.sha256.is_empty() && !sha256_matches(&temp, &info.sha256) {
        let _ = fs::remove_file(&temp);
        return Err("安装包校验失败，已删除下载文件".to_string());
    }
    fs::rename(&temp, &target).map_err(|e| e.to_string())?;

    // 用当前 exe 的副本当更新助手：它等本进程退出后再替换/安装，然后启动新版本
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = std::env::temp_dir().join("taskcap-update-helper.exe");
    fs::copy(&current, &helper).map_err(|e| format!("复制更新助手失败：{e}"))?;

    write_apply_result(false, "更新已开始，安装完成后会自动重启", &info.version);

    let spawned = std::process::Command::new(&helper)
        .arg("--apply-update")
        .arg(&target)
        .arg(&current)
        .arg(&info.version)
        .spawn();
    if spawned.is_err() {
        return Err("无法启动更新助手".to_string());
    }

    // 把待完成的结果留给下次启动读取；这里先按“进行中”记
    std::thread::sleep(Duration::from_millis(300));
    app.exit(0);
    Ok(())
}

fn download_file(app: &AppHandle, info: &UpdateInfo, dest: &Path) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(600))
        .build();
    let response = agent
        .get(&info.download_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| format!("下载失败：{error}"))?;
    let total = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(info.size);
    let mut reader = response.into_reader();
    let mut file = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 64 * 1024];
    let mut got: u64 = 0;
    let mut last_emit = Instant::now();

    loop {
        let read = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
        got += read as u64;
        if last_emit.elapsed() >= Duration::from_millis(250) {
            last_emit = Instant::now();
            if let Ok(mut inner) = app.state::<AppState>().updater.inner.lock() {
                inner.downloaded = got;
                inner.total = total;
            }
            emit_state(app);
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    if total > 0 && got != total {
        return Err(format!("下载不完整：{got}/{total} 字节"));
    }
    Ok(())
}

fn sha256_matches(path: &Path, expected: &str) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = format!("{:x}", hasher.finalize());
    digest.eq_ignore_ascii_case(expected.trim())
}

fn write_apply_result(ok: bool, message: &str, version: &str) {
    let Some(path) = result_path() else {
        return;
    };
    let payload = ApplyResult {
        ok,
        message: message.to_string(),
        version: version.to_string(),
    };
    if let Ok(raw) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(path, raw);
    }
}

// ---------------------------------------------------------------- 更新助手

/// `--apply-update <下载的安装包> <当前 exe> <新版本号>`：等旧进程退出后安装并重启
pub fn apply_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    let index = args.iter().position(|arg| arg == "--apply-update")?;
    let downloaded = PathBuf::from(args.get(index + 1)?);
    let target = PathBuf::from(args.get(index + 2)?);
    let version = args
        .get(index + 3)
        .cloned()
        .unwrap_or_else(current_version);
    Some(run_apply(downloaded, target, &version))
}

fn run_apply(downloaded: PathBuf, target: PathBuf, version: &str) -> i32 {
    log_line(&format!(
        "apply start version={version} file={} target={}",
        downloaded.display(),
        target.display()
    ));
    // 给旧进程足够时间释放 exe 与窗口
    std::thread::sleep(Duration::from_millis(1500));

    let is_installer = downloaded
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_lowercase().contains("setup"))
        .unwrap_or(false);
    let install_dir = target
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let ok = if is_installer {
        run_installer(&downloaded, &install_dir)
    } else {
        swap_exe(&downloaded, &target)
    };

    if ok {
        write_apply_result(true, "已更新到新版本", version);
        let _ = std::process::Command::new(&target).spawn();
        0
    } else {
        write_apply_result(false, "更新失败，已保留旧版本，可手动安装", version);
        let _ = std::process::Command::new(&target).spawn();
        1
    }
}

fn run_installer(installer: &Path, install_dir: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // 旧进程刚退出时文件可能还被占用，失败就重试几次
        for attempt in 1..=3 {
            let mut command = std::process::Command::new(installer);
            // NSIS 静默安装；/D 必须是最后一个参数且不加引号
            command.args(["/S"]).arg(format!("/D={}", install_dir.display()));
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            match command.status() {
                Ok(status) if status.success() => {
                    log_line(&format!("installer ok on attempt {attempt}"));
                    return true;
                }
                Ok(status) => {
                    log_line(&format!(
                        "installer attempt {attempt} exit={:?}",
                        status.code()
                    ));
                }
                Err(error) => {
                    log_line(&format!("installer attempt {attempt} error={error}"));
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        false
    }
    #[cfg(not(windows))]
    {
        let _ = (installer, install_dir);
        false
    }
}

/// 便携版：只替换 exe，其他文件（任务库在 %APPDATA%）一概不碰
fn swap_exe(new: &Path, target: &Path) -> bool {
    let old = target.with_extension("old.exe");
    let _ = fs::remove_file(&old);
    let mut moved = false;
    for _ in 0..150 {
        if fs::rename(target, &old).is_ok() {
            moved = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if !moved {
        return false;
    }
    if fs::rename(new, target).is_err() {
        let _ = fs::rename(&old, target);
        return false;
    }
    true
}

// ---------------------------------------------------------------- 后台自动检查

pub fn start_auto_check(app: AppHandle) {
    std::thread::spawn(move || {
        // 启动后先等一下，不跟首屏抢资源
        std::thread::sleep(Duration::from_secs(12));
        let mut last_check = Instant::now();
        let mut wait = AUTO_CHECK_INTERVAL;
        loop {
            let config = config::load_config();
            let runtime = app.state::<AppState>();
            let busy = runtime.updater.checking.load(Ordering::SeqCst)
                || runtime.updater.applying.load(Ordering::SeqCst);
            if config.auto_check_updates && !busy && last_check.elapsed() >= wait {
                let result = check(&app, false);
                // 失败（断网、限流）30 分钟后再试，成功则回到 6 小时
                wait = match result {
                    Ok(snapshot) if snapshot.last_error.is_none() => AUTO_CHECK_INTERVAL,
                    _ => Duration::from_secs(30 * 60),
                };
                last_check = Instant::now();
            }
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}

/// 启动时检查一次（受 auto_check_updates 控制）
pub fn check_on_startup(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(12));
        if config::load_config().auto_check_updates {
            let _ = check(&app, true);
        }
    });
}

/// 用户忽略某个版本
pub fn skip_version(app: &AppHandle, version: &str) -> Result<UpdateSnapshot, String> {
    let mut config = config::load_config();
    config.skipped_update_version = Some(version.to_string());
    config::save_config(&config)?;
    emit_state(app);
    Ok(snapshot(app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("v0.3.0", "0.2.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
    }

    #[test]
    fn picks_setup_asset_with_digest() {
        let release = json!({
            "tag_name": "v0.3.0",
            "body": "更新说明",
            "assets": [
                {"name": "TaskCap_0.3.0_x64-setup.exe", "size": 100, "browser_download_url": "https://example.com/setup.exe", "digest": "sha256:abc"},
                {"name": "notes.md", "size": 10, "browser_download_url": "https://example.com/notes.md"}
            ]
        });
        let info = pick_asset(&release).unwrap();
        assert_eq!(info.version, "0.3.0");
        assert!(info.is_installer);
        assert_eq!(info.sha256, "abc");
    }

    #[test]
    fn prefers_setup_asset() {
        let release = json!({
            "tag_name": "v0.3.0",
            "assets": [
                {"name": "taskcap-0.3.0-portable.exe", "size": 1, "browser_download_url": "https://example.com/portable.exe"},
                {"name": "TaskCap_0.3.0_x64-setup.exe", "size": 1, "browser_download_url": "https://example.com/setup.exe"}
            ]
        });
        let info = pick_asset(&release).unwrap();
        assert!(info.is_installer);
        assert_eq!(info.asset_name, "TaskCap_0.3.0_x64-setup.exe");
    }

    #[test]
    fn falls_back_to_portable_without_setup() {
        let release = json!({
            "tag_name": "v0.3.0",
            "assets": [
                {"name": "taskcap-0.3.0-portable.exe", "size": 1, "browser_download_url": "https://example.com/portable.exe"}
            ]
        });
        let info = pick_asset(&release).unwrap();
        assert!(!info.is_installer);
        assert_eq!(info.asset_name, "taskcap-0.3.0-portable.exe");
    }

    #[test]
    #[ignore = "需要联网，手动运行：cargo test --release live_fetch -- --ignored"]
    fn live_fetch_latest_parses() {
        let result = fetch_latest_release();
        assert!(result.is_ok(), "联网检查失败：{:?}", result.err());
        if let Ok(Some(info)) = result {
            assert!(!info.version.is_empty());
            assert!(info.download_url.starts_with("https://"));
        }
    }

    #[test]
    fn swap_replaces_only_the_exe() {
        let dir = std::env::temp_dir().join(format!("taskcap-swap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("taskcap.exe");
        let new = dir.join("taskcap-new.exe");
        fs::write(&exe, b"old").unwrap();
        fs::write(&new, b"new").unwrap();
        fs::write(dir.join("config.json"), b"{}").unwrap();

        assert!(swap_exe(&new, &exe));
        assert_eq!(fs::read(&exe).unwrap(), b"new");
        assert_eq!(fs::read(dir.join("config.json")).unwrap(), b"{}");
        let _ = fs::remove_dir_all(&dir);
    }
}
