use anyhow::Result;
use chrono::Utc;

use super::Db;
use crate::domain::{Activity, ActivityKind, Id};

impl Db {
    pub async fn log_activity(&self, task_id: Id, kind: ActivityKind, detail: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO activities (task_id, kind, detail, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(task_id)
        .bind(kind.as_str())
        .bind(detail)
        .bind(Utc::now())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn activities_for_task(&self, task_id: Id) -> Result<Vec<Activity>> {
        Ok(sqlx::query_as(
            "SELECT id, task_id, kind, detail, created_at FROM activities \
             WHERE task_id = ? ORDER BY created_at DESC, id DESC LIMIT 200",
        )
        .bind(task_id)
        .fetch_all(self.pool())
        .await?)
    }
}
