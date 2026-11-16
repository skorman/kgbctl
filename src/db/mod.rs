//! SQLite persistence for kgbctl, built on sqlx.
//!
//! Queries use the compile-time checked `query!` / `query_as!` macros. They are
//! checked against `DATABASE_URL` (see `.env`) when online, or against the
//! committed `.sqlx/` cache when `SQLX_OFFLINE=true`.

mod accounts;
mod groups;
mod instruments;
mod kraken_ledger;
mod orders;
mod positions;
mod sync_runs;
mod types;

use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

pub use types::*;

#[derive(Clone, Debug)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Open (creating if needed) the database file at `path`.
    pub async fn connect(path: &Path) -> sqlx::Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    /// A fresh, fully migrated in-memory database. A single connection that is
    /// never recycled, so every query sees the same database.
    #[cfg(test)]
    pub async fn connect_in_memory() -> sqlx::Result<Self> {
        let options = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    /// Apply any pending migrations from `./migrations` (embedded at build time).
    pub async fn migrate(&self) -> sqlx::Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }
}

/// Format a timestamp the same way the schema's `strftime('%Y-%m-%dT%H:%M:%fZ')`
/// defaults do, so stored TEXT timestamps sort and compare consistently.
fn ts(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}
