use crate::db;
use crate::models::{
    TaskArchive, TaskArchiveItem, TaskCounts, TaskItem, TaskPostponeOption, TaskPriority,
    TaskRepeatRule, TaskSubtask,
};
use crate::parser::parse;
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct TaskDailyReview {
    pub completed_today: Vec<TaskItem>,
    pub postponed_today: Vec<TaskItem>,
    pub tomorrow_tasks: Vec<TaskItem>,
    pub focus_seconds: f64,
}

pub struct TaskStore {
    conn: Connection,
    pub tasks: Vec<TaskItem>,
    pub focus_attention_task_id: Option<Uuid>,
    pub last_error: Option<String>,
}

impl TaskStore {
    pub fn new_in_memory() -> Result<Self, String> {
        Self::with_connection(db::open_connection(None)?)
    }

    pub fn new_file() -> Result<Self, String> {
        let path = db::database_path()?;
        Self::with_connection(db::open_connection(Some(&path))?)
    }

    fn with_connection(conn: Connection) -> Result<Self, String> {
        let mut store = Self {
            conn,
            tasks: Vec::new(),
            focus_attention_task_id: None,
            last_error: None,
        };
        store.reload_tasks()?;
        Ok(store)
    }

    pub fn incomplete_tasks(&self) -> Vec<TaskItem> {
        self.sorted(self.tasks.iter().filter(|t| !t.is_completed).cloned().collect())
    }

    pub fn prioritized_incomplete_tasks(&self) -> Vec<TaskItem> {
        self.sorted_by_priority(self.incomplete_tasks())
    }

    pub fn completed_tasks(&self) -> Vec<TaskItem> {
        self.sorted(self.tasks.iter().filter(|t| t.is_completed).cloned().collect())
    }

    pub fn today_tasks(&self) -> Vec<TaskItem> {
        self.sorted_today(
            self.tasks
                .iter()
                .filter(|t| !t.is_completed && t.is_in_today_queue())
                .cloned()
                .collect(),
        )
    }

    pub fn current_task(&self) -> Option<TaskItem> {
        self.incomplete_tasks().into_iter().find(|t| t.is_current)
    }

    pub fn incomplete_count(&self) -> usize {
        self.incomplete_tasks().len()
    }

    pub fn priority_counts(&self) -> HashMap<TaskPriority, usize> {
        let mut map = HashMap::new();
        for task in self.incomplete_tasks() {
            *map.entry(task.priority()).or_insert(0) += 1;
        }
        map
    }

    pub fn task_counts(&self) -> TaskCounts {
        Self::counts_from_tasks(&self.incomplete_tasks())
    }

    /// 对齐 Swift `focusPriorityCounts`：有今天队列则按今天任务统计，否则按全部未完成
    pub fn focus_priority_counts(&self) -> TaskCounts {
        let today = self.today_tasks();
        let source = if today.is_empty() {
            self.incomplete_tasks()
        } else {
            today
        };
        Self::counts_from_tasks(&source)
    }

    /// 对齐 Swift 悬浮岛 attention 态：专注中任务优先，否则当前任务
    pub fn attention_task(&self) -> Option<TaskItem> {
        self.active_focus_task()
            .or_else(|| self.current_task())
    }

    pub fn expanded_island_height(&self) -> u32 {
        let visible_rows = self.incomplete_count().clamp(1, 3);
        (visible_rows * 35 + 17).max(92) as u32
    }

    fn counts_from_tasks(tasks: &[TaskItem]) -> TaskCounts {
        let mut high = 0u32;
        let mut medium = 0u32;
        let mut low = 0u32;
        for task in tasks {
            match task.priority() {
                TaskPriority::High => high += 1,
                TaskPriority::Medium => medium += 1,
                TaskPriority::Low => low += 1,
            }
        }
        TaskCounts {
            high,
            medium,
            low,
            total: high + medium + low,
        }
    }

    pub fn active_focus_task(&self) -> Option<TaskItem> {
        self.incomplete_tasks()
            .into_iter()
            .find(|t| t.focus_started_at.is_some())
    }

    pub fn focus_attention_task(&self) -> Option<TaskItem> {
        if let Some(active) = self.active_focus_task() {
            return Some(active);
        }
        let id = self.focus_attention_task_id?;
        self.incomplete_tasks().into_iter().find(|t| t.id == id)
    }

    pub fn menu_bar_title(&self) -> String {
        if let Some(current) = self.current_task() {
            return current.title.clone();
        }
        if self.incomplete_tasks().is_empty() {
            "已完成".to_string()
        } else {
            "暂无当前任务".to_string()
        }
    }

    pub fn preview_tasks(&self, limit: usize) -> Vec<TaskItem> {
        let mut tasks = self.incomplete_tasks();
        tasks.sort_by(|lhs, rhs| {
            lhs.priority_raw_value
                .cmp(&rhs.priority_raw_value)
                .then(rhs.updated_at.cmp(&lhs.updated_at))
                .then(rhs.created_at.cmp(&lhs.created_at))
                .then(lhs.sort_index.cmp(&rhs.sort_index))
        });
        tasks.into_iter().take(limit).collect()
    }

    pub fn add_task(&mut self, raw_title: &str, notes: &str, priority: TaskPriority) -> Result<Option<TaskItem>, String> {
        let parsed = parse(raw_title, priority, Utc::now());
        let title = parsed.title.trim();
        if title.is_empty() {
            return Ok(None);
        }
        let mut item = self.new_task_item(
            title.to_string(),
            notes.to_string(),
            false,
            false,
            Utc::now(),
            Utc::now(),
            None,
            parsed.priority,
            parsed.due_at,
            parsed.reminder_at,
            parsed.repeat_rule,
            parsed.tags,
            parsed.project_name,
            parsed.estimated_minutes,
            if parsed.is_today { Some(self.next_today_sort_index()) } else { None },
            vec![],
        );
        self.insert_task(&mut item)?;
        Ok(Some(item))
    }

    pub fn add_task_from_metadata(
        &mut self,
        title: &str,
        notes: &str,
        is_completed: bool,
        is_current: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        completed_at: Option<DateTime<Utc>>,
        priority: TaskPriority,
        due_at: Option<DateTime<Utc>>,
        reminder_at: Option<DateTime<Utc>>,
        repeat_rule: Option<TaskRepeatRule>,
        tags: Vec<String>,
        project_name: Option<String>,
        estimated_minutes: Option<i32>,
        today_sort_index: Option<i32>,
        subtasks: Vec<TaskSubtask>,
    ) -> Result<Option<TaskItem>, String> {
        let title = title.trim();
        if title.is_empty() {
            return Ok(None);
        }
        let mut item = self.new_task_item(
            title.to_string(),
            notes.to_string(),
            is_completed,
            is_current && !is_completed,
            created_at,
            updated_at,
            completed_at,
            priority,
            due_at,
            reminder_at,
            repeat_rule,
            tags,
            project_name,
            estimated_minutes,
            today_sort_index,
            subtasks,
        );
        self.insert_task(&mut item)?;
        self.normalize_current_task()?;
        Ok(Some(item))
    }

    pub fn update_title(&mut self, id: Uuid, raw_title: &str) -> Result<(), String> {
        let title = raw_title.trim();
        if title.is_empty() {
            return Ok(());
        }
        self.mutate_task(id, |task| {
            if task.title != title {
                task.title = title.to_string();
                task.updated_at = Utc::now();
            }
        })
    }

    pub fn update_notes(&mut self, id: Uuid, notes: &str) -> Result<(), String> {
        self.mutate_task(id, |task| {
            task.notes = notes.to_string();
            task.updated_at = Utc::now();
        })
    }

    pub fn complete(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), String> {
        let source = self
            .tasks
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or("task not found")?;
        if source.is_completed {
            return Ok(());
        }
        let next = self.make_next_recurring_task(&source);
        let clear_attention = self.focus_attention_task_id == Some(id);
        self.mutate_task(id, |task| {
            stop_focus_clock(task, now);
            task.is_completed = true;
            task.is_current = false;
            task.completed_at = Some(now);
            task.updated_at = now;
            task.focus_started_at = None;
        })?;
        if clear_attention {
            self.focus_attention_task_id = None;
        }
        if let Some(mut recurring) = next {
            self.insert_task(&mut recurring)?;
        }
        self.normalize_current_task()
    }

    /// 取消完成：将已完成任务恢复为未完成（撤销/回退用）
    pub fn reopen(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), String> {
        let source = self
            .tasks
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or("task not found")?;
        if !source.is_completed {
            return Ok(());
        }
        self.mutate_task(id, |task| {
            task.is_completed = false;
            task.completed_at = None;
            task.updated_at = now;
        })?;
        self.normalize_current_task()
    }

    pub fn delete(&mut self, id: Uuid) -> Result<(), String> {
        if self.focus_attention_task_id == Some(id) {
            self.focus_attention_task_id = None;
        }
        self.conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id.to_string()])
            .map_err(|e| e.to_string())?;
        self.reload_tasks()?;
        self.normalize_current_task()
    }

    pub fn set_current(&mut self, id: Uuid) -> Result<(), String> {
        self.clear_current_flags()?;
        self.mutate_task(id, |task| {
            if !task.is_completed {
                task.is_current = true;
                task.updated_at = Utc::now();
            }
        })?;
        self.commit_reload()
    }

    pub fn set_priority(&mut self, id: Uuid, priority: TaskPriority) -> Result<(), String> {
        self.mutate_task(id, |task| task.set_priority(priority))
    }

    pub fn set_today_queue(&mut self, id: Uuid, is_in_today_queue: bool) -> Result<(), String> {
        let next_today = self.next_today_sort_index();
        let today = local_date_str();
        self.mutate_task(id, move |task| {
            if is_in_today_queue {
                task.today_sort_index = Some(task.today_sort_index.unwrap_or(next_today));
                task.today_added_date = Some(today);
            } else {
                task.today_sort_index = None;
                task.today_added_date = None;
            };
            task.updated_at = Utc::now();
        })
    }

    pub fn set_project_name(&mut self, id: Uuid, project_name: Option<String>) -> Result<(), String> {
        self.mutate_task(id, |task| {
            let cleaned = project_name.map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
            task.project_name = cleaned;
            task.updated_at = Utc::now();
        })
    }

    pub fn set_tags(&mut self, id: Uuid, tags: Vec<String>) -> Result<(), String> {
        self.mutate_task(id, |task| task.set_tags(tags))
    }

    pub fn set_estimated_minutes(&mut self, id: Uuid, minutes: Option<i32>) -> Result<(), String> {
        self.mutate_task(id, |task| {
            task.estimated_minutes = minutes;
            task.updated_at = Utc::now();
        })
    }

    pub fn set_repeat_rule(&mut self, id: Uuid, rule: Option<TaskRepeatRule>) -> Result<(), String> {
        self.mutate_task(id, |task| task.set_repeat_rule(rule))
    }

    pub fn set_due_reminder(
        &mut self,
        id: Uuid,
        due_at: Option<DateTime<Utc>>,
        reminder_at: Option<DateTime<Utc>>,
    ) -> Result<(), String> {
        self.mutate_task(id, |task| {
            task.due_at = due_at;
            task.reminder_at = reminder_at;
            task.updated_at = Utc::now();
        })
    }

    pub fn advance_current(&mut self) -> Result<(), String> {
        let active = self.incomplete_tasks();
        if active.is_empty() {
            return Ok(());
        }
        if self.current_task().is_none() {
            return self.set_current(active[0].id);
        }
        let current_id = self.current_task().unwrap().id;
        let idx = active.iter().position(|t| t.id == current_id).unwrap_or(0);
        let next_idx = if idx + 1 >= active.len() { 0 } else { idx + 1 };
        self.set_current(active[next_idx].id)
    }

    pub fn start_focus(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), String> {
        let others: Vec<Uuid> = self
            .incomplete_tasks()
            .into_iter()
            .filter(|t| t.focus_started_at.is_some() && t.id != id)
            .map(|t| t.id)
            .collect();
        for other_id in others {
            self.mutate_task(other_id, |task| stop_focus_clock(task, now))?;
        }
        self.clear_current_flags()?;
        self.focus_attention_task_id = Some(id);
        self.mutate_task(id, |task| {
            if task.is_completed {
                return;
            }
            task.is_current = true;
            if task.focus_started_at.is_none() {
                task.focus_started_at = Some(now);
            }
            task.updated_at = now;
        })
    }

    pub fn pause_focus(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), String> {
        self.mutate_task(id, |task| {
            if !task.is_completed {
                stop_focus_clock(task, now);
            }
        })?;
        self.focus_attention_task_id = Some(id);
        Ok(())
    }

    pub fn stop_focus(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), String> {
        self.mutate_task(id, |task| {
            // 停止 = 结束本轮，计时复位到 0（区别于暂停的累加保留）
            task.focus_started_at = None;
            task.set_focus_seconds(0.0);
            task.updated_at = now;
        })?;
        if self.focus_attention_task_id == Some(id) {
            self.focus_attention_task_id = None;
        }
        Ok(())
    }

    /// 关闭专注任务：清空专注状态并取消「当前任务」标记
    pub fn close_focus(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), String> {
        self.mutate_task(id, |task| {
            task.focus_started_at = None;
            task.set_focus_seconds(0.0);
            task.is_current = false;
            task.updated_at = now;
        })?;
        if self.focus_attention_task_id == Some(id) {
            self.focus_attention_task_id = None;
        }
        Ok(())
    }

    pub fn focus_seconds(&self, id: Uuid, now: DateTime<Utc>) -> f64 {
        let task = match self.tasks.iter().find(|t| t.id == id) {
            Some(t) => t,
            None => return 0.0,
        };
        let accumulated = task.focus_seconds();
        if let Some(started) = task.focus_started_at {
            return accumulated + (now - started).num_seconds().max(0) as f64;
        }
        accumulated
    }

    pub fn focus_target_minutes(&self, id: Uuid, default_minutes: i32) -> i32 {
        let task = self.tasks.iter().find(|t| t.id == id);
        task.and_then(|t| t.estimated_minutes)
            .unwrap_or(default_minutes)
            .max(1)
    }

    pub fn focus_remaining_seconds(&self, id: Uuid, now: DateTime<Utc>, default_minutes: i32) -> f64 {
        let target = self.focus_target_minutes(id, default_minutes) as f64 * 60.0;
        (target - self.focus_seconds(id, now)).max(0.0)
    }

    pub fn add_subtask(&mut self, id: Uuid, title: &str) -> Result<(), String> {
        let cleaned = title.trim();
        if cleaned.is_empty() {
            return Ok(());
        }
        self.mutate_task(id, |task| {
            let mut subtasks = task.subtasks();
            subtasks.push(TaskSubtask::new(cleaned));
            task.set_subtasks(subtasks);
        })
    }

    pub fn toggle_subtask(&mut self, task_id: Uuid, subtask_id: Uuid) -> Result<(), String> {
        self.mutate_task(task_id, |task| {
            let mut subtasks = task.subtasks();
            if let Some(item) = subtasks.iter_mut().find(|s| s.id == subtask_id) {
                item.is_completed = !item.is_completed;
                task.set_subtasks(subtasks);
            }
        })
    }

    pub fn delete_subtask(&mut self, task_id: Uuid, subtask_id: Uuid) -> Result<(), String> {
        self.mutate_task(task_id, |task| {
            let mut subtasks = task.subtasks();
            subtasks.retain(|s| s.id != subtask_id);
            task.set_subtasks(subtasks);
        })
    }

    pub fn postpone(&mut self, id: Uuid, option: TaskPostponeOption, now: DateTime<Utc>) -> Result<(), String> {
        let due_at = match option {
            TaskPostponeOption::FifteenMinutes => now + Duration::minutes(15),
            TaskPostponeOption::LaterToday => {
                let two_hours = now + Duration::hours(2);
                let evening = Utc
                    .with_ymd_and_hms(now.year(), now.month(), now.day(), 18, 0, 0)
                    .single()
                    .unwrap_or(two_hours);
                if two_hours > evening { two_hours } else { evening }
            }
            TaskPostponeOption::Tomorrow => {
                let tomorrow = now + Duration::days(1);
                Utc.with_ymd_and_hms(
                    tomorrow.year(),
                    tomorrow.month(),
                    tomorrow.day(),
                    9,
                    0,
                    0,
                )
                .single()
                .unwrap_or(tomorrow)
            }
            TaskPostponeOption::ThisWeek => {
                let later = now + Duration::days(3);
                Utc.with_ymd_and_hms(later.year(), later.month(), later.day(), 9, 0, 0)
                    .single()
                    .unwrap_or(later)
            }
        };
        let next_today = self.next_today_sort_index();
        let clear_today = matches!(option, TaskPostponeOption::Tomorrow | TaskPostponeOption::ThisWeek);
        let today = local_date_str();
        self.mutate_task(id, move |task| {
            task.due_at = Some(due_at);
            task.reminder_at = Some(due_at);
            task.postponed_at = Some(now);
            task.set_postpone_count(task.postpone_count() + 1);
            if same_day(due_at, now) {
                task.today_sort_index = Some(task.today_sort_index.unwrap_or(next_today));
                task.today_added_date = Some(today);
            } else if clear_today {
                task.today_sort_index = None;
                task.today_added_date = None;
            }
            task.updated_at = now;
        })
    }

    pub fn daily_review(&self, now: DateTime<Utc>) -> TaskDailyReview {
        let tomorrow = now + Duration::days(1);
        TaskDailyReview {
            completed_today: self
                .completed_tasks()
                .into_iter()
                .filter(|t| t.completed_at.map(|c| same_day(c, now)).unwrap_or(false))
                .collect(),
            postponed_today: self
                .incomplete_tasks()
                .into_iter()
                .filter(|t| t.postponed_at.map(|p| same_day(p, now)).unwrap_or(false))
                .collect(),
            tomorrow_tasks: self
                .incomplete_tasks()
                .into_iter()
                .filter(|t| t.due_at.map(|d| same_day(d, tomorrow)).unwrap_or(false))
                .collect(),
            focus_seconds: self
                .tasks
                .iter()
                .map(|t| self.focus_seconds(t.id, now))
                .sum(),
        }
    }

    pub fn suggested_today_tasks(&self, limit: usize, now: DateTime<Utc>) -> Vec<TaskItem> {
        let end_of_today = Utc
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 23, 59, 59)
            .single()
            .unwrap_or(now)
            + Duration::seconds(1);
        let mut tasks = self.incomplete_tasks();
        tasks.sort_by(|lhs, rhs| {
            let ls = self.suggestion_score(lhs, now, end_of_today);
            let rs = self.suggestion_score(rhs, now, end_of_today);
            rs.cmp(&ls)
                .then_with(|| match (lhs.due_at, rhs.due_at) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    _ => lhs.sort_index.cmp(&rhs.sort_index),
                })
        });
        tasks.into_iter().take(limit).collect()
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags = self.all_tag_suggestions();
        tags.truncate(10);
        tags
    }

    pub fn all_tag_suggestions(&self) -> Vec<String> {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for task in &self.tasks {
            for tag in task.tags() {
                *counts.entry(tag).or_insert(0) += 1;
            }
        }
        let mut tags: Vec<String> = counts.keys().cloned().collect();
        tags.sort_by(|a, b| {
            counts[b].cmp(&counts[a])
                .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
        });
        tags
    }

    pub fn all_projects(&self) -> Vec<String> {
        let mut projects: Vec<String> = self
            .tasks
            .iter()
            .filter_map(|t| t.project_name.as_ref().map(|p| p.trim().to_string()))
            .filter(|p| !p.is_empty())
            .collect();
        projects.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        projects.dedup();
        projects
    }

    pub fn upcoming_tasks(&self) -> Vec<TaskItem> {
        let mut tasks: Vec<TaskItem> = self
            .incomplete_tasks()
            .into_iter()
            .filter(|t| t.due_at.is_some())
            .collect();
        tasks.sort_by(|lhs, rhs| {
            lhs.due_at
                .cmp(&rhs.due_at)
                .then_with(|| lhs.priority_raw_value.cmp(&rhs.priority_raw_value))
        });
        tasks
    }

    pub fn incomplete_tasks_tagged(&self, tag: &str) -> Vec<TaskItem> {
        self.sorted_by_priority(
            self.incomplete_tasks()
                .into_iter()
                .filter(|t| t.tags().iter().any(|x| x.eq_ignore_ascii_case(tag)))
                .collect(),
        )
    }

    pub fn incomplete_tasks_in_project(&self, project: &str) -> Vec<TaskItem> {
        self.sorted_by_priority(
            self.incomplete_tasks()
                .into_iter()
                .filter(|t| {
                    t.project_name
                        .as_ref()
                        .map(|p| p.eq_ignore_ascii_case(project))
                        .unwrap_or(false)
                })
                .collect(),
        )
    }

    pub fn tasks_matching(&self, query: &str) -> Vec<TaskItem> {
        let cleaned = query.trim();
        if cleaned.is_empty() {
            return self.incomplete_tasks();
        }
        let q = cleaned.to_lowercase();
        self.sorted_by_priority(
            self.incomplete_tasks()
                .into_iter()
                .filter(|t| {
                    t.title.to_lowercase().contains(&q)
                        || t.notes.to_lowercase().contains(&q)
                        || t.tags().iter().any(|tag| tag.to_lowercase().contains(&q))
                        || t
                            .project_name
                            .as_ref()
                            .map(|p| p.to_lowercase().contains(&q))
                            .unwrap_or(false)
                })
                .collect(),
        )
    }

    pub fn export_tasks(&self, path: &Path, format: &str) -> Result<(), String> {
        let data = match format {
            "csv" => self.csv_export_text(),
            "md" | "markdown" => self.markdown_export_text(),
            _ => serde_json::to_string_pretty(&TaskArchive {
                version: 2,
                exported_at: Utc::now(),
                tasks: self.tasks.iter().map(TaskArchiveItem::from).collect(),
            })
            .map_err(|e| e.to_string())?,
        };
        std::fs::write(path, data).map_err(|e| e.to_string())
    }

    pub fn import_tasks(&mut self, path: &Path) -> Result<usize, String> {
        if path.extension().and_then(|s| s.to_str()).unwrap_or("").eq_ignore_ascii_case("csv") {
            return self.import_csv_tasks(path);
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let archive: TaskArchive = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let mut imported = 0usize;
        for item in archive.tasks {
            if self.tasks.iter().any(|t| t.id == item.id) {
                self.apply_archive_item(&item)?;
            } else {
                let mut task = self.task_from_archive(item);
                self.insert_task(&mut task)?;
                imported += 1;
            }
        }
        self.normalize_current_task()?;
        Ok(imported)
    }

    pub fn import_csv_tasks(&mut self, path: &Path) -> Result<usize, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let rows = parse_csv(&text);
        let Some(header) = rows.first() else {
            return Ok(0);
        };
        let normalized: Vec<String> = header.iter().map(|k| normalize_csv_key(k)).collect();
        let mut imported = 0usize;
        for row in rows.iter().skip(1) {
            let mut values = HashMap::new();
            for (idx, key) in normalized.iter().enumerate() {
                if !key.is_empty() {
                    values.insert(key.clone(), row.get(idx).cloned().unwrap_or_default());
                }
            }
            let Some(item) = archive_item_from_csv(&values) else {
                continue;
            };
            if self.tasks.iter().any(|t| t.id == item.id) {
                self.apply_archive_item(&item)?;
            } else {
                let mut task = self.task_from_archive(item);
                self.insert_task(&mut task)?;
                imported += 1;
            }
        }
        self.normalize_current_task()?;
        Ok(imported)
    }

    // --- internal helpers ---

    fn new_task_item(
        &self,
        title: String,
        notes: String,
        is_completed: bool,
        is_current: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        completed_at: Option<DateTime<Utc>>,
        priority: TaskPriority,
        due_at: Option<DateTime<Utc>>,
        reminder_at: Option<DateTime<Utc>>,
        repeat_rule: Option<TaskRepeatRule>,
        tags: Vec<String>,
        project_name: Option<String>,
        estimated_minutes: Option<i32>,
        today_sort_index: Option<i32>,
        subtasks: Vec<TaskSubtask>,
    ) -> TaskItem {
        let mut item = TaskItem {
            id: Uuid::new_v4(),
            title,
            notes,
            is_completed,
            is_current,
            created_at,
            updated_at,
            completed_at,
            sort_index: self.next_sort_index(),
            priority_raw_value: Some(priority as i32),
            due_at,
            reminder_at,
            repeat_rule_raw_value: repeat_rule.map(|r| r.as_str().to_string()),
            tags_raw_value: None,
            project_name,
            estimated_minutes,
            today_sort_index,
            today_added_date: today_sort_index.map(|_| local_date_str()),
            subtasks_raw_value: None,
            focus_started_at: None,
            focus_accumulated_seconds: Some(0.0),
            postponed_at: None,
            postpone_count_raw_value: Some(0),
        };
        item.set_tags(tags);
        item.set_subtasks(subtasks);
        item
    }

    fn insert_task(&mut self, item: &mut TaskItem) -> Result<(), String> {
        if item.is_current {
            self.clear_current_flags()?;
        }
        self.upsert_task(item)?;
        self.reload_tasks()
    }

    fn mutate_task<F>(&mut self, id: Uuid, mutator: F) -> Result<(), String>
    where
        F: FnOnce(&mut TaskItem),
    {
        let mut task = self
            .tasks
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or("task not found")?;
        mutator(&mut task);
        self.upsert_task(&task)?;
        self.reload_tasks()
    }

    fn commit_reload(&mut self) -> Result<(), String> {
        self.reload_tasks()
    }

    fn clear_current_flags(&mut self) -> Result<(), String> {
        for task in self.incomplete_tasks() {
            if task.is_current {
                self.mutate_task(task.id, |t| {
                    t.is_current = false;
                    t.updated_at = Utc::now();
                })?;
            }
        }
        Ok(())
    }



    fn normalize_current_task(&mut self) -> Result<(), String> {
        let active = self.incomplete_tasks();
        let stale_current: Vec<Uuid> = self
            .tasks
            .iter()
            .filter(|t| t.is_current && (t.is_completed || active.is_empty()))
            .map(|t| t.id)
            .collect();
        for id in stale_current {
            self.mutate_task(id, |t| {
                t.is_current = false;
                t.updated_at = Utc::now();
            })?;
        }
        let current_ids: Vec<Uuid> = active.iter().filter(|t| t.is_current).map(|t| t.id).collect();
        for id in current_ids.iter().skip(1) {
            self.mutate_task(*id, |t| {
                t.is_current = false;
                t.updated_at = Utc::now();
            })?;
        }
        if let Some(id) = self.focus_attention_task_id {
            if !active.iter().any(|t| t.id == id) {
                self.focus_attention_task_id = None;
            }
        }
        Ok(())
    }

    fn suggestion_score(&self, task: &TaskItem, now: DateTime<Utc>, end_of_today: DateTime<Utc>) -> i32 {
        let mut score = 0;
        if task.is_in_today_queue() {
            score += 80;
        }
        if task.is_current {
            score += 30;
        }
        match task.priority() {
            TaskPriority::High => score += 30,
            TaskPriority::Medium => score += 16,
            TaskPriority::Low => score += 6,
        }
        if let Some(due) = task.due_at {
            if due < now {
                score += 70;
            } else if due < end_of_today {
                score += 55;
            } else if (due - now).num_seconds() < 3 * 24 * 60 * 60 {
                score += 24;
            }
        }
        if let Some(minutes) = task.estimated_minutes {
            if minutes <= 30 {
                score += 10;
            } else if minutes <= 60 {
                score += 6;
            }
        }
        score
    }

    fn next_sort_index(&self) -> i32 {
        self.tasks.iter().map(|t| t.sort_index).max().unwrap_or(-1) + 1
    }

    fn next_today_sort_index(&self) -> i32 {
        self.tasks
            .iter()
            .filter_map(|t| t.today_sort_index)
            .max()
            .unwrap_or(-1)
            + 1
    }

    fn sorted(&self, mut items: Vec<TaskItem>) -> Vec<TaskItem> {
        items.sort_by(|a, b| a.sort_index.cmp(&b.sort_index).then(a.created_at.cmp(&b.created_at)));
        items
    }

    fn sorted_by_priority(&self, mut items: Vec<TaskItem>) -> Vec<TaskItem> {
        items.sort_by(|lhs, rhs| {
            lhs.is_in_today_queue()
                .cmp(&rhs.is_in_today_queue())
                .reverse()
                .then(lhs.priority_raw_value.cmp(&rhs.priority_raw_value))
                .then(match (lhs.due_at, rhs.due_at) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                })
                .then(lhs.is_current.cmp(&rhs.is_current).reverse())
                .then(lhs.sort_index.cmp(&rhs.sort_index))
                .then(lhs.created_at.cmp(&rhs.created_at))
        });
        items
    }

    fn sorted_today(&self, mut items: Vec<TaskItem>) -> Vec<TaskItem> {
        items.sort_by(|lhs, rhs| {
            lhs.today_sort_index
                .cmp(&rhs.today_sort_index)
                .then(match (lhs.due_at, rhs.due_at) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    _ => std::cmp::Ordering::Equal,
                })
                .then(lhs.priority_raw_value.cmp(&rhs.priority_raw_value))
                .then(lhs.sort_index.cmp(&rhs.sort_index))
        });
        items
    }

    fn make_next_recurring_task(&self, task: &TaskItem) -> Option<TaskItem> {
        let rule = task.repeat_rule()?;
        let due = task.due_at?;
        let next_due = self.next_date_after(due, rule)?;
        let mut next = self.new_task_item(
            task.title.clone(),
            task.notes.clone(),
            false,
            false,
            Utc::now(),
            Utc::now(),
            None,
            task.priority(),
            Some(next_due),
            if task.reminder_at.is_some() { Some(next_due) } else { None },
            Some(rule),
            task.tags(),
            task.project_name.clone(),
            task.estimated_minutes,
            if same_day(next_due, Utc::now()) {
                Some(self.next_today_sort_index())
            } else {
                None
            },
            task.subtasks()
                .into_iter()
                .map(|s| TaskSubtask::new(s.title))
                .collect(),
        );
        Some(next)
    }

    fn next_date_after(&self, date: DateTime<Utc>, rule: TaskRepeatRule) -> Option<DateTime<Utc>> {
        match rule {
            TaskRepeatRule::Daily => Some(date + Duration::days(1)),
            TaskRepeatRule::Weekly => Some(date + Duration::weeks(1)),
            TaskRepeatRule::Monthly => {
                let month = if date.month() == 12 { 1 } else { date.month() + 1 };
                let year = if date.month() == 12 { date.year() + 1 } else { date.year() };
                Utc.with_ymd_and_hms(year, month, date.day(), date.hour(), date.minute(), date.second())
                    .single()
            }
            TaskRepeatRule::Yearly => Utc
                .with_ymd_and_hms(date.year() + 1, date.month(), date.day(), date.hour(), date.minute(), date.second())
                .single(),
        }
    }

    fn markdown_export_text(&self) -> String {
        let mut lines = vec!["# TaskCap 导出".to_string(), String::new()];
        lines.push(format!("导出时间：{}", Utc::now().to_rfc3339()));
        lines.push(String::new());
        self.append_md_section(&mut lines, "未完成", &self.incomplete_tasks());
        self.append_md_section(&mut lines, "已完成", &self.completed_tasks());
        lines.join("\n")
    }

    fn append_md_section(&self, lines: &mut Vec<String>, title: &str, tasks: &[TaskItem]) {
        lines.push(format!("## {title}"));
        lines.push(String::new());
        if tasks.is_empty() {
            lines.push("- 无".to_string());
            lines.push(String::new());
            return;
        }
        for task in tasks {
            let mark = if task.is_completed { "x" } else { " " };
            let mut meta = vec![task.priority().title().to_string()];
            if let Some(due) = task.due_at {
                meta.push(format!("截止 {}", due.to_rfc3339()));
            }
            if let Some(project) = &task.project_name {
                meta.push(format!("+{project}"));
            }
            for tag in task.tags() {
                meta.push(format!("#{tag}"));
            }
            if let Some(minutes) = task.estimated_minutes {
                meta.push(format!("{minutes}分钟"));
            }
            lines.push(format!(
                "- [{mark}] {} _{}_",
                task.title,
                meta.join(" · ")
            ));
        }
        lines.push(String::new());
    }

    fn csv_export_text(&self) -> String {
        let header = [
            "id", "title", "notes", "completed", "current", "priority", "dueAt", "reminderAt",
            "repeat", "tags", "project", "estimatedMinutes", "today", "subtasks", "focusSeconds",
            "postponeCount",
        ];
        let mut rows = vec![header.join(",")];
        for task in &self.tasks {
            let row = [
                task.id.to_string(),
                csv_escape(&task.title),
                csv_escape(&task.notes),
                task.is_completed.to_string(),
                task.is_current.to_string(),
                task.priority().short_title().to_string(),
                task.due_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
                task.reminder_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
                task.repeat_rule().map(|r| r.title().to_string()).unwrap_or_default(),
                task.tags().join("|"),
                task.project_name.clone().unwrap_or_default(),
                task.estimated_minutes.map(|v| v.to_string()).unwrap_or_default(),
                task.is_in_today_queue().to_string(),
                task
                    .subtasks()
                    .iter()
                    .map(|s| format!("{}:{}", if s.is_completed { "x" } else { " " }, s.title))
                    .collect::<Vec<_>>()
                    .join("|"),
                (task.focus_seconds().round() as i64).to_string(),
                task.postpone_count().to_string(),
            ];
            rows.push(row.join(","));
        }
        rows.join("\n") + "\n"
    }

    fn task_from_archive(&self, item: TaskArchiveItem) -> TaskItem {
        let mut task = TaskItem {
            id: item.id,
            title: item.title,
            notes: item.notes,
            is_completed: item.is_completed,
            is_current: item.is_current,
            created_at: item.created_at,
            updated_at: item.updated_at,
            completed_at: item.completed_at,
            sort_index: item.sort_index,
            priority_raw_value: item.priority_raw_value,
            due_at: item.due_at,
            reminder_at: item.reminder_at,
            repeat_rule_raw_value: item.repeat_rule_raw_value,
            tags_raw_value: None,
            project_name: item.project_name,
            estimated_minutes: item.estimated_minutes,
            today_sort_index: item.today_sort_index,
            today_added_date: item.today_sort_index.map(|_| local_date_str()),
            subtasks_raw_value: None,
            focus_started_at: item.focus_started_at,
            focus_accumulated_seconds: Some(item.focus_accumulated_seconds),
            postponed_at: item.postponed_at,
            postpone_count_raw_value: Some(item.postpone_count),
        };
        task.set_tags(item.tags);
        task.set_subtasks(item.subtasks);
        task
    }

    fn apply_archive_item(&mut self, item: &TaskArchiveItem) -> Result<(), String> {
        let mut task = self.task_from_archive(item.clone());
        self.upsert_task(&task)?;
        task.id = item.id;
        Ok(())
    }

    pub fn reload_tasks(&mut self) -> Result<(), String> {
        // 清除非今天加入的「今天」标记（跨日自动过期）
        let today = local_date_str();
        let _ = self.conn.execute(
            "UPDATE tasks SET today_sort_index = NULL, today_added_date = NULL
             WHERE today_sort_index IS NOT NULL
               AND (today_added_date IS NULL OR today_added_date != ?1)",
            rusqlite::params![today],
        );

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, notes, is_completed, is_current, created_at, updated_at, completed_at,
                 sort_index, priority_raw_value, due_at, reminder_at, repeat_rule_raw_value, tags_raw_value,
                 project_name, estimated_minutes, today_sort_index, today_added_date, subtasks_raw_value,
                 focus_started_at, focus_accumulated_seconds, postponed_at, postpone_count_raw_value
                 FROM tasks ORDER BY sort_index ASC, created_at ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(TaskItem {
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_else(|_| Uuid::new_v4()),
                    title: row.get(1)?,
                    notes: row.get(2)?,
                    is_completed: row.get::<_, i32>(3)? != 0,
                    is_current: row.get::<_, i32>(4)? != 0,
                    created_at: parse_dt(row.get(5)?)?,
                    updated_at: parse_dt(row.get(6)?)?,
                    completed_at: parse_opt_dt(row.get(7)?)?,
                    sort_index: row.get(8)?,
                    priority_raw_value: row.get(9)?,
                    due_at: parse_opt_dt(row.get(10)?)?,
                    reminder_at: parse_opt_dt(row.get(11)?)?,
                    repeat_rule_raw_value: row.get(12)?,
                    tags_raw_value: row.get(13)?,
                    project_name: row.get(14)?,
                    estimated_minutes: row.get(15)?,
                    today_sort_index: row.get(16)?,
                    today_added_date: row.get(17)?,
                    subtasks_raw_value: row.get(18)?,
                    focus_started_at: parse_opt_dt(row.get(19)?)?,
                    focus_accumulated_seconds: row.get(20)?,
                    postponed_at: parse_opt_dt(row.get(21)?)?,
                    postpone_count_raw_value: row.get(22)?,
                })
            })
            .map_err(|e| e.to_string())?;
        self.tasks = rows.filter_map(|r| r.ok()).collect();
        self.last_error = None;
        Ok(())
    }

    fn upsert_task(&self, task: &TaskItem) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO tasks (
                    id, title, notes, is_completed, is_current, created_at, updated_at, completed_at,
                    sort_index, priority_raw_value, due_at, reminder_at, repeat_rule_raw_value, tags_raw_value,
                    project_name, estimated_minutes, today_sort_index, today_added_date, subtasks_raw_value,
                    focus_started_at, focus_accumulated_seconds, postponed_at, postpone_count_raw_value
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)
                ON CONFLICT(id) DO UPDATE SET
                    title=excluded.title, notes=excluded.notes, is_completed=excluded.is_completed,
                    is_current=excluded.is_current, created_at=excluded.created_at, updated_at=excluded.updated_at,
                    completed_at=excluded.completed_at, sort_index=excluded.sort_index,
                    priority_raw_value=excluded.priority_raw_value, due_at=excluded.due_at,
                    reminder_at=excluded.reminder_at, repeat_rule_raw_value=excluded.repeat_rule_raw_value,
                    tags_raw_value=excluded.tags_raw_value, project_name=excluded.project_name,
                    estimated_minutes=excluded.estimated_minutes, today_sort_index=excluded.today_sort_index,
                    today_added_date=excluded.today_added_date, subtasks_raw_value=excluded.subtasks_raw_value,
                    focus_started_at=excluded.focus_started_at,
                    focus_accumulated_seconds=excluded.focus_accumulated_seconds,
                    postponed_at=excluded.postponed_at, postpone_count_raw_value=excluded.postpone_count_raw_value",
                params![
                    task.id.to_string(),
                    task.title,
                    task.notes,
                    if task.is_completed { 1 } else { 0 },
                    if task.is_current { 1 } else { 0 },
                    task.created_at.to_rfc3339(),
                    task.updated_at.to_rfc3339(),
                    opt_dt(task.completed_at),
                    task.sort_index,
                    task.priority_raw_value,
                    opt_dt(task.due_at),
                    opt_dt(task.reminder_at),
                    task.repeat_rule_raw_value,
                    task.tags_raw_value,
                    task.project_name,
                    task.estimated_minutes,
                    task.today_sort_index,
                    task.today_added_date,
                    task.subtasks_raw_value,
                    opt_dt(task.focus_started_at),
                    task.focus_accumulated_seconds,
                    opt_dt(task.postponed_at),
                    task.postpone_count_raw_value,
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn stop_focus_clock(task: &mut TaskItem, now: DateTime<Utc>) {
    if let Some(started) = task.focus_started_at {
        task.set_focus_seconds(task.focus_seconds() + (now - started).num_seconds().max(0) as f64);
        task.focus_started_at = None;
        task.updated_at = now;
    }
}

fn same_day(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    a.date_naive() == b.date_naive()
}

fn local_date_str() -> String {
    chrono::Local::now().date_naive().to_string()
}

fn opt_dt(value: Option<DateTime<Utc>>) -> Option<String> {
    value.map(|d| d.to_rfc3339())
}

fn parse_dt(value: String) -> Result<DateTime<Utc>, rusqlite::Error> {
    DateTime::parse_from_rfc3339(&value)
        .map(|d| d.with_timezone(&Utc))
        .or_else(|_| {
            value
                .parse::<DateTime<Utc>>()
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))
        })
}

fn parse_opt_dt(value: Option<String>) -> Result<Option<DateTime<Utc>>, rusqlite::Error> {
    match value {
        Some(v) if !v.is_empty() => Ok(Some(parse_dt(v)?)),
        _ => Ok(None),
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn normalize_csv_key(key: &str) -> String {
    key.trim()
        .to_lowercase()
        .replace([' ', '_', '-'], "")
}

fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                row.push(field.clone());
                field.clear();
            }
            '\n' | '\r' if !in_quotes => {
                if ch == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(field.clone());
                field.clear();
                if row.iter().any(|c| !c.is_empty()) {
                    rows.push(row.clone());
                }
                row.clear();
            }
            other => field.push(other),
        }
    }
    row.push(field);
    if row.iter().any(|c| !c.is_empty()) {
        rows.push(row);
    }
    rows
}

fn archive_item_from_csv(values: &HashMap<String, String>) -> Option<TaskArchiveItem> {
    let title = first_csv_value(values, &["title", "content", "taskname", "name", "任务", "标题"]);
    if title.trim().is_empty() {
        return None;
    }
    let now = Utc::now();
    let id = first_csv_value(values, &["id", "uuid"])
        .parse::<Uuid>()
        .unwrap_or_else(|_| Uuid::new_v4());
    let notes = first_csv_value(values, &["notes", "description", "备注", "描述"]);
    let is_completed = csv_bool(first_csv_value(values, &["completed", "complete", "done", "已完成"]));
    let is_current = csv_bool(first_csv_value(values, &["current", "iscurrent", "当前"]));
    let created_at = csv_date(first_csv_value(values, &["createdat", "created", "创建时间"])).unwrap_or(now);
    let updated_at = csv_date(first_csv_value(values, &["updatedat", "updated", "更新时间"])).unwrap_or(now);
    let completed_at = csv_date(first_csv_value(values, &["completedat", "completeddate", "完成时间"]))
        .or(if is_completed { Some(now) } else { None });
    let priority = csv_priority(first_csv_value(values, &["priority", "优先级"]));
    let due_at = csv_date(first_csv_value(values, &["dueat", "duedate", "date", "到期", "截止"]));
    let reminder_at =
        csv_date(first_csv_value(values, &["reminderat", "reminder", "提醒"])).or(due_at);
    let repeat_rule = csv_repeat(first_csv_value(values, &["repeat", "recurring", "重复"]));
    let tags = csv_tags(first_csv_value(values, &["tags", "labels", "label", "标签"]));
    let project = first_csv_value(values, &["project", "projectname", "list", "section", "项目", "清单"]);
    let estimated_minutes = csv_int(first_csv_value(values, &["estimatedminutes", "duration", "预计分钟"]));
    let today = csv_bool(first_csv_value(values, &["today", "myday", "今天"]));
    let subtasks = csv_subtasks(first_csv_value(values, &["subtasks", "steps", "子任务"]));
    let focus_seconds = csv_int(first_csv_value(values, &["focusseconds", "专注秒数"])).unwrap_or(0) as f64;
    let postpone_count = csv_int(first_csv_value(values, &["postponecount", "推迟次数"])).unwrap_or(0);
    Some(TaskArchiveItem {
        id,
        title: title.to_string(),
        notes: notes.to_string(),
        is_completed,
        is_current,
        created_at,
        updated_at,
        completed_at,
        sort_index: csv_int(first_csv_value(values, &["sortindex", "order", "排序"])).unwrap_or(0),
        priority_raw_value: Some(priority as i32),
        due_at,
        reminder_at,
        repeat_rule_raw_value: repeat_rule.map(|r| r.as_str().to_string()),
        tags,
        project_name: if project.is_empty() { None } else { Some(project.to_string()) },
        estimated_minutes,
        today_sort_index: if today { Some(0) } else { None },
        subtasks,
        focus_started_at: None,
        focus_accumulated_seconds: focus_seconds,
        postponed_at: None,
        postpone_count,
    })
}

fn first_csv_value<'a>(values: &'a HashMap<String, String>, keys: &[&str]) -> &'a str {
    for key in keys {
        let normalized = normalize_csv_key(key);
        if let Some(value) = values.get(&normalized) {
            if !value.trim().is_empty() {
                return value;
            }
        }
    }
    ""
}

fn csv_bool(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "true" | "yes" | "1" | "x" | "done" | "completed" | "已完成" | "是"
    )
}

fn csv_int(value: &str) -> Option<i32> {
    value.trim().parse().ok()
}

fn csv_date(value: &str) -> Option<DateTime<Utc>> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(cleaned) {
        return Some(dt.with_timezone(&Utc));
    }
    for fmt in ["%Y-%m-%d %H:%M", "%Y/%m/%d %H:%M", "%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(cleaned, fmt) {
            return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
        }
        if let Ok(date) = chrono::NaiveDate::parse_from_str(cleaned, fmt) {
            if let Some(naive) = date.and_hms_opt(0, 0, 0) {
                return Some(DateTime::from_naive_utc_and_offset(naive, Utc));
            }
        }
    }
    None
}

fn csv_priority(value: &str) -> TaskPriority {
    let n = value.trim().to_lowercase();
    if n.contains('高') || n.contains("high") || n == "p1" || n == "1" || n == "4" {
        TaskPriority::High
    } else if n.contains('低') || n.contains("low") || n == "p3" || n == "3" {
        TaskPriority::Low
    } else {
        TaskPriority::Medium
    }
}

fn csv_repeat(value: &str) -> Option<TaskRepeatRule> {
    let n = value.trim().to_lowercase();
    if n.contains("每天") || n.contains("daily") {
        Some(TaskRepeatRule::Daily)
    } else if n.contains("每周") || n.contains("weekly") {
        Some(TaskRepeatRule::Weekly)
    } else if n.contains("每月") || n.contains("monthly") {
        Some(TaskRepeatRule::Monthly)
    } else if n.contains("每年") || n.contains("yearly") || n.contains("annually") {
        Some(TaskRepeatRule::Yearly)
    } else {
        None
    }
}

fn csv_tags(value: &str) -> Vec<String> {
    value
        .split(|c| c == '|' || c == ';' || c == ' ' || c == '#')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn csv_subtasks(value: &str) -> Vec<TaskSubtask> {
    value
        .split('|')
        .filter_map(|raw| {
            let text = raw.trim();
            if text.is_empty() {
                return None;
            }
            let (done, title) = if let Some(rest) = text.strip_prefix("x:") {
                (true, rest)
            } else if let Some(rest) = text.strip_prefix(" :") {
                (false, rest)
            } else {
                (false, text)
            };
            let title = title.trim();
            if title.is_empty() {
                None
            } else {
                Some(TaskSubtask {
                    id: Uuid::new_v4(),
                    title: title.to_string(),
                    is_completed: done,
                    created_at: Utc::now(),
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod checks;
