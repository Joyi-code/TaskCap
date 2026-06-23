use crate::models::{TaskPriority, TaskRepeatRule};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc, Weekday};
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTaskInput {
    pub title: String,
    pub priority: TaskPriority,
    pub due_at: Option<DateTime<Utc>>,
    pub reminder_at: Option<DateTime<Utc>>,
    pub repeat_rule: Option<TaskRepeatRule>,
    pub tags: Vec<String>,
    pub project_name: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub is_today: bool,
}

pub fn parse(
    raw_input: &str,
    fallback_priority: TaskPriority,
    now: DateTime<Utc>,
) -> ParsedTaskInput {
    let mut working = raw_input.to_string();
    let mut priority = fallback_priority;
    let mut tags: Vec<String> = Vec::new();
    let mut project_name: Option<String> = None;
    let mut estimated_minutes: Option<i32> = None;
    let mut due_at: Option<DateTime<Utc>> = None;
    let mut reminder_at: Option<DateTime<Utc>> = None;
    let mut repeat_rule: Option<TaskRepeatRule> = None;
    let mut is_today = false;

    // Rust `regex` crate 不支持 look-around，改用手写边界扫描（对齐 Swift NSRegularExpression）
    const PRIORITY_TOKENS: &[&str] = &[
        "!高", "!high", "!p1", "p1", "优先级高", "高优", "!中", "!medium", "!p2", "p2", "中优", "!低",
        "!low", "!p3", "p3", "低优",
    ];
    let mut priority_ranges = Vec::new();
    for token in PRIORITY_TOKENS {
        for (start, end) in find_bounded_tokens_ci(&working, token) {
            let matched = &working[start..end];
            let token_lc = matched.to_lowercase();
            priority = if token_lc.contains('高')
                || token_lc.contains("high")
                || token_lc.contains("p1")
            {
                TaskPriority::High
            } else if token_lc.contains('低')
                || token_lc.contains("low")
                || token_lc.contains("p3")
            {
                TaskPriority::Low
            } else {
                TaskPriority::Medium
            };
            priority_ranges.push((start, end));
        }
    }
    for (start, end) in priority_ranges.into_iter().rev() {
        working = remove_range(start, end, &working);
    }

    let mut tag_ranges = Vec::new();
    for (start, end, tag) in find_hash_tags(&working) {
        if !tags.contains(&tag) {
            tags.insert(0, tag);
        }
        tag_ranges.push((start, end));
    }
    for (start, end) in tag_ranges.into_iter().rev() {
        working = remove_range(start, end, &working);
    }

    if let Some((start, end, project)) = find_plus_project(&working) {
        project_name = Some(project);
        working = remove_range(start, end, &working);
    }

    if let Some((start, end, minutes)) = find_estimated_minutes(&working) {
        estimated_minutes = Some(minutes);
        working = remove_range(start, end, &working);
    }

    let date_result = parsed_date(&working, now);
    let time_result = parsed_time(&working);
    let repeat_result = parsed_repeat_rule(&working);

    if let Some(date) = date_result.date {
        due_at = Some(apply_time(time_result.components, date));
        reminder_at = due_at;
        is_today = due_at.map(|d| same_day(d, now)).unwrap_or(false);
    } else if let Some((hour, minute)) = time_result.components {
        let today = start_of_day(now);
        due_at = Some(apply_hm(hour, minute, today));
        reminder_at = due_at;
        is_today = true;
    }

    if working.contains("今天") {
        is_today = true;
    }

    if let Some(rule) = repeat_result.rule {
        repeat_rule = Some(rule);
    }

    let mut metadata_ranges = Vec::new();
    if let Some(r) = date_result.range {
        metadata_ranges.push(r);
    }
    if let Some(r) = time_result.range {
        metadata_ranges.push(r);
    }
    if let Some(r) = repeat_result.range {
        metadata_ranges.push(r);
    }
    let mut metadata_ranges = non_overlapping_ranges(metadata_ranges);
    metadata_ranges.sort_unstable_by_key(|(start, _)| *start);
    for (start, end) in metadata_ranges.into_iter().rev() {
        working = remove_range(start, end, &working);
    }

    let title = normalized_title(&working);
    let fallback_title = normalized_title(raw_input);
    ParsedTaskInput {
        title: if title.is_empty() { fallback_title } else { title },
        priority,
        due_at,
        reminder_at,
        repeat_rule,
        tags,
        project_name,
        estimated_minutes,
        is_today,
    }
}

struct DateParseResult {
    date: Option<DateTime<Utc>>,
    range: Option<(usize, usize)>,
}

struct TimeParseResult {
    components: Option<(u32, u32)>,
    range: Option<(usize, usize)>,
}

struct RepeatParseResult {
    rule: Option<TaskRepeatRule>,
    range: Option<(usize, usize)>,
}

fn parsed_date(text: &str, now: DateTime<Utc>) -> DateParseResult {
    let start_of_today = start_of_day(now);
    let simple_dates = [("今天", 0), ("今晚", 0), ("明天", 1), ("明晚", 1), ("后天", 2)];

    for (token, offset) in simple_dates {
        if let Some((start, end)) = find_token(text, token) {
            let date = start_of_today + Duration::days(offset);
            return DateParseResult {
                date: Some(date),
                range: Some((start, end)),
            };
        }
    }

    let weekdays = [
        ("周日", Weekday::Sun, 1),
        ("周一", Weekday::Mon, 2),
        ("周二", Weekday::Tue, 3),
        ("周三", Weekday::Wed, 4),
        ("周四", Weekday::Thu, 5),
        ("周五", Weekday::Fri, 6),
        ("周六", Weekday::Sat, 7),
    ];

    for (title, weekday, apple_weekday) in weekdays {
        let every_token = format!("每{title}");
        if let Some((start, end)) = find_token(text, &every_token) {
            let target = next_apple_weekday(start_of_today, apple_weekday);
            return DateParseResult {
                date: Some(target),
                range: Some((start, end)),
            };
        }
        if let Some((start, end)) = find_token(text, title) {
            let target = next_weekday(start_of_today, weekday);
            return DateParseResult {
                date: Some(target),
                range: Some((start, end)),
            };
        }
    }

    DateParseResult {
        date: None,
        range: None,
    }
}

fn parsed_repeat_rule(text: &str) -> RepeatParseResult {
    let tokens = [
        ("每天", TaskRepeatRule::Daily),
        ("每日", TaskRepeatRule::Daily),
        ("每周", TaskRepeatRule::Weekly),
        ("每星期", TaskRepeatRule::Weekly),
        ("每月", TaskRepeatRule::Monthly),
        ("每年", TaskRepeatRule::Yearly),
    ];
    for (token, rule) in tokens {
        if let Some((start, end)) = find_token(text, token) {
            return RepeatParseResult {
                rule: Some(rule),
                range: Some((start, end)),
            };
        }
    }
    RepeatParseResult {
        rule: None,
        range: None,
    }
}

fn parsed_time(text: &str) -> TimeParseResult {
    let patterns: [(&str, bool); 4] = [
        (r"(\d{1,2})[:：](\d{2})", false),
        (r"(\d{1,2})\s*点半", true),
        (r"(\d{1,2})\s*点(\d{1,2})\s*分?", false),
        (r"(\d{1,2})\s*点", false),
    ];

    for (pattern, half) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(cap) = re.captures(text) {
                let mut hour: u32 = cap.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
                let minute = if half {
                    30
                } else if let Some(m) = cap.get(2) {
                    m.as_str().parse().unwrap_or(0)
                } else {
                    0
                };
                if text.contains("下午") || text.contains("晚上") || text.contains("今晚") || text.contains("明晚") {
                    if hour < 12 {
                        hour += 12;
                    }
                }
                return TimeParseResult {
                    components: Some((hour.min(23), minute.min(59))),
                    range: cap.get(0).map(|m| (m.start(), m.end())),
                };
            }
        }
    }

    if let Some(r) = find_token(text, "今晚").or_else(|| find_token(text, "晚上")) {
        return TimeParseResult {
            components: Some((20, 0)),
            range: Some(r),
        };
    }
    if let Some(r) = find_token(text, "明早").or_else(|| find_token(text, "早上")) {
        return TimeParseResult {
            components: Some((9, 0)),
            range: Some(r),
        };
    }

    TimeParseResult {
        components: None,
        range: None,
    }
}

fn apply_time(components: Option<(u32, u32)>, date: DateTime<Utc>) -> DateTime<Utc> {
    match components {
        Some((h, m)) => apply_hm(h, m, date),
        None => date,
    }
}

fn apply_hm(hour: u32, minute: u32, date: DateTime<Utc>) -> DateTime<Utc> {
    let local = date.with_timezone(&Local);
    Local
        .with_ymd_and_hms(local.year(), local.month(), local.day(), hour, minute, 0)
        .single()
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or(date)
}

fn next_weekday(from: DateTime<Utc>, target: Weekday) -> DateTime<Utc> {
    let start = start_of_day(from);
    let current = start.with_timezone(&Local).weekday();
    let mut offset = target.num_days_from_monday() as i32 - current.num_days_from_monday() as i32;
    if offset <= 0 {
        offset += 7;
    }
    start + Duration::days(offset as i64)
}

fn next_apple_weekday(from: DateTime<Utc>, apple_weekday: i32) -> DateTime<Utc> {
    let start = start_of_day(from);
    let current_apple = apple_weekday_number(start.with_timezone(&Local).weekday());
    let mut offset = apple_weekday - current_apple;
    if offset <= 0 {
        offset += 7;
    }
    start + Duration::days(offset as i64)
}

fn apple_weekday_number(weekday: Weekday) -> i32 {
    match weekday {
        Weekday::Sun => 1,
        Weekday::Mon => 2,
        Weekday::Tue => 3,
        Weekday::Wed => 4,
        Weekday::Thu => 5,
        Weekday::Fri => 6,
        Weekday::Sat => 7,
    }
}

fn start_of_day(dt: DateTime<Utc>) -> DateTime<Utc> {
    let local = dt.with_timezone(&Local);
    Local
        .with_ymd_and_hms(local.year(), local.month(), local.day(), 0, 0, 0)
        .single()
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or(dt)
}

fn same_day(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    a.with_timezone(&Local).date_naive() == b.with_timezone(&Local).date_naive()
}

fn normalized_title(text: &str) -> String {
    let re = Regex::new(r"\s+").unwrap();
    re.replace_all(text.trim(), " ").trim().to_string()
}

fn find_token(text: &str, token: &str) -> Option<(usize, usize)> {
    text.find(token).map(|start| (start, start + token.len()))
}

fn is_token_start(text: &str, start: usize) -> bool {
    if start == 0 {
        return true;
    }
    text[..start]
        .chars()
        .last()
        .map(|c| c.is_whitespace())
        .unwrap_or(true)
}

fn is_token_end(text: &str, end: usize) -> bool {
    end >= text.len()
        || text[end..]
            .chars()
            .next()
            .map(|c| c.is_whitespace())
            .unwrap_or(true)
}

fn find_bounded_tokens_ci(text: &str, token: &str) -> Vec<(usize, usize)> {
    let lower = text.to_lowercase();
    let token_lc = token.to_lowercase();
    let mut results = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = lower[search_from..].find(&token_lc) {
        let start = search_from + rel;
        let end = start + token.len();
        if is_token_start(text, start) && is_token_end(text, end) {
            results.push((start, end));
        }
        search_from = start + token.len().max(1);
    }
    results
}

fn is_tag_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_' || ch == '-'
}

fn find_hash_tags(text: &str) -> Vec<(usize, usize, String)> {
    let mut results = Vec::new();
    for (idx, ch) in text.char_indices() {
        if ch != '#' || !is_token_start(text, idx) {
            continue;
        }
        let tag_start = idx + ch.len_utf8();
        let mut tag_end = tag_start;
        for (offset, c) in text[tag_start..].char_indices() {
            if !is_tag_char(c) {
                break;
            }
            tag_end = tag_start + offset + c.len_utf8();
        }
        if tag_end > tag_start {
            results.push((idx, tag_end, text[tag_start..tag_end].to_string()));
        }
    }
    results
}

fn find_plus_project(text: &str) -> Option<(usize, usize, String)> {
    for (idx, ch) in text.char_indices() {
        if ch != '+' || !is_token_start(text, idx) {
            continue;
        }
        let name_start = idx + ch.len_utf8();
        let mut name_end = name_start;
        for (offset, c) in text[name_start..].char_indices() {
            if !is_tag_char(c) {
                break;
            }
            name_end = name_start + offset + c.len_utf8();
        }
        if name_end > name_start {
            return Some((idx, name_end, text[name_start..name_end].to_string()));
        }
    }
    None
}

fn find_estimated_minutes(text: &str) -> Option<(usize, usize, i32)> {
    let duration_patterns: [(&str, bool); 4] = [
        (r"/(\d{1,3})m", false),
        (r"/(\d{1,2})h", true),
        (r"(\d{1,3})\s*分钟", false),
        (r"(\d{1,2})\s*小时", true),
    ];
    for (pattern, is_hours) in duration_patterns {
        if let Ok(re) = Regex::new(pattern) {
            for cap in re.captures_iter(text) {
                if let Some(full) = cap.get(0) {
                    let start = full.start();
                    let end = full.end();
                    if is_token_start(text, start) && is_token_end(text, end) {
                        let number: i32 = cap
                            .get(1)
                            .and_then(|m| m.as_str().parse().ok())
                            .unwrap_or(0);
                        return Some((start, end, if is_hours { number * 60 } else { number }));
                    }
                }
            }
        }
    }
    None
}

fn remove_range(start: usize, end: usize, text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..start.min(text.len())]);
    result.push_str(&text[end.min(text.len())..]);
    result
}

fn ranges_overlap(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

fn non_overlapping_ranges(ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut accepted = Vec::new();
    for range in ranges {
        if !accepted.iter().any(|existing| ranges_overlap(*existing, range)) {
            accepted.push(range);
        }
    }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    #[test]
    fn parses_chinese_time_with_minutes() {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        for input in &["明天9点45开会", "明天9点45分开会"] {
            let parsed = parse(input, TaskPriority::Medium, now);
            assert_eq!(parsed.title, "开会", "title mismatch for: {input}");
            let due_local = parsed.due_at.unwrap().with_timezone(&Local);
            assert_eq!(due_local.hour(), 9, "hour mismatch for: {input}");
            assert_eq!(due_local.minute(), 45, "minute mismatch for: {input}");
        }
    }

    #[test]
    fn parses_tomorrow_time_priority_tag_duration() {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        let parsed = parse("明天 10点 发周报 #工作 !高 /30m", TaskPriority::Medium, now);
        assert_eq!(parsed.title, "发周报");
        assert_eq!(parsed.priority, TaskPriority::High);
        assert_eq!(parsed.tags, vec!["工作"]);
        assert_eq!(parsed.estimated_minutes, Some(30));
        let due_local = parsed.due_at.unwrap().with_timezone(&Local);
        assert_eq!(due_local.day(), 2);
        assert_eq!(due_local.hour(), 10);
        assert_eq!(due_local.minute(), 0);
    }

    #[test]
    fn parses_daily_time_before_chinese_title_without_panicking() {
        let now = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let parsed = parse("每天10点写日报", TaskPriority::Medium, now);

        assert_eq!(parsed.title, "写日报");
        assert_eq!(parsed.repeat_rule, Some(TaskRepeatRule::Daily));
        let due_local = parsed.due_at.unwrap().with_timezone(&Local);
        assert_eq!(due_local.hour(), 10);
        assert_eq!(due_local.minute(), 0);
    }
}
