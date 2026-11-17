import { Link } from "react-router-dom";
import { useAccounts, usePositions } from "../api/hooks";
import { BrokerPill, Card, ErrorBanner, PL } from "../components/Common";
import { Loading } from "../components/Splash";
import { dateTime, money, signedMoney, signedPct } from "../lib/format";

export function Accounts() {
  const accounts = useAccounts();
  const positions = usePositions();
  if (accounts.isPending) return <Loading text="Loading accounts…" />;
  if (accounts.isError) return <ErrorBanner error={accounts.error} />;
  const totals = (acct: string) => {
    const ps = (positions.data ?? []).filter((p) => p.account === acct);
    const v = ps.reduce((a, p) => a + (p.market_value_usd ?? 0), 0);
    const b = ps.reduce((a, p) => a + (p.cost_basis_usd ?? 0), 0);
    return { n: ps.length, v, pl: v - b, pct: b ? (v - b) / b : 0 };
  };
  return (
    <div>
      <div className="page-head"><div><h1>Accounts</h1><p>Brokerage accounts the syncs have seen.</p></div></div>
      <Card>
        <div className="table-wrap"><table className="data num">
          <thead><tr><th>Account</th><th className="left">Broker</th><th className="left">Name</th><th className="left">Currency</th><th>Positions</th><th>Value</th><th>P/L $</th><th>P/L %</th><th className="left">First seen</th></tr></thead>
          <tbody>
            {accounts.data.map((a) => { const t = totals(a.external_id); return (
              <tr key={a.id}>
                <td className="mono"><Link to={`/positions?broker=${a.broker}&account=${encodeURIComponent(a.external_id)}`} style={{ textDecoration: "underline" }}>{a.external_id}</Link></td>
                <td className="left"><BrokerPill broker={a.broker} /></td>
                <td className="left muted">{a.name ?? "—"}</td>
                <td className="left">{a.base_currency}</td>
                <td>{t.n}</td>
                <td>{money(t.v)}</td>
                <td><PL v={t.pl} fmt={signedMoney} /></td>
                <td><PL v={t.pct} fmt={signedPct} /></td>
                <td className="left muted">{dateTime(a.created_at)}</td>
              </tr>); })}
            {accounts.data.length === 0 && <tr><td colSpan={9} className="empty">No accounts yet. Run a sync.</td></tr>}
          </tbody>
        </table></div>
      </Card>
    </div>
  );
}
