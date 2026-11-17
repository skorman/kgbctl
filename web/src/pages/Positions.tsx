import { useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { useAccounts, usePositions } from "../api/hooks";
import type { Broker, Position } from "../api/types";
import { BrokerPill, Card, ErrorBanner, PL } from "../components/Common";
import { Loading } from "../components/Splash";
import { money, num, price, signedMoney, signedPct, ago, titleCase } from "../lib/format";

type Key = keyof Position;
const COLS: { key: Key; label: string; left?: boolean }[] = [
  { key: "ticker", label: "Ticker", left: true }, { key: "broker", label: "Broker", left: true }, { key: "account", label: "Account", left: true },
  { key: "asset_class", label: "Class", left: true }, { key: "quantity", label: "Qty" }, { key: "market_price", label: "Price" },
  { key: "avg_cost", label: "Avg cost" }, { key: "market_value_usd", label: "Value" }, { key: "cost_basis_usd", label: "Basis" },
  { key: "unrealized_pl_usd", label: "P/L $" }, { key: "unrealized_pl_pct", label: "P/L %" }, { key: "updated_at", label: "Updated" },
];

/** Raw per-account positions from /positions, with the API's broker/account filters. */
export function Positions() {
  const [params, setParams] = useSearchParams();
  const broker = (params.get("broker") as Broker | null) ?? undefined;
  const account = params.get("account") ?? undefined;
  const q = params.get("q") ?? "";
  const positions = usePositions({ broker, account });
  const accounts = useAccounts();
  const nav = useNavigate();
  const [sort, setSort] = useState<{ key: Key; dir: 1 | -1 }>({ key: "market_value_usd", dir: -1 });

  const rows = useMemo(() => {
    const list = (positions.data ?? []).filter((p) => !q || p.ticker.toLowerCase().includes(q.toLowerCase()));
    return list.sort((a, b) => {
      const x = a[sort.key], y = b[sort.key];
      if (x == null) return 1; if (y == null) return -1;
      const c = typeof x === "string" ? x.localeCompare(String(y)) : Number(x) - Number(y);
      return c * sort.dir;
    });
  }, [positions.data, sort, q]);
  const set = (k: string, v: string) => { const p = new URLSearchParams(params); v ? p.set(k, v) : p.delete(k); setParams(p, { replace: true }); };
  const totals = rows.reduce((a, p) => ({ v: a.v + (p.market_value_usd ?? 0), b: a.b + (p.cost_basis_usd ?? 0) }), { v: 0, b: 0 });

  return (
    <div>
      <div className="page-head">
        <div><h1>Positions</h1><p>Open positions per account. Click one for its history.</p></div>
      </div>
      <div className="filters">
        <label>Broker
          <select className="select" value={broker ?? ""} onChange={(e) => set("broker", e.target.value)}>
            <option value="">All</option><option value="ibkr">Interactive Brokers</option><option value="kraken">Kraken</option>
          </select></label>
        <label>Account
          <select className="select" value={account ?? ""} onChange={(e) => set("account", e.target.value)}>
            <option value="">All</option>
            {(accounts.data ?? []).filter((a) => !broker || a.broker === broker).map((a) => <option key={a.id} value={a.external_id}>{a.external_id}{a.name ? ` · ${a.name}` : ""}</option>)}
          </select></label>
        <label>Ticker <input className="select" placeholder="filter…" value={q} onChange={(e) => set("q", e.target.value)} style={{ width: 120 }} /></label>
        <span className="muted" style={{ marginLeft: "auto", fontSize: 12.5 }}>{rows.length} positions · {money(totals.v)} · <PL v={totals.v - totals.b} fmt={signedMoney} /></span>
      </div>
      <ErrorBanner error={positions.error} />
      {positions.isPending ? <Loading text="Loading positions…" /> : (
        <Card className={positions.isFetching ? "chart-fade" : ""}>
          <div className="table-wrap"><table className="data num">
            <thead><tr>{COLS.map((c) => (
              <th key={c.key} className={`sortable${c.left ? " left" : ""}`} onClick={() => setSort((s) => ({ key: c.key, dir: s.key === c.key ? (s.dir === 1 ? -1 : 1) : (c.left ? 1 : -1) }))}>
                {c.label}{sort.key === c.key ? (sort.dir === 1 ? " ▲" : " ▼") : ""}
              </th>))}</tr></thead>
            <tbody>
              {rows.map((p) => (
                <tr key={p.id} className="clickable" onClick={() => nav(`/positions/${p.id}`)}>
                  <td className="ticker">{p.ticker}</td>
                  <td className="left"><BrokerPill broker={p.broker} /></td>
                  <td className="left mono">{p.account}</td>
                  <td className="left muted">{titleCase(p.asset_class)}</td>
                  <td>{num(p.quantity, Number.isInteger(p.quantity) ? 0 : 4)}</td>
                  <td>{p.market_price == null ? "—" : price(p.market_price)}</td>
                  <td>{p.avg_cost == null ? "—" : price(p.avg_cost)}</td>
                  <td>{p.market_value_usd == null ? "—" : money(p.market_value_usd)}</td>
                  <td>{p.cost_basis_usd == null ? "—" : money(p.cost_basis_usd)}</td>
                  <td><PL v={p.unrealized_pl_usd} fmt={signedMoney} /></td>
                  <td><PL v={p.unrealized_pl_pct} fmt={signedPct} /></td>
                  <td className="muted" title={p.updated_at}>{ago(p.updated_at)}</td>
                </tr>
              ))}
              {rows.length === 0 && <tr><td colSpan={COLS.length} className="empty">No positions match</td></tr>}
            </tbody>
          </table></div>
        </Card>
      )}
    </div>
  );
}
