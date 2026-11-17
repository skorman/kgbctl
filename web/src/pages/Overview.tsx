import { useMemo } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useHistories, usePositions, useSnapshot } from "../api/hooks";
import { Card, ErrorBanner, SyncPill } from "../components/Common";
import { Loading } from "../components/Splash";
import { TotalsCards } from "../components/Stat";
import { AllocationDonut } from "../charts/AllocationDonut";
import { PLBars } from "../charts/PLBars";
import { StackedHistory, type StackRow } from "../charts/StackedHistory";
import { brokerColor, tabColor } from "../charts/theme";
import { money, signedMoney, signedPct, pct } from "../lib/format";
import type { Position, PositionSnapshot } from "../api/types";

/** Sum every position's market value at each snapshot time, per broker, forward-filling gaps. */
function portfolioSeries(positions: Position[], histories: [number, PositionSnapshot[]][]): StackRow[] {
  const brokerOf = new Map(positions.map((p) => [p.id, p.broker]));
  const times = new Set<number>();
  const perPos = histories.map(([id, snaps]) => {
    const pts = snaps.map((s) => ({ t: Date.parse(s.taken_at), v: s.market_value_usd ?? 0 })).sort((a, b) => a.t - b.t);
    pts.forEach((p) => times.add(p.t));
    return { broker: brokerOf.get(id) ?? "ibkr", pts };
  });
  // Snapshots from one sync land within seconds of each other; bucket to the minute.
  const buckets = [...times].map((t) => Math.floor(t / 60000) * 60000);
  const uniq = [...new Set(buckets)].sort((a, b) => a - b);
  return uniq.map((t) => {
    const row: StackRow = { t, ibkr: 0, kraken: 0 };
    for (const { broker, pts } of perPos) {
      let v = 0;
      for (const p of pts) { if (Math.floor(p.t / 60000) * 60000 <= t) v = p.v; else break; }
      row[broker] += v;
    }
    return row;
  });
}

export function Overview() {
  const nav = useNavigate();
  const snap = useSnapshot();
  const positions = usePositions();
  const ids = useMemo(() => (positions.data ?? []).map((p) => p.id), [positions.data]);
  const histories = useHistories(ids);
  const series = useMemo(
    () => (positions.data ? portfolioSeries(positions.data, histories.data) : []),
    [positions.data, histories.data],
  );

  if (snap.isPending) return <Loading text="Loading portfolio…" />;
  if (snap.isError) return <ErrorBanner error={snap.error} />;
  const s = snap.data;
  const tabsWithValue = s.tabs.filter((t) => t.holdings.length > 0);
  // Tabs overlap (a ticker can sit in two ETFs), so weights are vs the tab sum, not AUM.
  const tabSum = tabsWithValue.reduce((a, t) => a + t.totals.value, 0);
  // Same ticker can sit in two tabs; count it once.
  const seen = new Map<string, { name: string; pl: number; pl_pct: number; value: number }>();
  for (const h of s.tabs.flatMap((t) => t.holdings)) seen.set(h.ticker, { name: h.ticker, pl: h.pl, pl_pct: h.pl_pct, value: h.value });
  const allHoldings = [...seen.values()];

  return (
    <div className={snap.isFetching ? "chart-fade" : ""}>
      <div className="page-head">
        <div><h1>Overview</h1><p>Everything the TUI shows, plus where it's going.</p></div>
      </div>
      <TotalsCards totals={s.totals} valueLabel="Total AUM" hero />

      <div className="grid cols-3" style={{ marginTop: 14 }}>
        <Card title="By broker" className="span-2">
          <div className="table-wrap"><table className="data num">
            <thead><tr><th>Source</th><th>Value</th><th>P/L $</th><th>P/L %</th><th>Share</th><th className="left">Last sync</th></tr></thead>
            <tbody>
              {s.sources.map((src) => (
                <tr key={src.broker}>
                  <td><span className="swatch" style={{ background: brokerColor(src.broker), marginRight: 8 }} />{src.label}</td>
                  <td>{money(src.totals.value)}</td>
                  <td className={src.totals.pl >= 0 ? "good" : "bad"}>{signedMoney(src.totals.pl)}</td>
                  <td className={src.totals.pl >= 0 ? "good" : "bad"}>{signedPct(src.totals.pl_pct)}</td>
                  <td>{pct(s.totals.value ? src.totals.value / s.totals.value : 0)}</td>
                  <td className="left"><SyncPill run={src.last_run} />{src.last_run?.status === "error" && <div className="muted" style={{ fontSize: 11.5, whiteSpace: "normal", maxWidth: 360 }}>{src.last_run.error}</div>}</td>
                </tr>
              ))}
            </tbody>
          </table></div>
          <p className="muted" style={{ fontSize: 12.5, margin: "10px 0 0" }}>
            The server only reads the database. Run <code className="mono">kgbctl -c sync</code> to refresh broker data; this page reloads every 30s.
          </p>
        </Card>
        <Card title="Allocation by broker">
          <AllocationDonut total={s.totals.value} centerLabel="AUM"
            slices={s.sources.map((src) => ({ name: src.label, value: src.totals.value, color: brokerColor(src.broker) }))} />
        </Card>
      </div>

      <div className="grid cols-3" style={{ marginTop: 14 }}>
        <Card title="Portfolio value over time" sub={histories.loading ? "loading history…" : `${series.length} snapshot${series.length === 1 ? "" : "s"}`} className="span-2">
          <StackedHistory rows={series} series={[
            { key: "ibkr", label: "Interactive Brokers", color: brokerColor("ibkr") },
            { key: "kraken", label: "Kraken", color: brokerColor("kraken") },
          ]} />
        </Card>
        <Card title="Allocation by ETF" sub="of tab total; tickers can overlap">
          <AllocationDonut total={tabSum} centerLabel="across tabs"
            slices={tabsWithValue.map((t, i) => ({ name: t.name, value: t.totals.value, color: tabColor(t.name, i) }))} />
        </Card>
      </div>

      <div className="grid cols-2" style={{ marginTop: 14 }}>
        <Card title="ETF performance" sub="unrealized P/L per tab">
          <div className="table-wrap"><table className="data num">
            <thead><tr><th>Tab</th><th>Holdings</th><th>Value</th><th>P/L $</th><th>P/L %</th></tr></thead>
            <tbody>
              {s.tabs.map((t, i) => (
                <tr key={t.name} className="clickable" onClick={() => nav(`/etf/${encodeURIComponent(t.name)}`)}>
                  <td><Link to={`/etf/${encodeURIComponent(t.name)}`}><span className="swatch" style={{ background: tabColor(t.name, i), marginRight: 8 }} />{t.name}</Link></td>
                  <td>{t.holdings.length}</td>
                  <td>{money(t.totals.value)}</td>
                  <td className={t.totals.pl >= 0 ? "good" : "bad"}>{signedMoney(t.totals.pl)}</td>
                  <td className={t.totals.pl >= 0 ? "good" : "bad"}>{signedPct(t.totals.pl_pct)}</td>
                </tr>
              ))}
            </tbody>
          </table></div>
        </Card>
        <Card title="Biggest winners & losers" sub="unrealized P/L, all tickers">
          <PLBars rows={allHoldings} max={12} />
        </Card>
      </div>
    </div>
  );
}
