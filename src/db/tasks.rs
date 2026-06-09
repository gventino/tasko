use anyhow::Result;
use chrono::{NaiveDate, Utc};

use super::Db;
use crate::domain::{Id, POSITION_GAP, Priority, Task, activity_kind};

pub struct NewTask {
    pub board_id: Id,
    pub column_id: Id,
    pub parent_id: Option<Id>,
    pub title: String,
    pub description: String,
    pub priority: Priority,
    pub due_date: Option<NaiveDate>,
}

impl Db {
    /// All tasks of a board, subtasks included; ordering is resolved in memory.
    pub async fn tasks_for_board(&self, board_id: Id) -> Result<Vec<Task>> {
        Ok(sqlx::query_as(
            "SELECT id, board_id, column_id, parent_id, key, title, description, priority, \
             position, due_date, done, created_at, updated_at \
             FROM tasks WHERE board_id = ? ORDER BY position, id",
        )
        .bind(board_id)
        .fetch_all(self.pool())
        .await?)
    }

    /// Insert a task, atomically allocating its board-scoped key (e.g. "TSK-7")
    /// and a position at the end of its column (or sibling list, for subtasks).
    pub async fn create_task(&self, new: NewTask) -> Result<Task> {
        let mut tx = self.pool().begin().await?;
        let (board_key, num): (String, i64) = sqlx::query_as(
            "UPDATE boards SET next_task_num = next_task_num + 1 WHERE id = ? \
             RETURNING key, next_task_num - 1",
        )
        .bind(new.board_id)
        .fetch_one(&mut *tx)
        .await?;
        let key = format!("{board_key}-{num}");

        let position: i64 = match new.parent_id {
            Some(parent_id) => {
                sqlx::query_scalar(
                    "SELECT COALESCE(MAX(position), 0) + ? FROM tasks WHERE parent_id = ?",
                )
                .bind(POSITION_GAP)
                .bind(parent_id)
                .fetch_one(&mut *tx)
                .await?
            }
            None => {
                sqlx::query_scalar(
                    "SELECT COALESCE(MAX(position), 0) + ? FROM tasks \
                     WHERE column_id = ? AND parent_id IS NULL",
                )
                .bind(POSITION_GAP)
                .bind(new.column_id)
                .fetch_one(&mut *tx)
                .await?
            }
        };

        let now = Utc::now();
        let task: Task = sqlx::query_as(
            "INSERT INTO tasks (board_id, column_id, parent_id, key, title, description, \
             priority, position, due_date, done, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?) \
             RETURNING id, board_id, column_id, parent_id, key, title, description, priority, \
             position, due_date, done, created_at, updated_at",
        )
        .bind(new.board_id)
        .bind(new.column_id)
        .bind(new.parent_id)
        .bind(&key)
        .bind(&new.title)
        .bind(&new.description)
        .bind(i64::from(new.priority))
        .bind(position)
        .bind(new.due_date)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO activities (task_id, kind, detail, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(task.id)
        .bind(activity_kind::CREATED)
        .bind("Task created")
        .bind(now)
        .execute(&mut *tx)
        .await?;
        if let Some(parent_id) = new.parent_id {
            sqlx::query(
                "INSERT INTO activities (task_id, kind, detail, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(parent_id)
            .bind(activity_kind::SUBTASK)
            .bind(format!("Added subtask \"{}\"", new.title))
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(task)
    }

    pub async fn update_task_content(
        &self,
        id: Id,
        title: &str,
        description: &str,
        priority: Priority,
        due_date: Option<NaiveDate>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET title = ?, description = ?, priority = ?, due_date = ?, \
             updated_at = ? WHERE id = ?",
        )
        .bind(title)
        .bind(description)
        .bind(i64::from(priority))
        .bind(due_date)
        .bind(Utc::now())
        .bind(id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn move_task(&self, id: Id, column_id: Id, position: i64) -> Result<()> {
        sqlx::query("UPDATE tasks SET column_id = ?, position = ?, updated_at = ? WHERE id = ?")
            .bind(column_id)
            .bind(position)
            .bind(Utc::now())
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Swap the positions of two tasks atomically (used for in-column reorder —
    /// never needs renumbering).
    pub async fn swap_task_positions(&self, a: Id, a_pos: i64, b: Id, b_pos: i64) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("UPDATE tasks SET position = ? WHERE id = ?")
            .bind(b_pos)
            .bind(a)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE tasks SET position = ? WHERE id = ?")
            .bind(a_pos)
            .bind(b)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_task_priority(&self, id: Id, priority: Priority) -> Result<()> {
        sqlx::query("UPDATE tasks SET priority = ?, updated_at = ? WHERE id = ?")
            .bind(i64::from(priority))
            .bind(Utc::now())
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn set_task_done(&self, id: Id, done: bool) -> Result<()> {
        sqlx::query("UPDATE tasks SET done = ?, updated_at = ? WHERE id = ?")
            .bind(done)
            .bind(Utc::now())
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Rewrite positions of a column's top-level tasks with fresh gaps and
    /// return them in their new order. Defensive maintenance op: reorders use
    /// swaps and appends use max+gap, so gaps cannot run out in normal flow.
    #[allow(dead_code)]
    pub async fn renumber_tasks(&self, column_id: Id) -> Result<Vec<Task>> {
        let tasks = self.top_level_tasks_in_column(column_id).await?;
        let mut tx = self.pool().begin().await?;
        for (i, task) in tasks.iter().enumerate() {
            sqlx::query("UPDATE tasks SET position = ? WHERE id = ?")
                .bind((i as i64 + 1) * POSITION_GAP)
                .bind(task.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        self.top_level_tasks_in_column(column_id).await
    }

    async fn top_level_tasks_in_column(&self, column_id: Id) -> Result<Vec<Task>> {
        Ok(sqlx::query_as(
            "SELECT id, board_id, column_id, parent_id, key, title, description, priority, \
             position, due_date, done, created_at, updated_at \
             FROM tasks WHERE column_id = ? AND parent_id IS NULL ORDER BY position, id",
        )
        .bind(column_id)
        .fetch_all(self.pool())
        .await?)
    }

    pub async fn delete_task(&self, id: Id) -> Result<()> {
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Case-insensitive global search across boards by key or title.
    pub async fn search_tasks_global(&self, query: &str, limit: i64) -> Result<Vec<Task>> {
        let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        Ok(sqlx::query_as(
            "SELECT id, board_id, column_id, parent_id, key, title, description, priority, \
             position, due_date, done, created_at, updated_at FROM tasks \
             WHERE parent_id IS NULL AND (title LIKE ? ESCAPE '\\' OR key LIKE ? ESCAPE '\\') \
             ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(&pattern)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(self.pool())
        .await?)
    }
}
