// Mirrors src/api/dto.rs and src/db/types.rs on the Rust side.
// Money is USD; percentages are fractions (0.0523 = +5.23%); timestamps RFC 3339 UTC.

export type Broker = "ibkr" | "kraken";
export type AssetClass = "stock" | "etf" | "option" | "future" | "bond" | "crypto" | "cash" | "other";
export type SyncStatus = "running" | "ok" | "error";
export type OrderSide = "buy" | "sell";
export type OrderType = "market" | "limit" | "stop" | "stop_limit" | "trailing_stop" | "other";
export type OrderStatus =
  | "pending" | "open" | "partially_filled" | "filled" | "cancelled" | "rejected" | "expired";

export interface Health { status: "ok" | "degraded"; version: string; db: "ok" | "error" }

export interface Totals { value: number; basis: number; pl: number; pl_pct: number }

export interface Holding {
  ticker: string; shares: number; price: number; value: number;
  basis: number; cost: number; pl: number; pl_pct: number;
}

export interface Tab { name: string; totals: Totals; holdings: Holding[] }
export interface TabSummary { name: string; totals: Totals; holding_count: number }

export interface SyncRun {
  id: number; broker: Broker; started_at: string; finished_at: string | null;
  status: SyncStatus; error: string | null;
  /** API user that pushed the run from another machine; null for runs made on the server. */
  pushed_by: string | null;
}

export interface Source { broker: Broker; label: string; totals: Totals; last_run: SyncRun | null }

export interface Snapshot { totals: Totals; sources: Source[]; tabs: Tab[] }

export interface Position {
  id: number; broker: Broker; account: string; ticker: string; asset_class: AssetClass;
  quantity: number; market_price: number | null; market_value_usd: number | null;
  avg_cost: number | null; cost_basis_usd: number | null;
  unrealized_pl_usd: number | null; unrealized_pl_pct: number | null; updated_at: string;
}

export interface PositionSnapshot {
  taken_at: string; quantity: number; avg_cost: number | null; market_price: number | null;
  market_value_usd: number | null; unrealized_pl_usd: number | null; unrealized_pl_pct: number | null;
}

export interface Account {
  id: number; broker: Broker; external_id: string; name: string | null;
  base_currency: string; created_at: string;
}

export interface GroupTotals {
  value_usd: number | null; cost_usd: number | null; pl_usd: number | null; pl_pct: number | null;
}
export interface Group { id: number; name: string; description: string | null; totals: GroupTotals | null }

export interface Instrument {
  id: number; symbol: string; asset_class: AssetClass; currency: string;
  exchange: string | null; name: string | null;
}
export interface GroupDetail { id: number; name: string; description: string | null; members: Instrument[] }

export interface Order {
  id: number; account_id: number; instrument_id: number; broker_order_id: string;
  side: OrderSide; order_type: OrderType; time_in_force: string | null; quantity: number;
  limit_price: number | null; stop_price: number | null; filled_quantity: number;
  avg_fill_price: number | null; fees_usd: number; status: OrderStatus;
  placed_at: string; updated_at: string; closed_at: string | null;
}

export interface RotatedToken { name: string; token: string }

export const BROKER_LABEL: Record<Broker, string> = { ibkr: "Interactive Brokers", kraken: "Kraken" };
