use crate::models::TaskItem;
use crate::AppState;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

pub struct ReminderState {
    pub fired: Mutex<HashSet<Uuid>>,
}

impl ReminderState {
    pub fn new() -> Self {
        Self {
            fired: Mutex::new(HashSet::new()),
        }
    }
}

pub fn start_reminder_loop(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(15));
        let app_handle = app.clone();
        let poll_handle = app_handle.clone();
        let _ = app_handle.run_on_main_thread(move || {
            let _ = poll_due_reminders(&poll_handle);
        });
    });
}

fn poll_due_reminders(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let now = Utc::now();
    let due = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        store
            .incomplete_tasks()
            .into_iter()
            .filter(|task| is_reminder_due(task, now))
            .collect::<Vec<_>>()
    };
    let reminder_state = &state.reminders;

    for task in due {
        let should_fire = {
            let mut fired = reminder_state.fired.lock().map_err(|e| e.to_string())?;
            if fired.contains(&task.id) {
                false
            } else {
                fired.insert(task.id);
                true
            }
        };
        if !should_fire {
            continue;
        }
        show_toast(app, &task)?;
        app.emit("reminder-due", task.id.to_string())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn is_reminder_due(task: &TaskItem, now: DateTime<Utc>) -> bool {
    let Some(reminder_at) = task.reminder_at.or(task.due_at) else {
        return false;
    };
    if reminder_at > now {
        return false;
    }
    let age = now - reminder_at;
    age.num_seconds() <= 120
}

fn show_toast(app: &AppHandle, task: &TaskItem) -> Result<(), String> {
    let mut body = task.priority().title().to_string();
    if let Some(project) = task.project_name.as_ref().filter(|p| !p.is_empty()) {
        body.push_str(" · ");
        body.push_str(project);
    }
    if !task.tags().is_empty() {
        body.push_str(" · ");
        body.push_str(&task.tags().iter().map(|t| format!("#{t}")).collect::<Vec<_>>().join(" "));
    }

    app.notification()
        .builder()
        .title("TaskCap 提醒")
        .body(format!("{}\n{}", task.title, body))
        .show()
        .map_err(|e| e.to_string())
}

pub fn clear_fired_on_reload(reminder_state: &ReminderState) {
    if let Ok(mut fired) = reminder_state.fired.lock() {
        fired.clear();
    }
}

pub fn clear_fired_task(reminder_state: &ReminderState, id: Uuid) {
    if let Ok(mut fired) = reminder_state.fired.lock() {
        fired.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TaskItem;
    use chrono::TimeZone;
    use uuid::Uuid;

    fn sample_task(reminder_at: Option<DateTime<Utc>>) -> TaskItem {
        let now = Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap();
        TaskItem {
            id: Uuid::new_v4(),
            title: "提醒任务".to_string(),
            notes: String::new(),
            is_completed: false,
            is_current: false,
            created_at: now,
            updated_at: now,
            completed_at: None,
            sort_index: 0,
            priority_raw_value: Some(1),
            due_at: None,
            reminder_at,
            repeat_rule_raw_value: None,
            tags_raw_value: None,
            project_name: None,
            estimated_minutes: None,
            today_sort_index: None,
            today_added_date: None,
            subtasks_raw_value: None,
            focus_started_at: None,
            focus_accumulated_seconds: None,
            postponed_at: None,
            postpone_count_raw_value: None,
        }
    }

    #[test]
    fn reminder_due_within_two_minute_window() {
        let now = Utc.with_ymd_and_hms(2026, 6, 10, 12, 1, 30).unwrap();
        let reminder_at = Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap();
        let task = sample_task(Some(reminder_at));
        assert!(is_reminder_due(&task, now));
    }

    #[test]
    fn reminder_not_due_before_time_or_after_window() {
        let reminder_at = Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap();
        let before = Utc.with_ymd_and_hms(2026, 6, 10, 11, 59, 0).unwrap();
        let after = Utc.with_ymd_and_hms(2026, 6, 10, 12, 3, 0).unwrap();
        let task = sample_task(Some(reminder_at));
        assert!(!is_reminder_due(&task, before));
        assert!(!is_reminder_due(&task, after));
    }
}