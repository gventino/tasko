use anyhow::Result;

use super::Db;
use crate::domain::{Column, Id, POSITION_GAP};

impl Db {
    pub async fn columns_for_board(&self, board_id: Id) -> Result<Vec<Column>> {
        Ok(sqlx::query_as(
            "SELECT id, board_id, name, position, wip_limit FROM columns \
             WHERE board_id = ? ORDER BY position, id",
        )
        .bind(board_id)
        .fetch_all(self.pool())
        .await?)
    }

    pub async fn get_column(&self, id: Id) -> Result<Option<Column>> {
        Ok(sqlx::query_as(
            "SELECT id, board_id, name, position, wip_limit FROM columns WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?)
    }

    pub async fn create_column(&self, board_id: Id, name: &str) -> Result<Column> {
        Ok(sqlx::query_as(
            "INSERT INTO columns (board_id, name, position) \
             VALUES (?, ?, (SELECT COALESCE(MAX(position), 0) + ? FROM columns WHERE board_id = ?)) \
             RETURNING id, board_id, name, position, wip_limit",
        )
            .bind(board_id)
            .bind(name)
            .bind(POSITION_GAP)
            .bind(board_id)
            .fetch_one(self.pool())
            .await?)
    }

    pub async fn rename_column(&self, id: Id, name: &str) -> Result<()> {
        sqlx::query("UPDATE columns SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn set_wip_limit(&self, id: Id, wip_limit: Option<i64>) -> Result<()> {
        sqlx::query("UPDATE columns SET wip_limit = ? WHERE id = ?")
            .bind(wip_limit)
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Swap the positions of two columns atomically.
    pub async fn swap_column_positions(&self, a: Id, a_pos: i64, b: Id, b_pos: i64) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("UPDATE columns SET position = ? WHERE id = ?")
            .bind(b_pos)
            .bind(a)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE columns SET position = ? WHERE id = ?")
            .bind(a_pos)
            .bind(b)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Rewrite column positions with fresh gaps; defensive counterpart of
    /// `renumber_tasks` for columns.
    #[allow(dead_code)]
    pub async fn renumber_columns(&self, board_id: Id) -> Result<Vec<Column>> {
        let columns = self.columns_for_board(board_id).await?;
        let mut tx = self.pool().begin().await?;
        for (i, column) in columns.iter().enumerate() {
            sqlx::query("UPDATE columns SET position = ? WHERE id = ?")
                .bind((i as i64 + 1) * POSITION_GAP)
                .bind(column.id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        self.columns_for_board(board_id).await
    }

    pub async fn delete_column(&self, id: Id) -> Result<()> {
        sqlx::query("DELETE FROM columns WHERE id = ?")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
