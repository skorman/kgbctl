//! Pull positions and trade history from the brokers into a `Store`: the local
//! database, or a remote kgbctl server's database through its API.
//!
//! Each broker sync is recorded as a `sync_runs` row, and the brokers run
//! independently: one failing doesn't stop the other. History that's already
//! stored isn't fetched again. Each sync resumes from a cursor taken from the
//! stored data (newest Kraken ledger entry, newest IBKR fill), so no separate
//! cursor state can drift.

pub mod ibkr;
pub mod kraken;
pub mod remote;
mod store;
pub mod wire;

use futures::future::join_all;

pub use remote::RemoteStore;
pub use store::Store;

use crate::db::Broker;

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
    store: &Store,
    broker: Broker,
    config: &Config,
    opts: SyncOptions,
) -> anyhow::Result<SyncReport> {
    let run_id = store.start_sync_run(broker).await?;
    let result = match broker {
        Broker::Kraken => match &config.kraken {
            Some(cfg) => kraken::sync(store, run_id, cfg, opts).await,
            None => Err(anyhow::anyhow!(
                "KRAKEN_API_KEY and KRAKEN_SECRET_KEY must be set"
            )),
        },
        Broker::Ibkr => ibkr::sync(store, run_id, &config.ibkr, opts).await,
    };
    let error = result.as_ref().err().map(|e| format!("{e:#}"));
    store.finish_sync_run(run_id, error.as_deref()).await?;
    result
}

/// Sync `brokers` concurrently. Every broker gets a result, even when another fails.
pub async fn run_all(
    store: &Store,
    brokers: &[Broker],
    config: &Config,
    opts: SyncOptions,
) -> Vec<(Broker, anyhow::Result<SyncReport>)> {
    let results = join_all(brokers.iter().map(|&b| run(store, b, config, opts))).await;
    brokers.iter().copied().zip(results).collect()
}
