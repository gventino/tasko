use anyhow::Result;
use chrono::{Duration, Local};

use crate::db::Db;
use crate::db::tasks::NewTask;
use crate::domain::{Priority, derive_board_key};

const TITLES: [&str; 12] = [
    "Fix login redirect loop",
    "Write onboarding docs",
    "Refactor billing module",
    "Add dark mode toggle",
    "Investigate flaky CI job",
    "Upgrade database driver",
    "Design empty states",
    "Cache board queries",
    "Improve error toasts",
    "Add CSV export",
    "Polish keyboard shortcuts",
    "Profile render hot path",
];

const LABELS: [(&str, i64); 4] = [("bug", 0), ("feature", 3), ("docs", 2), ("infra", 5)];

/// Populate the database with demo/load-test data (used by `--seed N`).
pub async fn seed(db: &Db, count: usize) -> Result<()> {
    let boards = db.list_boards().await?;
    let board = match boards.first() {
        Some(b) => b.clone(),
        None => {
            let keys: Vec<String> = Vec::new();
            db.create_board("Main Board", &derive_board_key("Main Board", &keys))
                .await?
        }
    };
    let columns = db.columns_for_board(board.id).await?;
    anyhow::ensure!(!columns.is_empty(), "board has no columns");

    let mut label_ids = Vec::new();
    let existing = db.labels_for_board(board.id).await?;
    for (name, color) in LABELS {
        match existing.iter().find(|l| l.name == name) {
            Some(l) => label_ids.push(l.id),
            None => label_ids.push(db.create_label(board.id, name, color).await?.id),
        }
    }

    let today = Local::now().date_naive();
    for i in 0..count {
        let column = &columns[i % columns.len()];
        let title = format!("{} #{}", TITLES[i % TITLES.len()], i + 1);
        let due_date = (i % 5 == 0).then(|| today + Duration::days(i as i64 % 10 - 2));
        let task = db
            .create_task(NewTask {
                board_id: board.id,
                column_id: column.id,
                parent_id: None,
                title,
                description: if i % 3 == 0 {
                    "Steps:\n1. Reproduce\n2. Fix\n3. Add regression test".to_string()
                } else {
                    String::new()
                },
                priority: Priority::ALL[i % 4],
                due_date,
            })
            .await?;
        if i % 4 == 0 {
            db.set_task_labels(task.id, &[label_ids[i % label_ids.len()]])
                .await?;
        }
        if i % 6 == 0 {
            for s in 0..(i % 3 + 1) {
                let sub = db
                    .create_task(NewTask {
                        board_id: board.id,
                        column_id: column.id,
                        parent_id: Some(task.id),
                        title: format!("Subtask {}", s + 1),
                        description: String::new(),
                        priority: Priority::Medium,
                        due_date: None,
                    })
                    .await?;
                if s == 0 {
                    db.set_task_done(sub.id, true).await?;
                }
            }
        }
    }
    Ok(())
}
