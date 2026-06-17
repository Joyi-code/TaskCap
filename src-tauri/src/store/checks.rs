use super::*;
use crate::models::TaskPriority;
use crate::parser::parse;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;
use tempfile::NamedTempFile;

fn assert_true(cond: bool, msg: &str) {
    assert!(cond, "{msg}");
}

#[test]
fn taskcap_checks() {
    // CRUD + current task rules
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store.add_task("Draft launch notes", "", TaskPriority::Medium).unwrap().unwrap();
        assert_true(store.incomplete_count() == 1, "First task was not added");
        assert_true(store.current_task().is_none(), "New tasks should not become current automatically");
        assert_eq!(store.menu_bar_title(), "暂无当前任务");
        assert_eq!(task.priority(), TaskPriority::Medium);
        store.update_title(task.id, "  Launch v1 checklist  ").unwrap();
        let updated = store.tasks.iter().find(|t| t.id == task.id).unwrap();
        assert_eq!(updated.title, "Launch v1 checklist");
        store.update_title(task.id, "   ").unwrap();
        let updated = store.tasks.iter().find(|t| t.id == task.id).unwrap();
        assert_eq!(updated.title, "Launch v1 checklist");
        store.set_current(task.id).unwrap();
        assert_eq!(store.current_task().unwrap().title, "Launch v1 checklist");
        assert!(store.current_task().unwrap().is_current);
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let first = store.add_task("First", "", TaskPriority::Medium).unwrap().unwrap();
        store.add_task("Second", "", TaskPriority::Medium).unwrap();
        store.set_current(first.id).unwrap();
        store.complete(first.id, Utc::now()).unwrap();
        assert_eq!(store.incomplete_count(), 1);
        assert!(store.current_task().is_none());
        assert_eq!(store.incomplete_tasks()[0].title, "Second");
        assert_eq!(store.completed_tasks().len(), 1);
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store.add_task("Only task", "", TaskPriority::Medium).unwrap().unwrap();
        store.complete(task.id, Utc::now()).unwrap();
        assert!(store.incomplete_tasks().is_empty());
        assert_eq!(store.completed_tasks()[0].title, "Only task");
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        store.add_task("One", "", TaskPriority::Medium).unwrap();
        store.add_task("Two", "", TaskPriority::Medium).unwrap();
        store.add_task("Three", "", TaskPriority::Medium).unwrap();
        store.advance_current().unwrap();
        assert_eq!(store.current_task().unwrap().title, "One");
        store.advance_current().unwrap();
        assert_eq!(store.current_task().unwrap().title, "Two");
        store.advance_current().unwrap();
        assert_eq!(store.current_task().unwrap().title, "Three");
        store.advance_current().unwrap();
        assert_eq!(store.current_task().unwrap().title, "One");
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        assert!(store.add_task("   ", "", TaskPriority::Medium).unwrap().is_none());
        assert_eq!(store.incomplete_count(), 0);
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let first = store.add_task("First", "", TaskPriority::Medium).unwrap().unwrap();
        store.add_task("Second", "", TaskPriority::Medium).unwrap();
        store.set_current(first.id).unwrap();
        store.delete(first.id).unwrap();
        assert_eq!(store.incomplete_count(), 1);
        assert!(store.current_task().is_none());
        assert_eq!(store.incomplete_tasks()[0].title, "Second");
    }

    // priority
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let low = store.add_task("Low", "", TaskPriority::Low).unwrap().unwrap();
        let high = store.add_task("High", "", TaskPriority::High).unwrap().unwrap();
        let medium = store.add_task("Medium", "", TaskPriority::Medium).unwrap().unwrap();
        let counts = store.priority_counts();
        assert_eq!(*counts.get(&TaskPriority::High).unwrap_or(&0), 1);
        assert_eq!(*counts.get(&TaskPriority::Medium).unwrap_or(&0), 1);
        assert_eq!(*counts.get(&TaskPriority::Low).unwrap_or(&0), 1);
        assert_eq!(
            store.preview_tasks(3).iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
            vec!["High", "Medium", "Low"]
        );
        store.set_priority(low.id, TaskPriority::High).unwrap();
        let counts = store.priority_counts();
        assert_eq!(*counts.get(&TaskPriority::High).unwrap_or(&0), 2);
        assert_eq!(*counts.get(&TaskPriority::Low).unwrap_or(&0), 0);
        store.complete(high.id, Utc::now()).unwrap();
        assert_eq!(
            store.preview_tasks(2).iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
            vec!["Low", "Medium"]
        );
        let _ = medium;
    }

    // legacy priority nil => medium
    {
        let mut item = TaskItem {
            id: Uuid::new_v4(),
            title: "Legacy".into(),
            notes: String::new(),
            is_completed: false,
            is_current: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            sort_index: 0,
            priority_raw_value: None,
            due_at: None,
            reminder_at: None,
            repeat_rule_raw_value: None,
            tags_raw_value: None,
            project_name: None,
            estimated_minutes: None,
            today_sort_index: None,
            subtasks_raw_value: None,
            focus_started_at: None,
            focus_accumulated_seconds: Some(0.0),
            postponed_at: None,
            postpone_count_raw_value: Some(0),
        };
        assert_eq!(item.priority(), TaskPriority::Medium);
        let _ = item;
    }

    // parser
    {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let parsed = parse("明天 10点 发周报 #工作 !高 /30m", TaskPriority::Medium, now);
        assert_eq!(parsed.title, "发周报");
        assert_eq!(parsed.priority, TaskPriority::High);
        assert_eq!(parsed.tags, vec!["工作"]);
        assert_eq!(parsed.estimated_minutes, Some(30));
        let due = parsed.due_at.unwrap();
        assert_eq!(due.day(), 2);
        assert_eq!(due.hour(), 10);
        assert_eq!(due.minute(), 0);
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store
            .add_task("今天 15:30 写日报 #工作 !低 /45m", "", TaskPriority::Medium)
            .unwrap()
            .unwrap();
        assert_eq!(task.title, "写日报");
        assert_eq!(task.priority(), TaskPriority::Low);
        assert_eq!(task.tags(), vec!["工作"]);
        assert_eq!(task.estimated_minutes, Some(45));
        assert!(task.due_at.is_some());
        assert!(task.is_in_today_queue());
        assert_eq!(store.today_tasks().len(), 1);
    }

    {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let parsed = parse("每周五 18:00 发周报 #工作 !高", TaskPriority::Medium, now);
        assert_eq!(parsed.repeat_rule, Some(crate::models::TaskRepeatRule::Weekly));
        assert_eq!(parsed.title, "发周报");
        let due = parsed.due_at.unwrap();
        assert_eq!(due.weekday().num_days_from_monday(), 4);
        assert_eq!(due.hour(), 18);
        assert_eq!(due.minute(), 0);
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store
            .add_task("每周五 18:00 发周报 #工作 !高 /30m", "", TaskPriority::Medium)
            .unwrap()
            .unwrap();
        let old_due = task.due_at;
        store.complete(task.id, Utc::now()).unwrap();
        assert_eq!(store.completed_tasks().len(), 1);
        assert_eq!(store.incomplete_count(), 1);
        let next = store.incomplete_tasks()[0].clone();
        assert_eq!(next.title, "发周报");
        assert_eq!(next.repeat_rule(), Some(crate::models::TaskRepeatRule::Weekly));
        assert_eq!(next.tags(), vec!["工作"]);
        assert_eq!(next.estimated_minutes, Some(30));
        assert_eq!(next.priority(), TaskPriority::High);
        assert_ne!(next.due_at, old_due);
    }

    // focus
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store.add_task("写产品方案 /25m", "", TaskPriority::Medium).unwrap().unwrap();
        let start = Utc.timestamp_opt(100, 0).unwrap();
        let pause = Utc.timestamp_opt(700, 0).unwrap();
        store.start_focus(task.id, start).unwrap();
        assert_eq!(store.active_focus_task().unwrap().id, task.id);
        assert_eq!(store.focus_attention_task().unwrap().id, task.id);
        store.pause_focus(task.id, pause).unwrap();
        assert_eq!(store.focus_seconds(task.id, pause) as i64, 600);
        assert_eq!(store.focus_remaining_seconds(task.id, pause, 45) as i64, 900);
        assert!(store.active_focus_task().is_none());
        assert_eq!(store.focus_attention_task().unwrap().id, task.id);
        store.start_focus(task.id, pause).unwrap();
        assert_eq!(store.active_focus_task().unwrap().id, task.id);
        let stop = Utc.timestamp_opt(760, 0).unwrap();
        store.stop_focus(task.id, stop).unwrap();
        assert!(store.active_focus_task().is_none());
        assert!(store.focus_attention_task().is_none());

        let default_task = store.add_task("阅读资料", "", TaskPriority::Medium).unwrap().unwrap();
        assert_eq!(store.focus_target_minutes(default_task.id, 15), 15);
        assert_eq!(store.focus_remaining_seconds(default_task.id, pause, 15) as i64, 900);
    }

    // subtasks
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store.add_task("整理发布清单", "", TaskPriority::Medium).unwrap().unwrap();
        store.add_subtask(task.id, "检查安装包").unwrap();
        store.add_subtask(task.id, "更新说明").unwrap();
        let t = store.tasks.iter().find(|x| x.id == task.id).unwrap();
        assert_eq!(t.subtasks().len(), 2);
        let sub_id = t.subtasks()[0].id;
        store.toggle_subtask(task.id, sub_id).unwrap();
        let t = store.tasks.iter().find(|x| x.id == task.id).unwrap();
        assert_eq!(t.completed_subtask_count(), 1);
        let sub2 = t.subtasks()[1].id;
        store.delete_subtask(task.id, sub2).unwrap();
        let t = store.tasks.iter().find(|x| x.id == task.id).unwrap();
        assert_eq!(t.subtasks().len(), 1);
    }

    // postpone + review
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap();
        let task = store.add_task("回复邮件", "", TaskPriority::Medium).unwrap().unwrap();
        store.set_today_queue(task.id, true).unwrap();
        store.postpone(task.id, TaskPostponeOption::Tomorrow, now).unwrap();
        let t = store.tasks.iter().find(|x| x.id == task.id).unwrap();
        assert_eq!(t.postpone_count(), 1);
        assert!(t.due_at.is_some());
        assert!(!t.is_in_today_queue());
        let review = store.daily_review(now);
        assert_eq!(review.postponed_today.len(), 1);
        assert_eq!(review.tomorrow_tasks.len(), 1);
    }

    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 10, 0, 0).unwrap();
        let today_due = Utc.with_ymd_and_hms(2026, 6, 1, 16, 0, 0).unwrap();
        let future_due = Utc.with_ymd_and_hms(2026, 6, 4, 9, 0, 0).unwrap();
        store
            .add_task_from_metadata(
                "低优未来",
                "",
                false,
                false,
                now,
                now,
                None,
                TaskPriority::Low,
                Some(future_due),
                None,
                None,
                vec![],
                None,
                None,
                None,
                vec![],
            )
            .unwrap();
        store
            .add_task_from_metadata(
                "高优今天",
                "",
                false,
                false,
                now,
                now,
                None,
                TaskPriority::High,
                Some(today_due),
                None,
                None,
                vec![],
                None,
                Some(20),
                None,
                vec![],
            )
            .unwrap();
        store
            .add_task_from_metadata(
                "中优无日期",
                "",
                false,
                false,
                now,
                now,
                None,
                TaskPriority::Medium,
                None,
                None,
                None,
                vec![],
                None,
                None,
                None,
                vec![],
            )
            .unwrap();
        assert_eq!(store.suggested_today_tasks(1, now)[0].title, "高优今天");
        let task_id = store
            .incomplete_tasks()
            .into_iter()
            .find(|t| t.title == "中优无日期")
            .unwrap()
            .id;
        store.postpone(task_id, TaskPostponeOption::ThisWeek, now).unwrap();
        let t = store.tasks.iter().find(|x| x.id == task_id).unwrap();
        assert!(t.due_at.is_some());
        assert!(!t.is_in_today_queue());
    }

    // export/import json
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store.add_task("导出测试 #数据 !高 /10m", "", TaskPriority::Medium).unwrap().unwrap();
        store.add_subtask(task.id, "子任务").unwrap();
        let file = NamedTempFile::new().unwrap();
        store.export_tasks(file.path(), "json").unwrap();
        let mut imported = TaskStore::new_in_memory().unwrap();
        let count = imported.import_tasks(file.path()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(imported.incomplete_count(), 1);
        let t = imported.incomplete_tasks()[0].clone();
        assert_eq!(t.priority(), TaskPriority::High);
        assert_eq!(t.tags(), vec!["数据"]);
        assert_eq!(t.subtasks().len(), 1);
    }

    // tags/projects/search
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let due = Utc.with_ymd_and_hms(2026, 6, 3, 10, 0, 0).unwrap();
        store
            .add_task_from_metadata(
                "准备客户演示",
                "带上设计稿",
                false,
                false,
                Utc::now(),
                Utc::now(),
                None,
                TaskPriority::High,
                Some(due),
                None,
                None,
                vec!["客户".into(), "演示".into()],
                Some("增长项目".into()),
                Some(45),
                None,
                vec![],
            )
            .unwrap();
        store.add_task("无日期任务 #杂项 +个人", "", TaskPriority::Medium).unwrap();
        assert!(store.all_tags().contains(&"客户".to_string()));
        assert!(store.all_projects().contains(&"增长项目".to_string()));
        assert_eq!(store.upcoming_tasks()[0].title, "准备客户演示");
        assert_eq!(store.incomplete_tasks_tagged("客户").len(), 1);
        assert_eq!(store.incomplete_tasks_in_project("增长项目").len(), 1);
        assert_eq!(store.tasks_matching("设计稿").len(), 1);
    }

    // metadata setters
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        let task = store.add_task("编辑详情", "", TaskPriority::Medium).unwrap().unwrap();
        store.set_project_name(task.id, Some("产品".into())).unwrap();
        store.set_tags(task.id, vec!["设计".into(), "发布".into()]).unwrap();
        store.set_estimated_minutes(task.id, Some(35)).unwrap();
        store.set_repeat_rule(task.id, Some(crate::models::TaskRepeatRule::Monthly)).unwrap();
        let t = store.tasks.iter().find(|x| x.id == task.id).unwrap();
        assert_eq!(t.project_name.as_deref(), Some("产品"));
        assert_eq!(t.tags(), vec!["设计", "发布"]);
        assert_eq!(t.estimated_minutes, Some(35));
        assert_eq!(t.repeat_rule(), Some(crate::models::TaskRepeatRule::Monthly));
    }

    // csv + markdown export/import
    {
        let mut store = TaskStore::new_in_memory().unwrap();
        store.add_task("CSV 导出 #数据 !高 /10m", "", TaskPriority::Medium).unwrap();
        let csv_file = NamedTempFile::new().unwrap();
        let md_file = NamedTempFile::new().unwrap();
        store.export_tasks(csv_file.path(), "csv").unwrap();
        store.export_tasks(md_file.path(), "markdown").unwrap();
        let csv_text = std::fs::read_to_string(csv_file.path()).unwrap();
        let md_text = std::fs::read_to_string(md_file.path()).unwrap();
        assert!(csv_text.contains("CSV 导出"));
        assert!(md_text.contains("# TaskCap 导出"));
        let mut imported = TaskStore::new_in_memory().unwrap();
        let count = imported.import_csv_tasks(csv_file.path()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(imported.incomplete_tasks()[0].tags(), vec!["数据"]);
    }

    // todoist csv
    {
        let todoist = r#"TYPE,CONTENT,DESCRIPTION,PRIORITY,DATE,LABELS,PROJECT
task,发周报,整理本周数据,1,2026-06-05 18:00,工作|周报,运营
"#;
        let file = NamedTempFile::new().unwrap();
        std::fs::write(file.path(), todoist).unwrap();
        let mut store = TaskStore::new_in_memory().unwrap();
        let count = store.import_csv_tasks(file.path()).unwrap();
        assert_eq!(count, 1);
        let task = store.incomplete_tasks()[0].clone();
        assert_eq!(task.title, "发周报");
        assert_eq!(task.notes, "整理本周数据");
        assert_eq!(task.priority(), TaskPriority::High);
        assert_eq!(task.tags(), vec!["工作", "周报"]);
        assert_eq!(task.project_name.as_deref(), Some("运营"));
        assert!(task.due_at.is_some());
    }
}