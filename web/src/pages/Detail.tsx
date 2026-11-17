import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useSnapshot } from "../api/hooks";
import { Card, ErrorBanner } from "../components/Common";
import { Loading } from "../components/Splash";
import { TotalsCards } from "../components/Stat";
import { HoldingsTable } from "../components/HoldingsTable";
import { WeightBars } from "../charts/WeightBars";
import { PLBars } from "../charts/PLBars";
import { tabColor } from "../charts/theme";
import { useKeys } from "../lib/keys";

/** The TUI's Detail view: one tab per ETF with cards and a holdings table. */
export function Detail() {
  const snap = useSnapshot();
  const { name } = useParams();
  const nav = useNavigate();
  const tabs = snap.data?.tabs ?? [];
  const index = Math.max(0, tabs.findIndex((t) => t.name.toLowerCase() === (name ?? "").toLowerCase()));
  const tab = tabs[index];
  const [selected, setSelected] = useState(0);

  useEffect(() => { setSelected(0); }, [index]);
  useEffect(() => {
    if (!name && tabs.length) nav(`/etf/${encodeURIComponent(tabs[0].name)}`, { replace: true });
  }, [name, tabs, nav]);

  const go = (step: number) => {
    if (!tabs.length) return;
    const next = (index + step + tabs.length) % tabs.length;
    nav(`/etf/${encodeURIComponent(tabs[next].name)}`);
  };
  useKeys((k, e) => {
    const n = tab?.holdings.length ?? 0;
    if (k === "ArrowLeft" || k === "h") go(-1);
    else if (k === "ArrowRight" || k === "l") go(1);
    else if ((k === "ArrowDown" || k === "j") && n) { e.preventDefault(); setSelected((s) => Math.min(n - 1, s + 1)); }
    else if ((k === "ArrowUp" || k === "k") && n) { e.preventDefault(); setSelected((s) => Math.max(0, s - 1)); }
    else if (k === "g" || k === "Home") setSelected(0);
    else if (k === "G" || k === "End") setSelected(Math.max(0, n - 1));
  }, [index, tabs.length, tab?.holdings.length]);

  const weightRows = useMemo(() => tab ? tab.holdings.map((h) => ({ name: h.ticker, value: h.value, weight: tab.totals.value ? h.value / tab.totals.value : 0 })) : [], [tab]);

  if (snap.isPending) return <Loading text="Loading holdings…" />;
  if (snap.isError) return <ErrorBanner error={snap.error} />;
  if (!tab) return <div className="empty">No tabs</div>;
  const color = tabColor(tab.name, index);

  return (
    <div className={snap.isFetching ? "chart-fade" : ""}>
      <div className="page-head">
        <div><h1>ETF detail</h1><p>Holdings per KGB ETF. ← → switch tabs, ↑ ↓ move rows.</p></div>
      </div>
      <div className="tabs" role="tablist">
        {tabs.map((t, i) => (
          <button key={t.name} role="tab" aria-selected={i === index} className={i === index ? "active" : ""}
            onClick={() => nav(`/etf/${encodeURIComponent(t.name)}`)}>
            <i className="swatch" style={{ background: tabColor(t.name, i) }} />{t.name}<span className="count">{t.holdings.length}</span>
          </button>
        ))}
      </div>
      <TotalsCards totals={tab.totals} valueLabel="Total Value" />
      {tab.holdings.length > 0 && (
        <div className="grid cols-2" style={{ marginTop: 14 }}>
          <Card title="Weight by holding" sub="market value, largest first">
            <WeightBars rows={weightRows} color={color} />
          </Card>
          <Card title="Unrealized P/L by holding">
            <PLBars rows={tab.holdings.map((h) => ({ name: h.ticker, pl: h.pl, pl_pct: h.pl_pct, value: h.value }))} max={14} />
          </Card>
        </div>
      )}
      <Card title={`${tab.name} holdings`} className="" sub={`${tab.holdings.length} tickers`}>
        <div style={{ marginTop: 4 }}>
          <HoldingsTable holdings={tab.holdings} total={tab.totals.value} selected={selected} onSelect={setSelected} />
        </div>
      </Card>
    </div>
  );
}
