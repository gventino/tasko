use chrono::{DateTime, NaiveDate, Utc};

pub type Id = i64;

/// Spacing between consecutive positions; leaves room for cheap mid-insertions.
pub const POSITION_GAP: i64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum Priority {
    Low = 0,
    #[default]
    Medium = 1,
    High = 2,
    Urgent = 3,
}

impl Priority {
    pub const ALL: [Priority; 4] = [
        Priority::Low,
        Priority::Medium,
        Priority::High,
        Priority::Urgent,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
            Priority::Urgent => "Urgent",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Priority::Low => "▽",
            Priority::Medium => "◆",
            Priority::High => "▲",
            Priority::Urgent => "⚑",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            Priority::Low => Priority::Medium,
            Priority::Medium => Priority::High,
            Priority::High => Priority::Urgent,
            Priority::Urgent => Priority::Low,
        }
    }
}

impl TryFrom<i64> for Priority {
    type Error = String;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Priority::Low),
            1 => Ok(Priority::Medium),
            2 => Ok(Priority::High),
            3 => Ok(Priority::Urgent),
            other => Err(format!("invalid priority: {other}")),
        }
    }
}

impl From<Priority> for i64 {
    fn from(value: Priority) -> Self {
        value as i64
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Board {
    pub id: Id,
    pub name: String,
    pub key: String,
    /// Backing for key allocation; consumed in SQL, mirrored here for completeness.
    #[allow(dead_code)]
    pub next_task_num: i64,
    /// Ordering handled by SQL `ORDER BY`; mirrored for schema completeness.
    #[allow(dead_code)]
    pub position: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Column {
    pub id: Id,
    pub board_id: Id,
    pub name: String,
    pub position: i64,
    pub wip_limit: Option<i64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Task {
    pub id: Id,
    pub board_id: Id,
    pub column_id: Id,
    pub parent_id: Option<Id>,
    pub key: String,
    pub title: String,
    pub description: String,
    #[sqlx(try_from = "i64")]
    pub priority: Priority,
    pub position: i64,
    pub due_date: Option<NaiveDate>,
    pub done: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        self.due_date.is_some_and(|due| due < today)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Label {
    pub id: Id,
    /// Scoping handled in SQL queries; mirrored for schema completeness.
    #[allow(dead_code)]
    pub board_id: Id,
    pub name: String,
    pub color: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Activity {
    #[allow(dead_code)]
    pub id: Id,
    #[allow(dead_code)]
    pub task_id: Id,
    /// Reserved for per-kind icons/filtering in the activity feed.
    #[allow(dead_code)]
    pub kind: String,
    pub detail: String,
    pub created_at: DateTime<Utc>,
}

pub mod activity_kind {
    pub const CREATED: &str = "created";
    pub const EDITED: &str = "edited";
    pub const MOVED: &str = "moved";
    pub const PRIORITY: &str = "priority";
    pub const LABELS: &str = "labels";
    pub const SUBTASK: &str = "subtask";
}

/// Midpoint position between two neighbors. `None` on either side means
/// start/end of the list. Returns `None` when no integer gap remains and the
/// column must be renumbered.
pub fn position_between(before: Option<i64>, after: Option<i64>) -> Option<i64> {
    match (before, after) {
        (None, None) => Some(POSITION_GAP),
        (Some(b), None) => b.checked_add(POSITION_GAP),
        (None, Some(a)) => {
            let mid = a / 2;
            (0 < mid && mid < a).then_some(mid)
        }
        (Some(b), Some(a)) => {
            let mid = b + (a - b) / 2;
            (b < mid && mid < a).then_some(mid)
        }
    }
}

/// Derive a Jira-style board key (e.g. "Task Organizer" -> "TO") that does not
/// collide with `existing`.
pub fn derive_board_key(name: &str, existing: &[String]) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let mut base: String = if words.len() >= 2 {
        words
            .iter()
            .filter_map(|w| w.chars().find(|c| c.is_alphanumeric()))
            .take(4)
            .collect()
    } else {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .take(3)
            .collect()
    };
    base = base.to_uppercase();
    if base.is_empty() {
        base = "BRD".to_string();
    }
    if !existing.iter().any(|k| k == &base) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}{n}");
        if !existing.iter().any(|k| k == &candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_between_empty_list() {
        assert_eq!(position_between(None, None), Some(POSITION_GAP));
    }

    #[test]
    fn position_between_appends_with_gap() {
        assert_eq!(
            position_between(Some(2048), None),
            Some(2048 + POSITION_GAP)
        );
    }

    #[test]
    fn position_between_prepends_by_halving() {
        assert_eq!(position_between(None, Some(1024)), Some(512));
        assert_eq!(position_between(None, Some(1)), None);
    }

    #[test]
    fn position_between_midpoint() {
        assert_eq!(position_between(Some(1024), Some(2048)), Some(1536));
        assert_eq!(position_between(Some(5), Some(6)), None);
    }

    #[test]
    fn derive_key_multi_word() {
        assert_eq!(derive_board_key("Task Organizer", &[]), "TO");
    }

    #[test]
    fn derive_key_single_word() {
        assert_eq!(derive_board_key("tasko", &[]), "TAS");
    }

    #[test]
    fn derive_key_collision_appends_number() {
        let existing = vec!["TAS".to_string(), "TAS2".to_string()];
        assert_eq!(derive_board_key("tasko", &existing), "TAS3");
    }

    #[test]
    fn derive_key_empty_name_falls_back() {
        assert_eq!(derive_board_key("!!!", &[]), "BRD");
    }

    #[test]
    fn priority_cycles_through_all() {
        let mut p = Priority::Low;
        for _ in 0..4 {
            p = p.cycle();
        }
        assert_eq!(p, Priority::Low);
    }
}
