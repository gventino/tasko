use anyhow::Result;

use super::Db;
use crate::domain::{Board, Id, POSITION_GAP};

const DEFAULT_COLUMNS: [&str; 3] = ["To Do", "In Progress", "Done"];

impl Db {
    pub async fn list_boards(&self) -> Result<Vec<Board>> {
        Ok(sqlx::query_as(
            "SELECT id, name, key, next_task_num, position FROM boards ORDER BY position, id",
        )
        .fetch_all(self.pool())
        .await?)
    }

    /// Create a board plus its default columns in one transaction.
    pub async fn create_board(&self, name: &str, key: &str) -> Result<Board> {
        let mut tx = self.pool().begin().await?;
        let position: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + ? FROM boards")
            .bind(POSITION_GAP)
            .fetch_one(&mut *tx)
            .await?;
        let board: Board = sqlx::query_as(
            "INSERT INTO boards (name, key, position) VALUES (?, ?, ?) \
             RETURNING id, name, key, next_task_num, position",
        )
        .bind(name)
        .bind(key)
        .bind(position)
        .fetch_one(&mut *tx)
        .await?;
        for (i, column_name) in DEFAULT_COLUMNS.iter().enumerate() {
            sqlx::query("INSERT INTO columns (board_id, name, position) VALUES (?, ?, ?)")
                .bind(board.id)
                .bind(column_name)
                .bind((i as i64 + 1) * POSITION_GAP)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(board)
    }

    pub async fn rename_board(&self, id: Id, name: &str) -> Result<()> {
        sqlx::query("UPDATE boards SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn delete_board(&self, id: Id) -> Result<()> {
        sqlx::query("DELETE FROM boards WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
