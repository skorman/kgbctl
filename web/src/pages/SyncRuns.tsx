import { useSearchParams } from "react-router-dom";
import { useSyncRuns } from "../api/hooks";
import type { Broker } from "../api/types";
import { BrokerPill, Card, ErrorBanner } from "../components/Common";
import { Loading } from "../components/Splash";
import { Stat } from "../components/Stat";
import { ago, dateTime, duration, titleCase } from "../lib/format";

export function SyncRuns() {
  const [params, setParams] = useSearchParams();
  const broker = (params.get("broker") as Broker | null) ?? undefined;
  const limit = Number(params.get("limit") ?? 50);
  const runs = useSyncRuns({ broker, limit });
  const set = (k: string, v: string) => { const p = new URLSearchParams(params); v ? p.set(k, v) : p.delete(k); setParams(p, { replace: true }); };
  const data = runs.data ?? [];
  const ok = data.filter((r) => r.status === "ok").length;
  const failed = data.filter((r) => r.status === "error").length;
  const lastOk = (b: Broker) => data.find((r) => r.broker === b && r.status === "ok");

  return (
    <div>
      <div className="page-head"><div><h1>Sync runs</h1><p>Every broker sync attempt, newest first. Syncs run where the broker credentials live and push their data here with <code className="mono">kgbctl -c sync --remote</code>.</p></div></div>
      <div className="grid cols-4" style={{ marginBottom: 14 }}>
        <div className="card"><Stat small label="Runs shown" value={data.length} delta={`${ok} ok · ${failed} failed`} /></div>
        <div className="card"><Stat small label="Success rate" value={data.length ? `${Math.round((ok / data.length) * 100)}%` : "—"} tone={failed > ok ? "bad" : "good"} /></div>
        <div className="card"><Stat small label="Last IBKR success" value={lastOk("ibkr") ? ago(lastOk("ibkr")!.finished_at ?? lastOk("ibkr")!.started_at) : "never"} /></div>
        <div className="card"><Stat small label="Last Kraken success" value={lastOk("kraken") ? ago(lastOk("kraken")!.finished_at ?? lastOk("kraken")!.started_at) : "never"} /></div>
      </div>
      <div className="filters">
        <label>Broker
          <select className="select" value={broker ?? ""} onChange={(e) => set("broker", e.target.value)}>
            <option value="">All</option><option value="ibkr">Interactive Brokers</option><option value="kraken">Kraken</option>
          </select></label>
        <label>Limit
          <select className="select" value={limit} onChange={(e) => set("limit", e.target.value)}>
            {[20, 50, 100, 250, 500].map((n) => <option key={n} value={n}>{n}</option>)}
          </select></label>
      </div>
      <ErrorBanner error={runs.error} />
      {runs.isPending ? <Loading text="Loading runs…" /> : (
        <Card className={runs.isFetching ? "chart-fade" : ""}>
          <div className="table-wrap"><table className="data">
            <thead><tr><th>#</th><th className="left">Broker</th><th className="left">Status</th><th className="left">Started</th><th>Duration</th><th className="left">Pushed by</th><th className="left">Error</th></tr></thead>
            <tbody>
              {data.map((r) => (
                <tr key={r.id}>
                  <td className="mono muted">{r.id}</td>
                  <td className="left"><BrokerPill broker={r.broker} /></td>
                  <td className="left"><span className={`pill ${r.status === "ok" ? "good" : r.status === "error" ? "bad" : "warn"}`}>{r.status === "ok" ? "✓" : r.status === "error" ? "✕" : "◌"} {titleCase(r.status)}</span></td>
                  <td className="left" title={r.started_at}>{dateTime(r.started_at)} <span className="muted">({ago(r.started_at)})</span></td>
                  <td className="num">{duration(r.started_at, r.finished_at)}</td>
                  <td className="left">{r.pushed_by ?? <span className="muted">server</span>}</td>
                  <td className="left" style={{ whiteSpace: "normal", maxWidth: 520 }}>{r.error ? <span className="bad">{r.error}</span> : <span className="muted">—</span>}</td>
                </tr>
              ))}
              {data.length === 0 && <tr><td colSpan={7} className="empty">No sync runs yet</td></tr>}
            </tbody>
          </table></div>
        </Card>
      )}
    </div>
  );
}
