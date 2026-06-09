use crate::domain::{Priority, Task};
use crate::state::BoardState;

/// Parsed board filter. Supports free text (matched against key+title,
/// case-insensitive, AND across words) plus `p:<priority>` and `l:<label>`.
#[derive(Default)]
pub struct Filter {
    pub raw: String,
    pub editing: bool,
    terms: Vec<String>,
    priority: Option<Priority>,
    label: Option<String>,
}

impl Filter {
    pub fn is_active(&self) -> bool {
        !self.raw.is_empty()
    }

    pub fn push(&mut self, c: char) {
        self.raw.push(c);
        self.reparse();
    }

    pub fn pop(&mut self) {
        self.raw.pop();
        self.reparse();
    }

    pub fn clear(&mut self) {
        self.raw.clear();
        self.terms.clear();
        self.priority = None;
        self.label = None;
        self.editing = false;
    }

    fn reparse(&mut self) {
        self.terms.clear();
        self.priority = None;
        self.label = None;
        for token in self.raw.split_whitespace() {
            let lower = token.to_lowercase();
            if let Some(p) = lower.strip_prefix("p:") {
                self.priority = Priority::ALL
                    .iter()
                    .copied()
                    .find(|pr| pr.name().to_lowercase().starts_with(p));
            } else if let Some(l) = lower.strip_prefix("l:") {
                if !l.is_empty() {
                    self.label = Some(l.to_string());
                }
            } else {
                self.terms.push(lower);
            }
        }
    }

    pub fn matches(&self, task: &Task, board: &BoardState) -> bool {
        if let Some(priority) = self.priority
            && task.priority != priority
        {
            return false;
        }
        if let Some(wanted) = &self.label {
            let has = board.task_labels.get(&task.id).is_some_and(|ids| {
                board
                    .labels
                    .iter()
                    .any(|l| ids.contains(&l.id) && l.name.to_lowercase().contains(wanted))
            });
            if !has {
                return false;
            }
        }
        if self.terms.is_empty() {
            return true;
        }
        let haystack = format!("{} {}", task.key, task.title).to_lowercase();
        self.terms.iter().all(|term| haystack.contains(term))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn task(id: i64, key: &str, title: &str, priority: Priority) -> Task {
        Task {
            id,
            board_id: 1,
            column_id: 1,
            parent_id: None,
            key: key.into(),
            title: title.into(),
            description: String::new(),
            priority,
            position: 0,
            due_date: None,
            done: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn board_with_labels() -> BoardState {
        let mut task_labels = HashMap::new();
        task_labels.insert(1, vec![10]);
        BoardState {
            board: crate::domain::Board {
                id: 1,
                name: "B".into(),
                key: "B".into(),
                next_task_num: 1,
                position: 0,
            },
            columns: Vec::new(),
            tasks: Vec::new(),
            subtasks: HashMap::new(),
            labels: vec![crate::domain::Label {
                id: 10,
                board_id: 1,
                name: "bug".into(),
                color: 0,
            }],
            task_labels,
        }
    }

    #[test]
    fn matches_text_case_insensitive() {
        let board = board_with_labels();
        let mut filter = Filter::default();
        for c in "LOGIN".chars() {
            filter.push(c);
        }
        assert!(filter.matches(&task(1, "B-1", "Fix login bug", Priority::Low), &board));
        assert!(!filter.matches(&task(2, "B-2", "Write docs", Priority::Low), &board));
    }

    #[test]
    fn matches_key() {
        let board = board_with_labels();
        let mut filter = Filter::default();
        for c in "b-2".chars() {
            filter.push(c);
        }
        assert!(filter.matches(&task(2, "B-2", "Write docs", Priority::Low), &board));
    }

    #[test]
    fn priority_prefix_filters() {
        let board = board_with_labels();
        let mut filter = Filter::default();
        for c in "p:urg".chars() {
            filter.push(c);
        }
        assert!(filter.matches(&task(1, "B-1", "x", Priority::Urgent), &board));
        assert!(!filter.matches(&task(2, "B-2", "x", Priority::Low), &board));
    }

    #[test]
    fn label_prefix_filters() {
        let board = board_with_labels();
        let mut filter = Filter::default();
        for c in "l:bug".chars() {
            filter.push(c);
        }
        assert!(filter.matches(&task(1, "B-1", "has bug label", Priority::Low), &board));
        assert!(!filter.matches(&task(2, "B-2", "no labels", Priority::Low), &board));
    }

    #[test]
    fn combined_terms_are_anded() {
        let board = board_with_labels();
        let mut filter = Filter::default();
        for c in "p:low login".chars() {
            filter.push(c);
        }
        assert!(filter.matches(&task(1, "B-1", "Fix login", Priority::Low), &board));
        assert!(!filter.matches(&task(1, "B-1", "Fix login", Priority::High), &board));
        assert!(!filter.matches(&task(1, "B-1", "Other", Priority::Low), &board));
    }
}
