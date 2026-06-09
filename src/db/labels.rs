use anyhow::Result;

use super::Db;
use crate::domain::{Id, Label};

impl Db {
    pub async fn labels_for_board(&self, board_id: Id) -> Result<Vec<Label>> {
        Ok(sqlx::query_as(
            "SELECT id, board_id, name, color FROM labels WHERE board_id = ? ORDER BY name, id",
        )
        .bind(board_id)
        .fetch_all(self.pool())
        .await?)
    }

    pub async fn create_label(&self, board_id: Id, name: &str, color: i64) -> Result<Label> {
        Ok(sqlx::query_as(
            "INSERT INTO labels (board_id, name, color) VALUES (?, ?, ?) \
             RETURNING id, board_id, name, color",
        )
        .bind(board_id)
        .bind(name)
        .bind(color)
        .fetch_one(self.pool())
        .await?)
    }

    pub async fn delete_label(&self, id: Id) -> Result<()> {
        sqlx::query("DELETE FROM labels WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// All (task_id, label_id) pairs of a board, to build the in-memory map.
    pub async fn task_label_pairs(&self, board_id: Id) -> Result<Vec<(Id, Id)>> {
        Ok(sqlx::query_as(
            "SELECT tl.task_id, tl.label_id FROM task_labels tl \
             JOIN labels l ON l.id = tl.label_id WHERE l.board_id = ?",
        )
        .bind(board_id)
        .fetch_all(self.pool())
        .await?)
    }

    pub async fn set_task_labels(&self, task_id: Id, label_ids: &[Id]) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("DELETE FROM task_labels WHERE task_id = ?")
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        for label_id in label_ids {
            sqlx::query("INSERT INTO task_labels (task_id, label_id) VALUES (?, ?)")
                .bind(task_id)
                .bind(label_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
