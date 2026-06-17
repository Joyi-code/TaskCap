use rusqlite::Connection;
use std::path::PathBuf;

pub const SCHEMA_SQL: &str = r#"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT '',
    is_completed INTEGER NOT NULL DEFAULT 0,
    is_current INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    sort_index INTEGER NOT NULL DEFAULT 0,
    priority_raw_value INTEGER,
    due_at TEXT,
    reminder_at TEXT,
    repeat_rule_raw_value TEXT,
    tags_raw_value TEXT,
    project_name TEXT,
    estimated_minutes INTEGER,
    today_sort_index INTEGER,
    today_added_date TEXT,
    subtasks_raw_value TEXT,
    focus_started_at TEXT,
    focus_accumulated_seconds REAL,
    postponed_at TEXT,
    postpone_count_raw_value INTEGER
);
"#;

/// SQLite 存储路径：%APPDATA%/taskcap/taskcap.db
pub fn database_path() -> Result<PathBuf, String> {
    let appdata = std::env::var_os("APPDATA").ok_or("APPDATA is not available")?;
    Ok(PathBuf::from(appdata)
        .join("taskcap")
        .join("taskcap.db"))
}

pub fn open_connection(path: Option<&PathBuf>) -> Result<Connection, String> {
    let conn = match path {
        Some(p) => {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            Connection::open(p).map_err(|e| e.to_string())?
        }
        None => Connection::open_in_memory().map_err(|e| e.to_string())?,
    };
    conn.execute_batch(SCHEMA_SQL).map_err(|e| e.to_string())?;
    // 兼容旧库：忽略"列已存在"错误
    let _ = conn.execute("ALTER TABLE tasks ADD COLUMN today_added_date TEXT", []);
    Ok(conn)
}