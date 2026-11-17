import { Link, useParams } from "react-router-dom";
import { usePosition, usePositionHistory } from "../api/hooks";
import { BrokerPill, Card, ErrorBanner, PL } from "../components/Common";
import { Loading } from "../components/Splash";
import { Stat, plTone } from "../components/Stat";
import { HistoryLine } from "../charts/HistoryLine";
import { brokerColor, chartInk } from "../charts/theme";
import { money, num, price, signedMoney, signedPct, dateTime, titleCase } from "../lib/format";

export function PositionDetail() {
  const id = Number(useParams().id);
  const pos = usePosition(id);
  const hist = usePositionHistory(id);
  if (pos.isPending) return <Loading text="Loading position…" />;
  if (pos.isError) return <ErrorBanner error={pos.error} />;
  const p = pos.data;
  const snaps = (hist.data ?? []).slice().sort((a, b) => Date.parse(a.taken_at) - Date.parse(b.taken_at));
  const pts = (k: "market_value_usd" | "market_price" | "unrealized_pl_usd") =>
    snaps.filter((s) => s[k] != null).map((s) => ({ t: Date.parse(s.taken_at), v: s[k] as number }));
  const color = brokerColor(p.broker);
  const ink = chartInk();
  const pl = p.unrealized_pl_usd ?? 0;

  return (
    <div>
      <div className="page-head">
        <div>
          <h1><span className="ticker">{p.ticker}</span> <span className="muted" style={{ fontWeight: 400, fontSize: 15 }}>· {titleCase(p.asset_class)} · <BrokerPill broker={p.broker} /> <span className="mono">{p.account}</span></span></h1>
          <p>Updated {dateTime(p.updated_at)} · <Link to="/positions" style={{ textDecoration: "underline" }}>all positions</Link></p>
        </div>
      </div>
      <div className="grid cols-4">
        <div className="card"><Stat label="Market value" value={p.market_value_usd == null ? "—" : money(p.market_value_usd)} delta={<>{num(p.quantity, Number.isInteger(p.quantity) ? 0 : 4)} @ {p.market_price == null ? "—" : price(p.market_price)}</>} /></div>
        <div className="card"><Stat label="Cost basis" value={p.cost_basis_usd == null ? "—" : money(p.cost_basis_usd)} delta={<>avg cost {p.avg_cost == null ? "—" : price(p.avg_cost)}</>} /></div>
        <div className="card"><Stat label="Unrealized P/L" value={p.unrealized_pl_usd == null ? "—" : signedMoney(p.unrealized_pl_usd)} tone={plTone(pl)} /></div>
        <div className="card"><Stat label="Return" value={p.unrealized_pl_pct == null ? "—" : signedPct(p.unrealized_pl_pct)} tone={plTone(p.unrealized_pl_pct ?? 0)} /></div>
      </div>
      <ErrorBanner error={hist.error} />
      <div className="grid cols-2" style={{ marginTop: 14 }}>
        <Card title="Market value" sub={`${snaps.length} snapshot${snaps.length === 1 ? "" : "s"}`}>
          <HistoryLine points={pts("market_value_usd")} color={color} label="Market value"
            reference={p.cost_basis_usd != null ? { v: p.cost_basis_usd, label: "cost basis" } : undefined} />
        </Card>
        <Card title="Price" sub={p.avg_cost ? `avg cost ${price(p.avg_cost)}` : undefined}>
          <HistoryLine points={pts("market_price")} color={color} label="Price" kind="price"
            reference={p.avg_cost ? { v: p.avg_cost, label: "avg cost" } : undefined} />
        </Card>
      </div>
      <div className="grid cols-2" style={{ marginTop: 14 }}>
        <Card title="Unrealized P/L">
          <HistoryLine points={pts("unrealized_pl_usd")} color={pl >= 0 ? ink.good : ink.bad} label="Unrealized P/L" reference={{ v: 0, label: "" }} />
        </Card>
        <Card title="Snapshots" sub="one per successful sync">
          <div className="table-wrap" style={{ maxHeight: 260, overflowY: "auto" }}><table className="data num">
            <thead><tr><th>Taken</th><th>Qty</th><th>Price</th><th>Value</th><th>P/L $</th><th>P/L %</th></tr></thead>
            <tbody>
              {snaps.slice().reverse().map((s) => (
                <tr key={s.taken_at}>
                  <td className="muted">{dateTime(s.taken_at)}</td>
                  <td>{num(s.quantity, Number.isInteger(s.quantity) ? 0 : 4)}</td>
                  <td>{s.market_price == null ? "—" : price(s.market_price)}</td>
                  <td>{s.market_value_usd == null ? "—" : money(s.market_value_usd)}</td>
                  <td><PL v={s.unrealized_pl_usd} fmt={signedMoney} /></td>
                  <td><PL v={s.unrealized_pl_pct} fmt={signedPct} /></td>
                </tr>
              ))}
              {snaps.length === 0 && <tr><td colSpan={6} className="empty">No snapshots yet</td></tr>}
            </tbody>
          </table></div>
        </Card>
      </div>
    </div>
  );
}
