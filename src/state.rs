use std::collections::HashMap;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::db::Db;
use crate::domain::{Board, Column, Id, Label, Task, derive_board_key};

/// Everything the UI needs about the active board, kept in memory and updated
/// optimistically; the database is the durable copy.
pub struct BoardState {
    pub board: Board,
    pub columns: Vec<Column>,
    /// Top-level tasks per column (parallel to `columns`), sorted by position.
    pub tasks: Vec<Vec<Task>>,
    /// Subtasks by parent id, sorted by position.
    pub subtasks: HashMap<Id, Vec<Task>>,
    pub labels: Vec<Label>,
    pub task_labels: HashMap<Id, Vec<Id>>,
}

impl BoardState {
    pub fn column_index(&self, column_id: Id) -> Option<usize> {
        self.columns.iter().position(|c| c.id == column_id)
    }

    pub fn subtask_progress(&self, task_id: Id) -> Option<(usize, usize)> {
        let subs = self.subtasks.get(&task_id)?;
        if subs.is_empty() {
            return None;
        }
        Some((subs.iter().filter(|s| s.done).count(), subs.len()))
    }

    /// Find a task (top-level or subtask) by id.
    pub fn find_task(&self, task_id: Id) -> Option<&Task> {
        self.tasks
            .iter()
            .flatten()
            .chain(self.subtasks.values().flatten())
            .find(|t| t.id == task_id)
    }

    /// Mutable lookup across top-level tasks and subtasks.
    pub fn find_task_mut(&mut self, task_id: Id) -> Option<&mut Task> {
        self.tasks
            .iter_mut()
            .flatten()
            .chain(self.subtasks.values_mut().flatten())
            .find(|t| t.id == task_id)
    }
}

impl std::fmt::Debug for BoardState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BoardState({})", self.board.name)
    }
}

/// Load the full state of one board (4 queries, runs off the UI loop).
pub async fn load_board_state(db: &Db, board: Board) -> Result<BoardState> {
    let columns = db.columns_for_board(board.id).await?;
    let all_tasks = db.tasks_for_board(board.id).await?;
    let labels = db.labels_for_board(board.id).await?;
    let pairs = db.task_label_pairs(board.id).await?;

    let mut tasks: Vec<Vec<Task>> = columns.iter().map(|_| Vec::new()).collect();
    let mut subtasks: HashMap<Id, Vec<Task>> = HashMap::new();
    for task in all_tasks {
        match task.parent_id {
            Some(parent) => subtasks.entry(parent).or_default().push(task),
            None => {
                if let Some(idx) = columns.iter().position(|c| c.id == task.column_id) {
                    tasks[idx].push(task);
                }
            }
        }
    }
    let mut task_labels: HashMap<Id, Vec<Id>> = HashMap::new();
    for (task_id, label_id) in pairs {
        task_labels.entry(task_id).or_default().push(label_id);
    }

    Ok(BoardState {
        board,
        columns,
        tasks,
        subtasks,
        labels,
        task_labels,
    })
}

/// First load: guarantees at least one board exists, then loads the first one.
pub async fn bootstrap(db: &Db) -> Result<(Vec<Board>, BoardState)> {
    let mut boards = db.list_boards().await?;
    if boards.is_empty() {
        db.create_board("Main Board", &derive_board_key("Main Board", &[]))
            .await?;
        boards = db.list_boards().await?;
    }
    let state = load_board_state(db, boards[0].clone()).await?;
    Ok((boards, state))
}

pub type Tx = mpsc::UnboundedSender<crate::app::Message>;
