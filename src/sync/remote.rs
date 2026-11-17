//! `Store` operations over the `/api/v1/sync/*` endpoints of another kgbctl
//! server, authenticated with that server's bearer token. Broker credentials
//! never leave this machine; only what a sync would write is sent.

use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, Utc};
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::wire::*;
use crate::db::{Broker, ExecutionBatch, KrakenLedgerEntry, PositionUpdate, SyncStatus};

/// Ledger entries per request, to keep bodies well under the server's limit.
const LEDGER_CHUNK: usize = 1000;

#[derive(Clone)]
pub struct RemoteStore {
    http: reqwest::Client,
    /// `https://host`, without a trailing slash or `/api/v1`.
    base: String,
    token: String,
}

impl std::fmt::Debug for RemoteStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteStore")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl RemoteStore {
    pub fn new(base: &str, token: &str) -> anyhow::Result<Self> {
        let base = base.trim_end_matches('/').trim_end_matches("/api/v1");
        if !base.starts_with("http://") && !base.starts_with("https://") {
            bail!("remote URL must start with http:// or https://: {base:?}");
        }
        if token.trim().is_empty() {
            bail!("remote token is empty");
        }
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()?,
            base: base.to_string(),
            token: token.trim().to_string(),
        })
    }

    /// Host part of the base URL, for messages.
    pub fn host(&self) -> String {
        self.base
            .split("://")
            .nth(1)
            .unwrap_or(&self.base)
            .to_string()
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> anyhow::Result<T> {
        let url = format!("{}/api/v1/sync{path}", self.base);
        let mut req = self
            .http
            .request(method.clone(), &url)
            .bearer_auth(&self.token);
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req
            .send()
            .await
            .with_context(|| format!("{method} {url}"))?;
        let status = resp.status();
        let bytes = resp.bytes().await?;
        if !status.is_success() {
            return Err(api_error(status, &bytes, &url));
        }
        if bytes.is_empty() {
            // Only `()` callers reach here; "null" parses as unit.
            return serde_json::from_str("null").map_err(Into::into);
        }
        serde_json::from_slice(&bytes).with_context(|| format!("decoding response from {url}"))
    }

    pub async fn start_sync_run(&self, broker: Broker) -> anyhow::Result<i64> {
        let r: Id = self
            .call(Method::POST, "/runs", Some(&StartRun { broker }))
            .await?;
        Ok(r.id)
    }

    pub async fn finish_sync_run(&self, run_id: i64, error: Option<&str>) -> anyhow::Result<()> {
        let body = FinishRun {
            status: match error {
                None => SyncStatus::Ok,
                Some(_) => SyncStatus::Error,
            },
            error: error.map(String::from),
        };
        self.call(Method::POST, &format!("/runs/{run_id}/finish"), Some(&body))
            .await
    }

    pub async fn upsert_account(
        &self,
        broker: Broker,
        external_id: &str,
        name: Option<&str>,
        base_currency: &str,
    ) -> anyhow::Result<i64> {
        let body = UpsertAccount {
            broker,
            external_id: external_id.into(),
            name: name.map(String::from),
            base_currency: base_currency.into(),
        };
        let r: Id = self.call(Method::POST, "/accounts", Some(&body)).await?;
        Ok(r.id)
    }

    pub async fn apply_position_sync(
        &self,
        run_id: i64,
        account_id: i64,
        broker: Broker,
        updates: &[PositionUpdate],
    ) -> anyhow::Result<()> {
        let body = ApplyPositions {
            account_id,
            broker,
            updates: updates.to_vec(),
        };
        self.call(
            Method::POST,
            &format!("/runs/{run_id}/positions"),
            Some(&body),
        )
        .await
    }

    pub async fn kraken_ledger_cursor(&self, account_id: i64) -> anyhow::Result<Option<f64>> {
        let r: LedgerCursor = self
            .call(
                Method::GET,
                &format!("/accounts/{account_id}/kraken/cursor"),
                None::<&()>,
            )
            .await?;
        Ok(r.time)
    }

    pub async fn insert_kraken_ledger(
        &self,
        account_id: i64,
        entries: &[KrakenLedgerEntry],
    ) -> anyhow::Result<u64> {
        let mut inserted = 0;
        for chunk in entries.chunks(LEDGER_CHUNK) {
            let r: Count = self
                .call(
                    Method::POST,
                    &format!("/accounts/{account_id}/kraken/ledger"),
                    Some(&chunk),
                )
                .await?;
            inserted += r.count;
        }
        Ok(inserted)
    }

    pub async fn kraken_ledger(&self, account_id: i64) -> anyhow::Result<Vec<KrakenLedgerEntry>> {
        self.call(
            Method::GET,
            &format!("/accounts/{account_id}/kraken/ledger"),
            None::<&()>,
        )
        .await
    }

    pub async fn latest_fill_time(&self, account_id: i64) -> anyhow::Result<Option<DateTime<Utc>>> {
        let r: LatestFill = self
            .call(
                Method::GET,
                &format!("/accounts/{account_id}/fills/latest"),
                None::<&()>,
            )
            .await?;
        Ok(r.executed_at)
    }

    pub async fn record_executions(
        &self,
        account_id: i64,
        broker: Broker,
        batches: &[ExecutionBatch],
    ) -> anyhow::Result<u64> {
        let body = RecordExecutions {
            broker,
            batches: batches.to_vec(),
        };
        let r: Count = self
            .call(
                Method::POST,
                &format!("/accounts/{account_id}/executions"),
                Some(&body),
            )
            .await?;
        Ok(r.count)
    }
}

/// Turn the server's `{"error": {"code", "message"}}` into a readable error.
fn api_error(status: StatusCode, body: &[u8], url: &str) -> anyhow::Error {
    let message = serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(String::from))
        .unwrap_or_else(|| String::from_utf8_lossy(body).trim().to_string());
    match status {
        StatusCode::UNAUTHORIZED => anyhow!("remote rejected the token (401): {message}"),
        StatusCode::FORBIDDEN => anyhow!(
            "remote user may not sync (403): {message}. Grant it with `kgbctl -c user-grant-sync --name <name>` on the server"
        ),
        _ => anyhow!("remote {url} returned {status}: {message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api;
    use crate::db::{
        AssetClass, Db, NewFill, NewInstrument, NewOrder, OrderSide, OrderStatus, OrderType,
    };
    use crate::sync::Store;

    /// Serve `db` on an ephemeral port; returns the base URL.
    async fn serve(db: Db) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, api::router(db)).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn btc(qty: f64, price: f64) -> PositionUpdate {
        PositionUpdate {
            instrument: NewInstrument {
                symbol: "BTC".into(),
                asset_class: AssetClass::Crypto,
                currency: "USD".into(),
                exchange: None,
                name: None,
            },
            broker_instrument_id: "XXBT".into(),
            quantity: qty,
            avg_cost: Some(100.0),
            cost_basis_usd: Some(qty * 100.0),
            market_price: Some(price),
            market_value_usd: Some(qty * price),
            unrealized_pl_usd: Some(qty * (price - 100.0)),
            unrealized_pl_pct: Some(price / 100.0 - 1.0),
            realized_pl_usd: None,
        }
    }

    fn ledger(id: &str, time: f64) -> KrakenLedgerEntry {
        KrakenLedgerEntry {
            ledger_id: id.into(),
            refid: format!("R{id}"),
            time,
            entry_type: "deposit".into(),
            subtype: None,
            asset: "XXBT".into(),
            amount: 1.0,
            fee: 0.0,
            balance: None,
            raw: Some("{}".into()),
        }
    }

    /// Everything a Kraken and an IBKR sync do, minus the brokers.
    async fn scenario(store: &Store) -> (u64, Option<f64>, usize, u64, Option<DateTime<Utc>>) {
        let run = store.start_sync_run(Broker::Kraken).await.unwrap();
        let account = store
            .upsert_account(Broker::Kraken, "main", None, "USD")
            .await
            .unwrap();
        assert_eq!(store.kraken_ledger_cursor(account).await.unwrap(), None);
        let inserted = store
            .insert_kraken_ledger(account, &[ledger("L1", 10.0), ledger("L2", 20.0)])
            .await
            .unwrap();
        let cursor = store.kraken_ledger_cursor(account).await.unwrap();
        let stored = store.kraken_ledger(account).await.unwrap().len();
        store
            .apply_position_sync(run, account, Broker::Kraken, &[btc(1.0, 90_000.0)])
            .await
            .unwrap();
        store.finish_sync_run(run, None).await.unwrap();

        let run = store.start_sync_run(Broker::Ibkr).await.unwrap();
        let ibkr = store
            .upsert_account(Broker::Ibkr, "U1", None, "USD")
            .await
            .unwrap();
        assert_eq!(store.latest_fill_time(ibkr).await.unwrap(), None);
        let at = "2026-11-15T14:30:00Z".parse::<DateTime<Utc>>().unwrap();
        let batch = ExecutionBatch {
            instrument: NewInstrument {
                symbol: "NVDA".into(),
                asset_class: AssetClass::Stock,
                currency: "USD".into(),
                exchange: Some("NASDAQ".into()),
                name: None,
            },
            broker_instrument_id: "4815747".into(),
            order: NewOrder {
                instrument_id: 0,
                broker_order_id: "42".into(),
                side: OrderSide::Buy,
                order_type: OrderType::Other,
                time_in_force: None,
                quantity: 10.0,
                limit_price: None,
                stop_price: None,
                filled_quantity: 10.0,
                avg_fill_price: Some(100.0),
                fees_usd: 0.0,
                status: OrderStatus::Filled,
                placed_at: at,
                updated_at: at,
                closed_at: Some(at),
                raw: None,
            },
            fills: vec![NewFill {
                broker_fill_id: "42.01".into(),
                quantity: 10.0,
                price: 100.0,
                fee_usd: 1.0,
                executed_at: at,
            }],
        };
        let new_fills = store
            .record_executions(ibkr, Broker::Ibkr, &[batch])
            .await
            .unwrap();
        let latest = store.latest_fill_time(ibkr).await.unwrap();
        store
            .finish_sync_run(run, Some("TWS went away"))
            .await
            .unwrap();
        (inserted, cursor, stored, new_fills, latest)
    }

    #[tokio::test]
    async fn remote_store_matches_local_store() {
        let local_db = Db::connect_in_memory().await.unwrap();
        let local = Store::from(local_db.clone());

        let remote_db = Db::connect_in_memory().await.unwrap();
        let token = api::token::generate().unwrap();
        remote_db
            .create_api_user("Laptop", &api::token::hash(&token), true)
            .await
            .unwrap();
        let base = serve(remote_db.clone()).await;
        let remote = Store::from(RemoteStore::new(&format!("{base}/api/v1/"), &token).unwrap());
        assert!(remote.is_remote());
        assert!(remote.describe().starts_with("127.0.0.1:"));

        assert_eq!(scenario(&local).await, scenario(&remote).await);

        let (local_pos, remote_pos) = (
            local_db.list_positions().await.unwrap(),
            remote_db.list_positions().await.unwrap(),
        );
        assert_eq!(local_pos.len(), 1);
        assert_eq!(local_pos[0].ticker, remote_pos[0].ticker);
        assert_eq!(
            local_pos[0].market_value_usd,
            remote_pos[0].market_value_usd
        );

        let runs = remote_db.list_sync_runs(None, 10).await.unwrap();
        assert_eq!(runs.len(), 2);
        assert!(
            runs.iter()
                .all(|r| r.pushed_by.as_deref() == Some("Laptop"))
        );
        assert_eq!(runs[0].error.as_deref(), Some("TWS went away"));
        assert!(
            local_db
                .list_sync_runs(None, 10)
                .await
                .unwrap()
                .iter()
                .all(|r| r.pushed_by.is_none())
        );
    }

    #[tokio::test]
    async fn remote_errors_are_explained() {
        let db = Db::connect_in_memory().await.unwrap();
        let reader = api::token::generate().unwrap();
        db.create_api_user("Phone", &api::token::hash(&reader), false)
            .await
            .unwrap();
        let base = serve(db).await;

        let err = RemoteStore::new(&base, "kgb_nope")
            .unwrap()
            .start_sync_run(Broker::Kraken)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("401"), "{err}");

        let err = RemoteStore::new(&base, &reader)
            .unwrap()
            .start_sync_run(Broker::Kraken)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("may not sync"), "{err}");

        assert!(RemoteStore::new("kgb.example.com", "t").is_err());
        assert!(RemoteStore::new("https://kgb.example.com", " ").is_err());
    }
}
