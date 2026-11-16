//! Pull positions and trade history from the brokers into the database.
//!
//! Each broker sync is recorded as a `sync_runs` row, and the brokers run
//! independently: one failing doesn't stop the other. History that's already
//! stored isn't fetched again. Each sync resumes from a cursor taken from the
//! stored data (newest Kraken ledger entry, newest IBKR fill), so no separate
//! cursor state can drift.

pub mod ibkr;
pub mod kraken;

use futures::future::join_all;

use crate::db::{Broker, Db};

#[derive(Clone, Copy, Debug, Default)]
pub struct SyncOptions {
    /// Ignore the stored cursors and fetch all available history.
    pub full: bool,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub kraken: Option<kraken::Config>,
    pub ibkr: ibkr::Config,
}

/// What one broker sync wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct SyncReport {
    pub positions: usize,
    /// New history rows: Kraken ledger entries or IBKR fills.
    pub new_records: u64,
    pub record_kind: &'static str,
}

/// Sync one broker, recording the attempt in `sync_runs`.
pub async fn run(
    db: &Db,
    broker: Broker,
    config: &Config,
    opts: SyncOptions,
) -> anyhow::Result<SyncReport> {
    let run_id = db.start_sync_run(broker).await?;
    let result = match broker {
        Broker::Kraken => match &config.kraken {
            Some(cfg) => kraken::sync(db, run_id, cfg, opts).await,
            None => Err(anyhow::anyhow!(
                "KRAKEN_API_KEY and KRAKEN_SECRET_KEY must be set"
            )),
        },
        Broker::Ibkr => ibkr::sync(db, run_id, &config.ibkr, opts).await,
    };
    match &result {
        Ok(_) => db.finish_sync_run_ok(run_id).await?,
        Err(e) => db.finish_sync_run_err(run_id, &format!("{e:#}")).await?,
    }
    result
}

/// Sync `brokers` concurrently. Every broker gets a result, even when another fails.
pub async fn run_all(
    db: &Db,
    brokers: &[Broker],
    config: &Config,
    opts: SyncOptions,
) -> Vec<(Broker, anyhow::Result<SyncReport>)> {
    let results = join_all(brokers.iter().map(|&b| run(db, b, config, opts))).await;
    brokers.iter().copied().zip(results).collect()
}
