//! Where a sync writes: the local database, or a remote kgbctl server reached
//! through its API. Every operation a broker sync needs is here, so the broker
//! code is the same for both.

use chrono::{DateTime, Utc};

use super::remote::RemoteStore;
use crate::db::{Broker, Db, ExecutionBatch, KrakenLedgerEntry, PositionUpdate};

#[derive(Clone, Debug)]
pub enum Store {
    Local(Db),
    Remote(RemoteStore),
}

impl From<Db> for Store {
    fn from(db: Db) -> Self {
        Self::Local(db)
    }
}

impl From<RemoteStore> for Store {
    fn from(remote: RemoteStore) -> Self {
        Self::Remote(remote)
    }
}

impl Store {
    /// Where the data goes, for messages: the remote host or "local database".
    pub fn describe(&self) -> String {
        match self {
            Self::Local(_) => "local database".into(),
            Self::Remote(r) => r.host(),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }

    pub async fn start_sync_run(&self, broker: Broker) -> anyhow::Result<i64> {
        match self {
            Self::Local(db) => Ok(db.start_sync_run(broker, None).await?),
            Self::Remote(r) => r.start_sync_run(broker).await,
        }
    }

    pub async fn finish_sync_run(&self, run_id: i64, error: Option<&str>) -> anyhow::Result<()> {
        match self {
            Self::Local(db) => {
                match error {
                    None => db.finish_sync_run_ok(run_id).await?,
                    Some(e) => db.finish_sync_run_err(run_id, e).await?,
                }
                Ok(())
            }
            Self::Remote(r) => r.finish_sync_run(run_id, error).await,
        }
    }

    pub async fn upsert_account(
        &self,
        broker: Broker,
        external_id: &str,
        name: Option<&str>,
        base_currency: &str,
    ) -> anyhow::Result<i64> {
        match self {
            Self::Local(db) => Ok(db
                .upsert_account(broker, external_id, name, base_currency)
                .await?),
            Self::Remote(r) => {
                r.upsert_account(broker, external_id, name, base_currency)
                    .await
            }
        }
    }

    pub async fn apply_position_sync(
        &self,
        run_id: i64,
        account_id: i64,
        broker: Broker,
        updates: &[PositionUpdate],
    ) -> anyhow::Result<()> {
        match self {
            Self::Local(db) => Ok(db
                .apply_position_sync(run_id, account_id, broker, updates)
                .await?),
            Self::Remote(r) => {
                r.apply_position_sync(run_id, account_id, broker, updates)
                    .await
            }
        }
    }

    pub async fn kraken_ledger_cursor(&self, account_id: i64) -> anyhow::Result<Option<f64>> {
        match self {
            Self::Local(db) => Ok(db.kraken_ledger_cursor(account_id).await?),
            Self::Remote(r) => r.kraken_ledger_cursor(account_id).await,
        }
    }

    pub async fn insert_kraken_ledger(
        &self,
        account_id: i64,
        entries: &[KrakenLedgerEntry],
    ) -> anyhow::Result<u64> {
        match self {
            Self::Local(db) => Ok(db.insert_kraken_ledger(account_id, entries).await?),
            Self::Remote(r) => r.insert_kraken_ledger(account_id, entries).await,
        }
    }

    /// Every stored ledger entry, oldest first. The remote store leaves `raw` empty.
    pub async fn kraken_ledger(&self, account_id: i64) -> anyhow::Result<Vec<KrakenLedgerEntry>> {
        match self {
            Self::Local(db) => Ok(db.kraken_ledger(account_id).await?),
            Self::Remote(r) => r.kraken_ledger(account_id).await,
        }
    }

    pub async fn latest_fill_time(&self, account_id: i64) -> anyhow::Result<Option<DateTime<Utc>>> {
        match self {
            Self::Local(db) => Ok(db.latest_fill_time(account_id).await?),
            Self::Remote(r) => r.latest_fill_time(account_id).await,
        }
    }

    pub async fn record_executions(
        &self,
        account_id: i64,
        broker: Broker,
        batches: &[ExecutionBatch],
    ) -> anyhow::Result<u64> {
        match self {
            Self::Local(db) => Ok(db.record_executions(account_id, broker, batches).await?),
            Self::Remote(r) => r.record_executions(account_id, broker, batches).await,
        }
    }
}
