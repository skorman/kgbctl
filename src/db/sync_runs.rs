use chrono::{DateTime, Utc};

use super::{Broker, Db, SyncRun, SyncStatus};

impl Db {
    /// Record the start of a sync attempt; returns the run id.
    pub async fn start_sync_run(&self, broker: Broker) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            "INSERT INTO sync_runs (broker) VALUES (?) RETURNING id",
            broker
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn finish_sync_run_ok(&self, id: i64) -> sqlx::Result<()> {
        self.finish_sync_run(id, SyncStatus::Ok, None).await
    }

    pub async fn finish_sync_run_err(&self, id: i64, error: &str) -> sqlx::Result<()> {
        self.finish_sync_run(id, SyncStatus::Error, Some(error))
            .await
    }

    async fn finish_sync_run(
        &self,
        id: i64,
        status: SyncStatus,
        error: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            r#"
            UPDATE sync_runs
            SET finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), status = ?, error = ?
            WHERE id = ?
            "#,
            status,
            error,
            id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The most recent sync attempt for `broker`, if any.
    pub async fn last_sync_run(&self, broker: Broker) -> sqlx::Result<Option<SyncRun>> {
        sqlx::query_as!(
            SyncRun,
            r#"
            SELECT id, broker AS "broker: Broker",
                   started_at AS "started_at: DateTime<Utc>",
                   finished_at AS "finished_at: DateTime<Utc>",
                   status AS "status: SyncStatus", error
            FROM sync_runs
            WHERE broker = ?
            ORDER BY id DESC
            LIMIT 1
            "#,
            broker,
        )
        .fetch_optional(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sync_run_lifecycle() {
        let db = Db::connect_in_memory().await.unwrap();

        let id = db.start_sync_run(Broker::Kraken).await.unwrap();
        let run = db.last_sync_run(Broker::Kraken).await.unwrap().unwrap();
        assert_eq!(run.status, SyncStatus::Running);
        assert!(run.finished_at.is_none());

        db.finish_sync_run_err(id, "timeout").await.unwrap();
        let run = db.last_sync_run(Broker::Kraken).await.unwrap().unwrap();
        assert_eq!(run.status, SyncStatus::Error);
        assert_eq!(run.error.as_deref(), Some("timeout"));
        assert!(run.finished_at.is_some());

        assert!(db.last_sync_run(Broker::Ibkr).await.unwrap().is_none());
    }
}
