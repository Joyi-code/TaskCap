use crate::models::{TaskCounts, TaskItem, TaskPriority};
use crate::store::{TaskDailyReview, TaskStore};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub id: String,
    pub title: String,
    pub notes: String,
    pub priority: i32,
    pub is_completed: bool,
    pub is_marked_complete: bool,
    pub is_current: bool,
    pub is_in_today_queue: bool,
    pub due_at: Option<String>,
    pub reminder_at: Option<String>,
    pub tags: Vec<String>,
    pub project_name: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub repeat_rule: Option<String>,
    pub focus_seconds: f64,
    pub is_focus_running: bool,
    pub subtask_done: usize,
    pub subtask_total: usize,
    pub completed_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelReviewDto {
    pub completed_today: usize,
    pub postponed_today: usize,
    pub tomorrow_count: usize,
    pub focus_minutes: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelSnapshot {
    pub menu_bar_title: String,
    pub counts: TaskCounts,
    pub today_count: u32,
    pub incomplete: Vec<TaskDetail>,
    pub completed: Vec<TaskDetail>,
    pub suggested: Vec<TaskDetail>,
    pub all_tags: Vec<String>,
    pub all_tag_suggestions: Vec<String>,
    pub all_projects: Vec<String>,
    pub review: PanelReviewDto,
    pub focus_task: Option<TaskDetail>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchTasksResult {
    pub incomplete: Vec<TaskDetail>,
    pub completed: Vec<TaskDetail>,
}

pub fn task_detail_from(
    task: &TaskItem,
    store: &TaskStore,
    now: DateTime<Utc>,
    marked_complete: bool,
) -> TaskDetail {
    TaskDetail {
        id: task.id.to_string(),
        title: task.title.clone(),
        notes: task.notes.clone(),
        priority: task.priority_raw_value.unwrap_or(1),
        is_completed: task.is_completed,
        is_marked_complete: marked_complete,
        is_current: task.is_current,
        is_in_today_queue: task.is_in_today_queue(),
        due_at: task.due_at.map(|d| d.to_rfc3339()),
        reminder_at: task.reminder_at.map(|d| d.to_rfc3339()),
        tags: task.tags(),
        project_name: task.project_name.clone(),
        estimated_minutes: task.estimated_minutes,
        repeat_rule: task.repeat_rule_raw_value.clone(),
        focus_seconds: store.focus_seconds(task.id, now),
        is_focus_running: task.focus_started_at.is_some(),
        subtask_done: task.completed_subtask_count(),
        subtask_total: task.subtasks().len(),
        completed_at: task.completed_at.map(|d| d.to_rfc3339()),
    }
}

pub fn build_panel_snapshot(store: &TaskStore, marked_complete: &HashSet<Uuid>) -> PanelSnapshot {
    let now = Utc::now();
    let review_data = store.daily_review(now);
    let focus_source = store
        .active_focus_task()
        .or_else(|| store.current_task());
    PanelSnapshot {
        menu_bar_title: store.menu_bar_title(),
        counts: store.task_counts(),
        today_count: store.today_tasks().len() as u32,
        incomplete: store
            .incomplete_tasks()
            .iter()
            .map(|t| task_detail_from(t, store, now, marked_complete.contains(&t.id)))
            .collect(),
        completed: store
            .completed_tasks()
            .iter()
            .map(|t| task_detail_from(t, store, now, false))
            .collect(),
        suggested: store
            .suggested_today_tasks(5, now)
            .iter()
            .map(|t| task_detail_from(t, store, now, marked_complete.contains(&t.id)))
            .collect(),
        all_tags: store.all_tags(),
        all_tag_suggestions: store.all_tag_suggestions(),
        all_projects: store.all_projects(),
        review: review_from(&review_data),
        focus_task: focus_source
            .as_ref()
            .map(|t| task_detail_from(t, store, now, marked_complete.contains(&t.id))),
    }
}

pub fn search_tasks(store: &TaskStore, query: &str, marked_complete: &HashSet<Uuid>) -> SearchTasksResult {
    let now = Utc::now();
    let cleaned = query.trim();
    if cleaned.is_empty() {
        return SearchTasksResult {
            incomplete: store
                .incomplete_tasks()
                .iter()
                .map(|t| task_detail_from(t, store, now, marked_complete.contains(&t.id)))
                .collect(),
            completed: vec![],
        };
    }
    let q = cleaned.to_lowercase();
    let incomplete = store
        .tasks_matching(cleaned)
        .iter()
        .map(|t| task_detail_from(t, store, now, marked_complete.contains(&t.id)))
        .collect();
    let completed = store
        .completed_tasks()
        .into_iter()
        .filter(|t| task_matches_query(t, &q))
        .map(|t| task_detail_from(&t, store, now, false))
        .collect();
    SearchTasksResult {
        incomplete,
        completed,
    }
}

fn task_matches_query(task: &TaskItem, q: &str) -> bool {
    task.title.to_lowercase().contains(q)
        || task.notes.to_lowercase().contains(q)
        || task
            .tags()
            .iter()
            .any(|tag| tag.to_lowercase().contains(q))
        || task
            .project_name
            .as_ref()
            .map(|p| p.to_lowercase().contains(q))
            .unwrap_or(false)
}

/// 历史查询：已完成任务，按关键词 + 完成时间范围过滤，按完成时间倒序
pub fn query_history(
    store: &TaskStore,
    query: &str,
    start_at: Option<DateTime<Utc>>,
    end_at: Option<DateTime<Utc>>,
) -> Vec<TaskDetail> {
    let now = Utc::now();
    let q = query.trim().to_lowercase();
    let has_range = start_at.is_some() || end_at.is_some();
    let mut items: Vec<TaskItem> = store
        .completed_tasks()
        .into_iter()
        .filter(|t| {
            if !q.is_empty() && !task_matches_query(t, &q) {
                return false;
            }
            if has_range {
                match t.completed_at {
                    Some(c) => {
                        if let Some(s) = start_at {
                            if c < s {
                                return false;
                            }
                        }
                        if let Some(e) = end_at {
                            if c > e {
                                return false;
                            }
                        }
                    }
                    None => return false,
                }
            }
            true
        })
        .collect();
    items.sort_by(|a, b| b.completed_at.cmp(&a.completed_at));
    items
        .into_iter()
        .map(|t| task_detail_from(&t, store, now, false))
        .collect()
}

fn review_from(review: &TaskDailyReview) -> PanelReviewDto {
    PanelReviewDto {
        completed_today: review.completed_today.len(),
        postponed_today: review.postponed_today.len(),
        tomorrow_count: review.tomorrow_tasks.len(),
        focus_minutes: review.focus_seconds / 60.0,
    }
}

pub fn priority_from_raw(value: i32) -> TaskPriority {
    TaskPriority::from_raw(Some(value))
}
