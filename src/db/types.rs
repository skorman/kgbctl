//! Row types and TEXT-backed enums mirroring the CHECK constraints in
//! `migrations/0001_init.sql`.

use chrono::{DateTime, Utc};

#[derive(
    sqlx::Type, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash,
)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Broker {
    Ibkr,
    Kraken,
}

#[derive(sqlx::Type, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Stock,
    Etf,
    Option,
    Future,
    Bond,
    Crypto,
    Cash,
    Other,
}

#[derive(sqlx::Type, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Running,
    Ok,
    Error,
}

#[derive(sqlx::Type, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(sqlx::Type, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
    TrailingStop,
    Other,
}

#[derive(sqlx::Type, serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub id: i64,
    pub broker: Broker,
    pub external_id: String,
    pub name: Option<String>,
    pub base_currency: String,
    pub created_at: DateTime<Utc>,
}

/// Someone allowed to call the API. The token itself is never loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct ApiUser {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
    /// May push sync data through the `/sync/*` endpoints.
    pub can_sync: bool,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct Instrument {
    pub id: i64,
    pub symbol: String,
    pub asset_class: AssetClass,
    pub currency: String,
    pub exchange: Option<String>,
    pub name: Option<String>,
}

/// An instrument as reported by a broker, before it has a database id.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct NewInstrument {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub currency: String,
    pub exchange: Option<String>,
    pub name: Option<String>,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct SyncRun {
    pub id: i64,
    pub broker: Broker,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: SyncStatus,
    pub error: Option<String>,
    /// API user that pushed the run from another machine; `None` for local runs.
    pub pushed_by: Option<String>,
}

/// One position as reported by a broker during a sync.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct PositionUpdate {
    pub instrument: NewInstrument,
    /// Broker's identifier for the instrument (IBKR conId, Kraken asset code).
    pub broker_instrument_id: String,
    pub quantity: f64,
    pub avg_cost: Option<f64>,
    pub cost_basis_usd: Option<f64>,
    pub market_price: Option<f64>,
    pub market_value_usd: Option<f64>,
    pub unrealized_pl_usd: Option<f64>,
    pub unrealized_pl_pct: Option<f64>,
    pub realized_pl_usd: Option<f64>,
}

/// A row of `v_positions`: what the TUI positions table shows.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct PositionView {
    pub id: i64,
    pub broker: Broker,
    pub account: String,
    pub ticker: String,
    pub asset_class: AssetClass,
    pub quantity: f64,
    pub market_price: Option<f64>,
    pub market_value_usd: Option<f64>,
    pub avg_cost: Option<f64>,
    pub cost_basis_usd: Option<f64>,
    pub unrealized_pl_usd: Option<f64>,
    pub unrealized_pl_pct: Option<f64>,
    pub updated_at: DateTime<Utc>,
}

/// A row of `position_snapshots`: one position's state at one sync.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct PositionSnapshot {
    pub taken_at: DateTime<Utc>,
    pub quantity: f64,
    pub avg_cost: Option<f64>,
    pub market_price: Option<f64>,
    pub market_value_usd: Option<f64>,
    pub unrealized_pl_usd: Option<f64>,
    pub unrealized_pl_pct: Option<f64>,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct PositionGroup {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A row of `v_group_totals`. Only groups with at least one open position appear.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupTotal {
    pub name: String,
    pub value_usd: Option<f64>,
    pub cost_usd: Option<f64>,
    pub pl_usd: Option<f64>,
    pub pl_pct: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub id: i64,
    pub account_id: i64,
    pub instrument_id: i64,
    pub broker_order_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub time_in_force: Option<String>,
    pub quantity: f64,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub filled_quantity: f64,
    pub avg_fill_price: Option<f64>,
    pub fees_usd: f64,
    pub status: OrderStatus,
    pub placed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub raw: Option<String>,
}

/// An order as reported by a broker; upserted on `(account_id, broker_order_id)`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct NewOrder {
    pub instrument_id: i64,
    pub broker_order_id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub time_in_force: Option<String>,
    pub quantity: f64,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub filled_quantity: f64,
    pub avg_fill_price: Option<f64>,
    pub fees_usd: f64,
    pub status: OrderStatus,
    pub placed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub raw: Option<String>,
}

/// One row of Kraken's `Ledgers` history, stored as reported.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct KrakenLedgerEntry {
    pub ledger_id: String,
    pub refid: String,
    /// Unix seconds.
    pub time: f64,
    pub entry_type: String,
    pub subtype: Option<String>,
    /// Raw asset code: XXBT, ZUSD, ETH.S, ...
    pub asset: String,
    pub amount: f64,
    pub fee: f64,
    pub balance: Option<f64>,
    pub raw: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct NewFill {
    pub broker_fill_id: String,
    pub quantity: f64,
    pub price: f64,
    pub fee_usd: f64,
    pub executed_at: DateTime<Utc>,
}

/// One order and its fills as reported by a broker, with the instrument
/// described rather than referenced, so it can be recorded in any database
/// (local or remote) in a single transaction.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ExecutionBatch {
    pub instrument: NewInstrument,
    /// Broker's identifier for the instrument (IBKR conId).
    pub broker_instrument_id: String,
    /// `instrument_id` is ignored and resolved from `instrument` on insert.
    pub order: NewOrder,
    pub fills: Vec<NewFill>,
}
