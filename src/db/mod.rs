mod activities;
mod boards;
mod columns;
mod labels;
pub mod tasks;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

/// Cheap to clone (inner pool is an Arc); commands grab a clone and run on tokio.
#[derive(Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn connect(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .with_context(|| format!("opening database at {}", path.display()))?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    /// Single-connection in-memory database (each sqlite memory connection is
    /// its own database, so the pool must never grow past one).
    #[cfg(test)]
    pub async fn connect_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// Resolve the database file path: `TASKO_DB` env override, otherwise the
/// platform data dir (created if missing).
pub fn default_db_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("TASKO_DB")
        && !path.is_empty()
    {
        return Ok(PathBuf::from(path));
    }
    let dirs = ProjectDirs::from("", "", "tasko")
        .context("could not determine a data directory for this platform")?;
    std::fs::create_dir_all(dirs.data_dir())
        .with_context(|| format!("creating data dir {}", dirs.data_dir().display()))?;
    Ok(dirs.data_dir().join("tasko.db"))
}

#[cfg(test)]
mod tests;
