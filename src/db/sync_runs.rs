use chrono::{DateTime, Utc};

use super::{Broker, Db, SyncRun, SyncStatus};

impl Db {
    /// Record the start of a sync attempt; returns the run id. `pushed_by` is
    /// the API user pushing the run from another machine, `None` for local runs.
    pub async fn start_sync_run(
        &self,
        broker: Broker,
        pushed_by: Option<&str>,
    ) -> sqlx::Result<i64> {
        sqlx::query_scalar!(
            "INSERT INTO sync_runs (broker, pushed_by) VALUES (?, ?) RETURNING id",
            broker,
            pushed_by,
        )
        .fetch_one(&self.pool)
        .await
    }

    /// Fail every run of `broker` still marked running. A remote client that
    /// dies mid-sync can't finish its run, so the next one closes it out.
    /// Returns how many runs were closed.
    pub async fn supersede_running_syncs(&self, broker: Broker) -> sqlx::Result<u64> {
        let result = sqlx::query!(
            r#"
            UPDATE sync_runs
            SET finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), status = 'error',
                error = 'superseded by a new run'
            WHERE broker = ? AND status = 'running'
            "#,
            broker,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// One run by id, if it exists.
    pub async fn sync_run(&self, id: i64) -> sqlx::Result<Option<SyncRun>> {
        sqlx::query_as!(
            SyncRun,
            r#"
            SELECT id, broker AS "broker: Broker",
                   started_at AS "started_at: DateTime<Utc>",
                   finished_at AS "finished_at: DateTime<Utc>",
                   status AS "status: SyncStatus", error, pushed_by
            FROM sync_runs
            WHERE id = ?
            "#,
            id,
        )
        .fetch_optional(&self.pool)
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
                   status AS "status: SyncStatus", error, pushed_by
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

    /// Recent sync attempts, newest first, for one broker or all.
    pub async fn list_sync_runs(
        &self,
        broker: Option<Broker>,
        limit: i64,
    ) -> sqlx::Result<Vec<SyncRun>> {
        sqlx::query_as!(
            SyncRun,
            r#"
            SELECT id, broker AS "broker: Broker",
                   started_at AS "started_at: DateTime<Utc>",
                   finished_at AS "finished_at: DateTime<Utc>",
                   status AS "status: SyncStatus", error, pushed_by
            FROM sync_runs
            WHERE ?1 IS NULL OR broker = ?1
            ORDER BY id DESC
            LIMIT ?2
            "#,
            broker,
            limit,
        )
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sync_run_lifecycle() {
        let db = Db::connect_in_memory().await.unwrap();

        let id = db.start_sync_run(Broker::Kraken, None).await.unwrap();
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

    #[tokio::test]
    async fn list_sync_runs_filters_and_limits() {
        let db = Db::connect_in_memory().await.unwrap();
        let k1 = db.start_sync_run(Broker::Kraken, None).await.unwrap();
        let i1 = db.start_sync_run(Broker::Ibkr, None).await.unwrap();
        let k2 = db.start_sync_run(Broker::Kraken, None).await.unwrap();

        let ids = |runs: Vec<SyncRun>| runs.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(
            ids(db.list_sync_runs(None, 10).await.unwrap()),
            [k2, i1, k1]
        );
        assert_eq!(
            ids(db.list_sync_runs(Some(Broker::Kraken), 10).await.unwrap()),
            [k2, k1]
        );
        assert_eq!(ids(db.list_sync_runs(None, 1).await.unwrap()), [k2]);
    }

    #[tokio::test]
    async fn superseding_closes_only_running_runs_of_that_broker() {
        let db = Db::connect_in_memory().await.unwrap();
        let done = db
            .start_sync_run(Broker::Kraken, Some("laptop"))
            .await
            .unwrap();
        db.finish_sync_run_ok(done).await.unwrap();
        let stale = db
            .start_sync_run(Broker::Kraken, Some("laptop"))
            .await
            .unwrap();
        let other = db.start_sync_run(Broker::Ibkr, None).await.unwrap();

        assert_eq!(db.supersede_running_syncs(Broker::Kraken).await.unwrap(), 1);
        let stale = db.sync_run(stale).await.unwrap().unwrap();
        assert_eq!(stale.status, SyncStatus::Error);
        assert_eq!(stale.pushed_by.as_deref(), Some("laptop"));
        assert_eq!(
            db.sync_run(done).await.unwrap().unwrap().status,
            SyncStatus::Ok
        );
        assert_eq!(
            db.sync_run(other).await.unwrap().unwrap().status,
            SyncStatus::Running
        );
        assert!(db.sync_run(999).await.unwrap().is_none());
    }
}
