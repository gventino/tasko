//! Request/response shapes for the HTTP API. Kept separate from `domain` so
//! the core types stay free of web concerns; `From` impls bridge the two.

use chrono::{DateTime, NaiveDate, Utc};
use poem_openapi::types::MaybeUndefined;
use poem_openapi::{Enum, Object};

use crate::domain::{Activity, Board, Column, Label, Priority, Task};

/// Task priority, serialized as a lowercase string in JSON.
#[derive(Debug, Clone, Copy, Enum)]
#[oai(rename_all = "lowercase")]
pub enum PriorityDto {
    Low,
    Medium,
    High,
    Urgent,
}

impl From<Priority> for PriorityDto {
    fn from(value: Priority) -> Self {
        match value {
            Priority::Low => Self::Low,
            Priority::Medium => Self::Medium,
            Priority::High => Self::High,
            Priority::Urgent => Self::Urgent,
        }
    }
}

impl From<PriorityDto> for Priority {
    fn from(value: PriorityDto) -> Self {
        match value {
            PriorityDto::Low => Self::Low,
            PriorityDto::Medium => Self::Medium,
            PriorityDto::High => Self::High,
            PriorityDto::Urgent => Self::Urgent,
        }
    }
}

#[derive(Debug, Object)]
pub struct BoardDto {
    pub id: i64,
    pub name: String,
    pub key: String,
    pub position: i64,
}

impl From<Board> for BoardDto {
    fn from(b: Board) -> Self {
        Self {
            id: b.id,
            name: b.name,
            key: b.key,
            position: b.position,
        }
    }
}

#[derive(Debug, Object)]
pub struct ColumnDto {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    pub position: i64,
    pub wip_limit: Option<i64>,
}

impl From<Column> for ColumnDto {
    fn from(c: Column) -> Self {
        Self {
            id: c.id,
            board_id: c.board_id,
            name: c.name,
            position: c.position,
            wip_limit: c.wip_limit,
        }
    }
}

#[derive(Debug, Object)]
pub struct TaskDto {
    pub id: i64,
    pub board_id: i64,
    pub column_id: i64,
    pub parent_id: Option<i64>,
    pub key: String,
    pub title: String,
    pub description: String,
    pub priority: PriorityDto,
    pub position: i64,
    pub due_date: Option<NaiveDate>,
    pub done: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Task> for TaskDto {
    fn from(t: Task) -> Self {
        Self {
            id: t.id,
            board_id: t.board_id,
            column_id: t.column_id,
            parent_id: t.parent_id,
            key: t.key,
            title: t.title,
            description: t.description,
            priority: t.priority.into(),
            position: t.position,
            due_date: t.due_date,
            done: t.done,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

#[derive(Debug, Object)]
pub struct LabelDto {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    pub color: i64,
}

impl From<Label> for LabelDto {
    fn from(l: Label) -> Self {
        Self {
            id: l.id,
            board_id: l.board_id,
            name: l.name,
            color: l.color,
        }
    }
}

#[derive(Debug, Object)]
pub struct ActivityDto {
    pub id: i64,
    pub task_id: i64,
    pub kind: String,
    pub detail: String,
    pub created_at: DateTime<Utc>,
}

impl From<Activity> for ActivityDto {
    fn from(a: Activity) -> Self {
        Self {
            id: a.id,
            task_id: a.task_id,
            kind: a.kind.as_str().to_string(),
            detail: a.detail,
            created_at: a.created_at,
        }
    }
}

#[derive(Debug, Object)]
pub struct CreateBoard {
    pub name: String,
    /// Optional Jira-style key; derived from the name when omitted.
    pub key: Option<String>,
}

#[derive(Debug, Object)]
pub struct UpdateBoard {
    pub name: String,
}

#[derive(Debug, Object)]
pub struct CreateColumn {
    pub name: String,
}

#[derive(Debug, Object)]
pub struct UpdateColumn {
    pub name: Option<String>,
    /// Present sets the WIP limit; explicit `null` clears it; absent leaves it.
    pub wip_limit: MaybeUndefined<i64>,
}

#[derive(Debug, Object)]
pub struct CreateTask {
    pub column_id: i64,
    /// Set to create a subtask under an existing task.
    pub parent_id: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<PriorityDto>,
    pub due_date: Option<NaiveDate>,
}

#[derive(Debug, Object)]
pub struct PatchTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<PriorityDto>,
    /// Present sets the due date; explicit `null` clears it; absent leaves it.
    pub due_date: MaybeUndefined<NaiveDate>,
    pub done: Option<bool>,
    /// Move target column (paired with `position`).
    pub column_id: Option<i64>,
    /// Move target position (paired with `column_id`).
    pub position: Option<i64>,
}

#[derive(Debug, Object)]
pub struct CreateLabel {
    pub name: String,
    pub color: Option<i64>,
}

#[derive(Debug, Object)]
pub struct UpdateLabel {
    pub name: Option<String>,
    pub color: Option<i64>,
}

#[derive(Debug, Object)]
pub struct SetTaskLabels {
    pub label_ids: Vec<i64>,
}
