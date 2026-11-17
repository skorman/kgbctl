import { useState } from "react";
import { Link } from "react-router-dom";
import { useGroup, useGroups, useSnapshot } from "../api/hooks";
import { Card, ErrorBanner, PL } from "../components/Common";
import { Loading } from "../components/Splash";
import { tabColor } from "../charts/theme";
import { money, signedMoney, signedPct, titleCase } from "../lib/format";

/** Position groups (the KGB ETF definitions) and their member instruments. */
export function Groups() {
  const groups = useGroups();
  const snap = useSnapshot();
  const [open, setOpen] = useState<string | null>(null);
  const detail = useGroup(open ?? "");
  const held = new Set(snap.data?.tabs.find((t) => t.name.toLowerCase() === open?.toLowerCase())?.holdings.map((h) => h.ticker) ?? []);

  if (groups.isPending) return <Loading text="Loading groups…" />;
  if (groups.isError) return <ErrorBanner error={groups.error} />;
  return (
    <div>
      <div className="page-head"><div><h1>Groups</h1><p>The KGB ETF definitions. Totals are null when nothing in the group is held.</p></div></div>
      <div className="grid cols-3">
        <Card title="Groups" className="">
          <div className="table-wrap"><table className="data num">
            <thead><tr><th>Group</th><th>Value</th><th>P/L $</th><th>P/L %</th></tr></thead>
            <tbody>
              {groups.data.map((g, i) => (
                <tr key={g.id} className={`clickable${open === g.name ? " selected" : ""}`} onClick={() => setOpen(g.name)}>
                  <td><span className="swatch" style={{ background: tabColor(g.name, i), marginRight: 8 }} />{g.name}{g.description && <div className="muted" style={{ fontSize: 11.5 }}>{g.description}</div>}</td>
                  <td>{g.totals?.value_usd == null ? <span className="muted">—</span> : money(g.totals.value_usd)}</td>
                  <td><PL v={g.totals?.pl_usd} fmt={signedMoney} /></td>
                  <td><PL v={g.totals?.pl_pct} fmt={signedPct} /></td>
                </tr>
              ))}
            </tbody>
          </table></div>
        </Card>
        <Card title={open ? `${open} members` : "Members"} className="span-2"
          right={open && <Link className="btn ghost sm" to={`/etf/${encodeURIComponent(open)}`}>Open holdings →</Link>}>
          {!open && <div className="empty">Select a group to see its members</div>}
          {open && detail.isPending && <Loading text="Loading members…" />}
          {open && detail.data && (
            <div className="table-wrap"><table className="data">
              <thead><tr><th>Symbol</th><th className="left">Name</th><th className="left">Class</th><th className="left">Currency</th><th className="left">Exchange</th><th className="left">Held</th></tr></thead>
              <tbody>
                {detail.data.members.map((m) => (
                  <tr key={m.id}>
                    <td className="ticker">{m.symbol}</td>
                    <td className="left muted">{m.name ?? "—"}</td>
                    <td className="left">{titleCase(m.asset_class)}</td>
                    <td className="left">{m.currency}</td>
                    <td className="left muted">{m.exchange ?? "—"}</td>
                    <td className="left">{held.has(m.symbol) ? <span className="pill good">● held</span> : <span className="pill neutral">not held</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table></div>
          )}
          <ErrorBanner error={detail.error} />
        </Card>
      </div>
    </div>
  );
}
