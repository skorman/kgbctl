//! JSON response shapes. Row types that serialize as-is (`Account`, `Instrument`,
//! `SyncRun`, `PositionView`, `PositionSnapshot`) are returned directly; these are
//! the ones that add computed fields or drop columns.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::db::{
    Broker, GroupTotal, Instrument, Order, OrderSide, OrderStatus, OrderType, PositionGroup,
    SyncRun,
};
use crate::portfolio::{Holding, Snapshot, Source, Tab, Totals};

#[derive(Serialize)]
pub struct HealthDto {
    pub status: &'static str,
    pub version: &'static str,
    pub db: &'static str,
}

#[derive(Serialize)]
pub struct RotatedTokenDto {
    pub name: String,
    /// The caller's new token; the one used for the request no longer works.
    pub token: String,
}

#[derive(Serialize)]
pub struct TotalsDto {
    pub value: f64,
    pub basis: f64,
    pub pl: f64,
    pub pl_pct: f64,
}

impl From<Totals> for TotalsDto {
    fn from(t: Totals) -> Self {
        Self {
            value: t.value,
            basis: t.basis,
            pl: t.pl(),
            pl_pct: t.pl_pct(),
        }
    }
}

#[derive(Serialize)]
pub struct HoldingDto {
    pub ticker: String,
    pub shares: f64,
    pub price: f64,
    pub value: f64,
    pub basis: f64,
    pub cost: f64,
    pub pl: f64,
    pub pl_pct: f64,
}

impl From<Holding> for HoldingDto {
    fn from(h: Holding) -> Self {
        Self {
            cost: h.cost(),
            pl: h.value - h.basis,
            pl_pct: h.pl_pct(),
            ticker: h.ticker,
            shares: h.shares,
            price: h.price,
            value: h.value,
            basis: h.basis,
        }
    }
}

#[derive(Serialize)]
pub struct TabDto {
    pub name: String,
    pub totals: TotalsDto,
    pub holdings: Vec<HoldingDto>,
}

impl From<Tab> for TabDto {
    fn from(t: Tab) -> Self {
        Self {
            name: t.name,
            totals: t.totals.into(),
            holdings: t.holdings.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize)]
pub struct TabSummaryDto {
    pub name: String,
    pub totals: TotalsDto,
    pub holding_count: usize,
}

impl From<&Tab> for TabSummaryDto {
    fn from(t: &Tab) -> Self {
        Self {
            name: t.name.clone(),
            totals: t.totals.into(),
            holding_count: t.holdings.len(),
        }
    }
}

#[derive(Serialize)]
pub struct SourceDto {
    pub broker: Broker,
    pub label: &'static str,
    pub totals: TotalsDto,
    pub last_run: Option<SyncRun>,
}

impl From<Source> for SourceDto {
    fn from(s: Source) -> Self {
        Self {
            label: s.label(),
            broker: s.broker,
            totals: s.totals.into(),
            last_run: s.last_run,
        }
    }
}

#[derive(Serialize)]
pub struct SnapshotDto {
    pub totals: TotalsDto,
    pub sources: Vec<SourceDto>,
    pub tabs: Vec<TabDto>,
}

impl From<Snapshot> for SnapshotDto {
    fn from(s: Snapshot) -> Self {
        Self {
            totals: s.totals.into(),
            sources: s.sources.into_iter().map(Into::into).collect(),
            tabs: s.tabs.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize)]
pub struct GroupTotalsDto {
    pub value_usd: Option<f64>,
    pub cost_usd: Option<f64>,
    pub pl_usd: Option<f64>,
    pub pl_pct: Option<f64>,
}

impl From<GroupTotal> for GroupTotalsDto {
    fn from(t: GroupTotal) -> Self {
        Self {
            value_usd: t.value_usd,
            cost_usd: t.cost_usd,
            pl_usd: t.pl_usd,
            pl_pct: t.pl_pct,
        }
    }
}

#[derive(Serialize)]
pub struct GroupDto {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    /// `null` when the group has no open positions.
    pub totals: Option<GroupTotalsDto>,
}

#[derive(Serialize)]
pub struct GroupDetailDto {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub members: Vec<Instrument>,
}

impl GroupDetailDto {
    pub fn new(g: PositionGroup, members: Vec<Instrument>) -> Self {
        Self {
            id: g.id,
            name: g.name,
            description: g.description,
            members,
        }
    }
}

/// An `Order` without the raw broker payload.
#[derive(Serialize)]
pub struct OrderDto {
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
}

impl From<Order> for OrderDto {
    fn from(o: Order) -> Self {
        Self {
            id: o.id,
            account_id: o.account_id,
            instrument_id: o.instrument_id,
            broker_order_id: o.broker_order_id,
            side: o.side,
            order_type: o.order_type,
            time_in_force: o.time_in_force,
            quantity: o.quantity,
            limit_price: o.limit_price,
            stop_price: o.stop_price,
            filled_quantity: o.filled_quantity,
            avg_fill_price: o.avg_fill_price,
            fees_usd: o.fees_usd,
            status: o.status,
            placed_at: o.placed_at,
            updated_at: o.updated_at,
            closed_at: o.closed_at,
        }
    }
}
