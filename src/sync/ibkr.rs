//! IBKR positions and executions via the TWS socket API, ported from
//! `poc/kgbpoc.py::fetch_holdings` and `poc/positions.py`.
//!
//! TWS must be running with the socket API enabled. Keep "Read-Only API" checked
//! in TWS: this module only reads. Positions are a full snapshot each sync.
//! Executions are fetched since the newest stored fill. TWS only reports recent
//! executions (about a day to a week), so frequent syncs are what build the history.

use std::collections::HashMap;

use anyhow::{Context, anyhow};
use chrono::{DateTime, Local, NaiveDateTime, TimeDelta, TimeZone, Utc};
use futures::StreamExt;
use ibapi::Client;
use ibapi::accounts::types::AccountId;
use ibapi::accounts::{AccountPortfolioValue, AccountUpdate};
use ibapi::contracts::{Contract, SecurityType};
use ibapi::orders::{CommissionReport, ExecutionData, ExecutionFilter, ExecutionSide, Executions};
use ibapi::subscriptions::{SubscriptionItem, SubscriptionItemStreamExt};

use super::{SyncOptions, SyncReport};
use crate::db::{
    AssetClass, Broker, Db, NewFill, NewInstrument, NewOrder, OrderSide, OrderStatus, OrderType,
    PositionUpdate,
};

/// Re-fetch this much before the newest stored fill; fills are idempotent.
const EXECUTION_OVERLAP: TimeDelta = TimeDelta::hours(1);

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    /// Must be unique among clients connected to the same TWS.
    pub client_id: i32,
}

pub async fn sync(
    db: &Db,
    run_id: i64,
    cfg: &Config,
    opts: SyncOptions,
) -> anyhow::Result<SyncReport> {
    let address = format!("{}:{}", cfg.host, cfg.port);
    let client = Client::connect(&address, cfg.client_id)
        .await
        .with_context(|| format!("connecting to TWS at {address}"))?;

    let mut positions = 0;
    let mut new_records = 0;
    for code in client.managed_accounts().await? {
        let account_id = db.upsert_account(Broker::Ibkr, &code, None, "USD").await?;

        let updates = portfolio(&client, &code)
            .await
            .with_context(|| format!("portfolio for {code}"))?;
        db.apply_position_sync(run_id, account_id, Broker::Ibkr, &updates)
            .await?;
        positions += updates.len();

        new_records += sync_executions(db, &client, &code, account_id, opts)
            .await
            .with_context(|| format!("executions for {code}"))?;
    }

    client.disconnect().await;
    Ok(SyncReport {
        positions,
        new_records,
        record_kind: "fills",
    })
}

/// One account's portfolio, the same source as `ib.portfolio()` in the POC.
async fn portfolio(client: &Client, code: &str) -> anyhow::Result<Vec<PositionUpdate>> {
    let mut sub = client.account_updates(&AccountId(code.into())).await?;
    let mut updates = Vec::new();
    while let Some(item) = sub.next().await {
        match item? {
            SubscriptionItem::Data(AccountUpdate::PortfolioValue(v)) => {
                updates.push(position_update(&v))
            }
            SubscriptionItem::Data(AccountUpdate::End) => break,
            _ => {}
        }
    }
    // TWS streams one account at a time; stop this one before the next is requested.
    sub.cancel().await;
    Ok(updates)
}

// Values are taken as USD, as in the POC; non-USD positions need FX conversion later.
fn position_update(v: &AccountPortfolioValue) -> PositionUpdate {
    // For options and futures, average_cost already includes the multiplier
    // while market_price is per unit, so cost basis is position * average_cost.
    let cost = v.position * v.average_cost;
    PositionUpdate {
        instrument: instrument(&v.contract),
        broker_instrument_id: v.contract.contract_id.to_string(),
        quantity: v.position,
        avg_cost: Some(v.average_cost),
        cost_basis_usd: Some(cost),
        market_price: Some(v.market_price),
        market_value_usd: Some(v.market_value),
        unrealized_pl_usd: Some(v.unrealized_pnl),
        unrealized_pl_pct: (cost != 0.0).then(|| v.unrealized_pnl / cost.abs()),
        realized_pl_usd: Some(v.realized_pnl),
    }
}

fn instrument(c: &Contract) -> NewInstrument {
    let asset_class = asset_class(&c.security_type);
    // Stocks keep the plain ticker so they match the seeded group members.
    // Everything else uses the local symbol (e.g. "AAPL  261218C00200000"),
    // because the underlying's ticker isn't unique for derivatives.
    let symbol = match asset_class {
        AssetClass::Stock => c.symbol.as_str(),
        _ if !c.local_symbol.is_empty() => &c.local_symbol,
        _ => c.symbol.as_str(),
    };
    let exchange = [c.primary_exchange.as_str(), c.exchange.as_str()]
        .into_iter()
        .find(|e| !e.is_empty())
        .map(String::from);
    NewInstrument {
        symbol: symbol.into(),
        asset_class,
        currency: c.currency.as_str().into(),
        exchange,
        name: None,
    }
}

/// IBKR reports ETFs as STK, so they map to stock (see `0002_seed_groups.sql`).
fn asset_class(t: &SecurityType) -> AssetClass {
    match t {
        SecurityType::Stock => AssetClass::Stock,
        SecurityType::Option | SecurityType::FuturesOption => AssetClass::Option,
        SecurityType::Future | SecurityType::ContinuousFuture => AssetClass::Future,
        SecurityType::Bond => AssetClass::Bond,
        SecurityType::ForexPair => AssetClass::Cash,
        SecurityType::Crypto => AssetClass::Crypto,
        _ => AssetClass::Other,
    }
}

/// Fetch executions since the newest stored fill and record them as orders
/// and fills. Returns how many fills were new.
async fn sync_executions(
    db: &Db,
    client: &Client,
    code: &str,
    account_id: i64,
    opts: SyncOptions,
) -> anyhow::Result<u64> {
    let since = match opts.full {
        true => None,
        false => db
            .latest_fill_time(account_id)
            .await?
            .map(|t| t - EXECUTION_OVERLAP),
    };
    let filter = ExecutionFilter {
        account_code: code.into(),
        // UTC form of the filter time.
        time: since
            .map(|t| t.format("%Y%m%d-%H:%M:%S").to_string())
            .unwrap_or_default(),
        ..ExecutionFilter::default()
    };

    let mut executions = Vec::new();
    let mut commissions: HashMap<String, CommissionReport> = HashMap::new();
    let mut stream = client.executions(filter).await?.filter_data();
    while let Some(item) = stream.next().await {
        match item? {
            Executions::ExecutionData(d) => executions.push(d),
            Executions::CommissionReport(c) => {
                commissions.insert(c.execution_id.clone(), c);
            }
        }
    }

    let mut new_fills = 0;
    let mut orders: HashMap<String, Vec<&ExecutionData>> = HashMap::new();
    for d in &executions {
        orders.entry(order_key(d)).or_default().push(d);
    }

    for (key, execs) in orders {
        let first = execs[0];
        let instrument_id = db.upsert_instrument(&instrument(&first.contract)).await?;
        db.link_broker_id(
            Broker::Ibkr,
            &first.contract.contract_id.to_string(),
            instrument_id,
        )
        .await?;

        let times = execs
            .iter()
            .map(|d| parse_time(&d.execution.time))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let latest = execs
            .iter()
            .max_by(|a, b| {
                a.execution
                    .cumulative_quantity
                    .total_cmp(&b.execution.cumulative_quantity)
            })
            .unwrap();
        let placed_at = *times.iter().min().unwrap();
        let updated_at = *times.iter().max().unwrap();

        // Executions carry no order type or limit prices, and only filled
        // quantity, so the order row is a summary of what was filled.
        let order_id = db
            .upsert_order(
                account_id,
                &NewOrder {
                    instrument_id,
                    broker_order_id: key,
                    side: match first.execution.side {
                        ExecutionSide::Bought => OrderSide::Buy,
                        ExecutionSide::Sold => OrderSide::Sell,
                    },
                    order_type: OrderType::Other,
                    time_in_force: None,
                    quantity: latest.execution.cumulative_quantity,
                    limit_price: None,
                    stop_price: None,
                    filled_quantity: latest.execution.cumulative_quantity,
                    avg_fill_price: Some(latest.execution.average_price),
                    fees_usd: 0.0,
                    status: OrderStatus::Filled,
                    placed_at,
                    updated_at,
                    closed_at: Some(updated_at),
                    raw: Some(format!("{:?}", latest.execution)),
                },
            )
            .await?;

        for (d, executed_at) in execs.iter().zip(&times) {
            // Commission currency is taken as USD, like the position values.
            let fee_usd = commissions
                .get(&d.execution.execution_id)
                .map_or(0.0, |c| c.commission);
            let new = db
                .insert_fill(
                    order_id,
                    &NewFill {
                        broker_fill_id: d.execution.execution_id.clone(),
                        quantity: d.execution.shares,
                        price: d.execution.price,
                        fee_usd,
                        executed_at: *executed_at,
                    },
                )
                .await?;
            new_fills += u64::from(new);
        }
        db.refresh_order_fees(order_id).await?;
    }
    Ok(new_fills)
}

/// `orders.broker_order_id` for an execution: the permId, or for trades from
/// outside IB (permId 0) the execId minus its per-fill suffix.
fn order_key(d: &ExecutionData) -> String {
    match d.execution.perm_id {
        0 => {
            let id = &d.execution.execution_id;
            id.rsplit_once('.')
                .map_or(id.as_str(), |(order, _)| order)
                .to_string()
        }
        perm => perm.to_string(),
    }
}

/// Parse an execution time: "20261115-14:30:00" (UTC) or
/// "20261115 09:30:00 US/Eastern" (zone optional; local time if missing).
fn parse_time(s: &str) -> anyhow::Result<DateTime<Utc>> {
    if let Ok(t) = NaiveDateTime::parse_from_str(s.trim(), "%Y%m%d-%H:%M:%S") {
        return Ok(t.and_utc());
    }
    let mut parts = s.split_whitespace();
    let (date, time) = (parts.next(), parts.next());
    let naive = NaiveDateTime::parse_from_str(
        &format!("{} {}", date.unwrap_or_default(), time.unwrap_or_default()),
        "%Y%m%d %H:%M:%S",
    )
    .with_context(|| format!("unrecognised execution time {s:?}"))?;
    let local = match parts.next() {
        Some(zone) => {
            let tz: chrono_tz::Tz = zone
                .parse()
                .map_err(|_| anyhow!("unknown time zone {zone:?}"))?;
            tz.from_local_datetime(&naive)
                .earliest()
                .map(|t| t.with_timezone(&Utc))
        }
        None => Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|t| t.with_timezone(&Utc)),
    };
    local.ok_or_else(|| anyhow!("nonexistent local time {s:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibapi::contracts::{Currency, Exchange, Symbol};
    use ibapi::orders::Execution;

    #[test]
    fn parses_execution_times() {
        let want = "2026-11-15T14:30:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(parse_time("20261115-14:30:00").unwrap(), want);
        assert_eq!(parse_time("20261115 09:30:00 US/Eastern").unwrap(), want);
        assert_eq!(
            parse_time("20261115  09:30:00 America/New_York").unwrap(),
            want
        );
        assert!(parse_time("yesterday").is_err());
        assert!(parse_time("20261115 09:30:00 Mars/Olympus").is_err());
    }

    #[test]
    fn order_key_prefers_perm_id() {
        let mut d = ExecutionData {
            execution: Execution {
                perm_id: 42,
                execution_id: "0000e1a7.6575b2a1.01.01".into(),
                ..Execution::default()
            },
            ..ExecutionData::default()
        };
        assert_eq!(order_key(&d), "42");
        d.execution.perm_id = 0;
        assert_eq!(order_key(&d), "0000e1a7.6575b2a1.01");
    }

    #[test]
    fn maps_contracts_to_instruments() {
        let mut c = Contract {
            symbol: Symbol("PICK".into()),
            local_symbol: "PICK".into(),
            currency: Currency("USD".into()),
            primary_exchange: Exchange("BATS".into()),
            ..Contract::default()
        };
        let i = instrument(&c);
        assert_eq!(
            (i.symbol.as_str(), i.asset_class),
            ("PICK", AssetClass::Stock)
        );
        assert_eq!(i.exchange.as_deref(), Some("BATS"));

        c.security_type = SecurityType::Option;
        c.symbol = Symbol("AAPL".into());
        c.local_symbol = "AAPL  261218C00200000".into();
        let i = instrument(&c);
        assert_eq!(
            (i.symbol.as_str(), i.asset_class),
            ("AAPL  261218C00200000", AssetClass::Option)
        );
    }
}
