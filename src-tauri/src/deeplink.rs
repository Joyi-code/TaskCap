use crate::models::TaskPriority;
use crate::store::TaskStore;
use std::process::Command;
use std::sync::Mutex;
use tauri::AppHandle;
use url::Url;

pub enum DeepLinkAction {
    Add {
        title: String,
        notes: String,
        priority: TaskPriority,
    },
    Focus,
    Complete,
    Show,
}

pub fn register_protocol() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    let proto_key = r"HKCU\Software\Classes\taskcap";
    for (key, value) in [
        (proto_key, "URL:taskcap Protocol"),
        (&format!("{proto_key}\\URL Protocol"), ""),
        (&format!("{proto_key}\\shell\\open\\command"), command.as_str()),
    ] {
        let status = Command::new("reg")
            .args(["add", key, "/ve", "/d", value, "/f"])
            .status()
            .map_err(|e| format!("注册 taskcap:// 协议失败：{e}"))?;
        if !status.success() {
            return Err("注册 taskcap:// 协议失败".to_string());
        }
    }
    Ok(())
}

pub fn parse_taskcap_url(raw: &str) -> Result<DeepLinkAction, String> {
    let url = Url::parse(raw).map_err(|e| e.to_string())?;
    if url.scheme() != "taskcap" {
        return Err("unsupported url scheme".to_string());
    }

    let action = url
        .host_str()
        .map(str::to_lowercase)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let path = url.path().trim_matches('/');
            if path.is_empty() {
                None
            } else {
                Some(path.to_lowercase())
            }
        })
        .ok_or_else(|| "missing action".to_string())?;

    match action.as_str() {
        "add" | "new" => {
            let mut query_pairs: Vec<(String, String)> = url
                .query_pairs()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            let title = query_pairs
                .iter()
                .find(|(k, _)| k == "title")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let notes = query_pairs
                .iter()
                .find(|(k, _)| k == "notes")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let priority = query_pairs
                .iter()
                .find(|(k, _)| k == "priority")
                .map(|(_, v)| parse_priority(v))
                .unwrap_or(TaskPriority::Medium);
            let _ = query_pairs;
            Ok(DeepLinkAction::Add {
                title,
                notes,
                priority,
            })
        }
        "focus" | "start" => Ok(DeepLinkAction::Focus),
        "complete" | "done" => Ok(DeepLinkAction::Complete),
        "show" => Ok(DeepLinkAction::Show),
        other => Err(format!("unknown action: {other}")),
    }
}

fn parse_priority(raw: &str) -> TaskPriority {
    match raw.to_lowercase().as_str() {
        "high" | "p1" | "高" | "0" => TaskPriority::High,
        "low" | "p3" | "低" | "2" => TaskPriority::Low,
        _ => TaskPriority::Medium,
    }
}

pub fn handle_deep_link(
    app: &AppHandle,
    store: &Mutex<TaskStore>,
    raw: &str,
    show_panel: fn(AppHandle) -> Result<(), String>,
    show_quickadd: fn(AppHandle) -> Result<(), String>,
) -> Result<(), String> {
    let action = parse_taskcap_url(raw)?;
    match action {
        DeepLinkAction::Add {
            title,
            notes,
            priority,
        } => {
            if title.trim().is_empty() {
                let _ = show_quickadd(app.clone());
                return Ok(());
            }
            let mut store = store.lock().map_err(|e| e.to_string())?;
            store.add_task(&title, &notes, priority)?;
        }
        DeepLinkAction::Focus => {
            let mut store = store.lock().map_err(|e| e.to_string())?;
            let target_id = store
                .current_task()
                .map(|t| t.id)
                .or_else(|| store.incomplete_tasks().first().map(|t| t.id));
            if let Some(id) = target_id {
                store.start_focus(id, chrono::Utc::now())?;
            }
        }
        DeepLinkAction::Complete => {
            let mut store = store.lock().map_err(|e| e.to_string())?;
            let target_id = store
                .current_task()
                .map(|t| t.id)
                .or_else(|| store.incomplete_tasks().first().map(|t| t.id));
            if let Some(id) = target_id {
                store.complete(id, chrono::Utc::now())?;
            }
        }
        DeepLinkAction::Show => {
            let _ = show_panel(app.clone());
        }
    }
    Ok(())
}

pub fn collect_startup_urls() -> Vec<String> {
    std::env::args()
        .filter(|arg| arg.starts_with("taskcap://"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskPriority;

    #[test]
    fn parses_add_with_priority() {
        let action = parse_taskcap_url("taskcap://add?title=周报&priority=high").unwrap();
        match action {
            DeepLinkAction::Add {
                title,
                notes,
                priority,
            } => {
                assert_eq!(title, "周报");
                assert_eq!(notes, "");
                assert_eq!(priority, TaskPriority::High);
            }
            _ => panic!("expected add action"),
        }
    }

    #[test]
    fn parses_show_and_focus_aliases() {
        assert!(matches!(
            parse_taskcap_url("taskcap://show").unwrap(),
            DeepLinkAction::Show
        ));
        assert!(matches!(
            parse_taskcap_url("taskcap://start").unwrap(),
            DeepLinkAction::Focus
        ));
        assert!(matches!(
            parse_taskcap_url("taskcap://done").unwrap(),
            DeepLinkAction::Complete
        ));
    }
}