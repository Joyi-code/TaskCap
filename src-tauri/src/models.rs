use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskPriority {
    High = 0,
    Medium = 1,
    Low = 2,
}

impl TaskPriority {
    pub fn from_raw(value: Option<i32>) -> Self {
        match value {
            Some(0) => Self::High,
            Some(2) => Self::Low,
            _ => Self::Medium,
        }
    }

    pub fn short_title(&self) -> &'static str {
        match self {
            Self::High => "高",
            Self::Medium => "中",
            Self::Low => "低",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::High => "高优先级",
            Self::Medium => "中优先级",
            Self::Low => "低优先级",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskRepeatRule {
    #[serde(rename = "daily")]
    Daily,
    #[serde(rename = "weekly")]
    Weekly,
    #[serde(rename = "monthly")]
    Monthly,
    #[serde(rename = "yearly")]
    Yearly,
}

impl TaskRepeatRule {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "daily" | "每天" | "每日" => Some(Self::Daily),
            "weekly" | "每周" | "每星期" => Some(Self::Weekly),
            "monthly" | "每月" => Some(Self::Monthly),
            "yearly" | "每年" => Some(Self::Yearly),
            _ => None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Daily => "每天",
            Self::Weekly => "每周",
            Self::Monthly => "每月",
            Self::Yearly => "每年",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPostponeOption {
    FifteenMinutes,
    LaterToday,
    Tomorrow,
    ThisWeek,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSubtask {
    pub id: Uuid,
    pub title: String,
    pub is_completed: bool,
    pub created_at: DateTime<Utc>,
}

impl TaskSubtask {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            is_completed: false,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: Uuid,
    pub title: String,
    pub notes: String,
    pub is_completed: bool,
    pub is_current: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub sort_index: i32,
    pub priority_raw_value: Option<i32>,
    pub due_at: Option<DateTime<Utc>>,
    pub reminder_at: Option<DateTime<Utc>>,
    pub repeat_rule_raw_value: Option<String>,
    pub tags_raw_value: Option<String>,
    pub project_name: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub today_sort_index: Option<i32>,
    pub today_added_date: Option<String>,
    pub subtasks_raw_value: Option<String>,
    pub focus_started_at: Option<DateTime<Utc>>,
    pub focus_accumulated_seconds: Option<f64>,
    pub postponed_at: Option<DateTime<Utc>>,
    pub postpone_count_raw_value: Option<i32>,
}

impl TaskItem {
    pub fn priority(&self) -> TaskPriority {
        TaskPriority::from_raw(self.priority_raw_value)
    }

    pub fn set_priority(&mut self, priority: TaskPriority) {
        self.priority_raw_value = Some(priority as i32);
        self.updated_at = Utc::now();
    }

    pub fn repeat_rule(&self) -> Option<TaskRepeatRule> {
        self.repeat_rule_raw_value
            .as_deref()
            .and_then(TaskRepeatRule::from_str)
    }

    pub fn set_repeat_rule(&mut self, rule: Option<TaskRepeatRule>) {
        self.repeat_rule_raw_value = rule.map(|r| r.as_str().to_string());
        self.updated_at = Utc::now();
    }

    pub fn tags(&self) -> Vec<String> {
        self.tags_raw_value
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }

    pub fn set_tags(&mut self, tags: Vec<String>) {
        let joined = tags
            .into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        self.tags_raw_value = if joined.is_empty() {
            None
        } else {
            Some(joined)
        };
        self.updated_at = Utc::now();
    }

    pub fn subtasks(&self) -> Vec<TaskSubtask> {
        let raw = match &self.subtasks_raw_value {
            Some(v) => v,
            None => return vec![],
        };
        serde_json::from_str(raw).unwrap_or_default()
    }

    pub fn set_subtasks(&mut self, subtasks: Vec<TaskSubtask>) {
        let cleaned: Vec<TaskSubtask> = subtasks
            .into_iter()
            .filter(|s| !s.title.trim().is_empty())
            .collect();
        self.subtasks_raw_value = if cleaned.is_empty() {
            None
        } else {
            serde_json::to_string(&cleaned).ok()
        };
        self.updated_at = Utc::now();
    }

    pub fn is_in_today_queue(&self) -> bool {
        if self.today_sort_index.is_none() {
            return false;
        }
        match &self.today_added_date {
            Some(date) => *date == Local::now().date_naive().to_string(),
            None => false,
        }
    }

    pub fn focus_seconds(&self) -> f64 {
        self.focus_accumulated_seconds.unwrap_or(0.0)
    }

    pub fn set_focus_seconds(&mut self, value: f64) {
        self.focus_accumulated_seconds = Some(value.max(0.0));
        self.updated_at = Utc::now();
    }

    pub fn postpone_count(&self) -> i32 {
        self.postpone_count_raw_value.unwrap_or(0)
    }

    pub fn set_postpone_count(&mut self, value: i32) {
        self.postpone_count_raw_value = Some(value.max(0));
        self.updated_at = Utc::now();
    }

    pub fn completed_subtask_count(&self) -> usize {
        self.subtasks().iter().filter(|s| s.is_completed).count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCounts {
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArchive {
    pub version: i32,
    pub exported_at: DateTime<Utc>,
    pub tasks: Vec<TaskArchiveItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArchiveItem {
    pub id: Uuid,
    pub title: String,
    pub notes: String,
    pub is_completed: bool,
    pub is_current: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub sort_index: i32,
    pub priority_raw_value: Option<i32>,
    pub due_at: Option<DateTime<Utc>>,
    pub reminder_at: Option<DateTime<Utc>>,
    pub repeat_rule_raw_value: Option<String>,
    pub tags: Vec<String>,
    pub project_name: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub today_sort_index: Option<i32>,
    pub subtasks: Vec<TaskSubtask>,
    pub focus_started_at: Option<DateTime<Utc>>,
    pub focus_accumulated_seconds: f64,
    pub postponed_at: Option<DateTime<Utc>>,
    pub postpone_count: i32,
}

impl From<&TaskItem> for TaskArchiveItem {
    fn from(task: &TaskItem) -> Self {
        Self {
            id: task.id,
            title: task.title.clone(),
            notes: task.notes.clone(),
            is_completed: task.is_completed,
            is_current: task.is_current,
            created_at: task.created_at,
            updated_at: task.updated_at,
            completed_at: task.completed_at,
            sort_index: task.sort_index,
            priority_raw_value: task.priority_raw_value,
            due_at: task.due_at,
            reminder_at: task.reminder_at,
            repeat_rule_raw_value: task.repeat_rule_raw_value.clone(),
            tags: task.tags(),
            project_name: task.project_name.clone(),
            estimated_minutes: task.estimated_minutes,
            today_sort_index: task.today_sort_index,
            subtasks: task.subtasks(),
            focus_started_at: task.focus_started_at,
            focus_accumulated_seconds: task.focus_seconds(),
            postponed_at: task.postponed_at,
            postpone_count: task.postpone_count(),
        }
    }
}