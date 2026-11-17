import { useMemo, useState } from "react";
import type { Holding } from "../api/types";
import { num, price, pct, signedPct, money, signedMoney } from "../lib/format";

type Key = keyof Holding | "weight";
const COLS: { key: Key; label: string }[] = [
  { key: "ticker", label: "Ticker" }, { key: "value", label: "Value ($)" }, { key: "shares", label: "Shares" },
  { key: "cost", label: "Cost" }, { key: "price", label: "Price" }, { key: "weight", label: "Weight" },
  { key: "pl", label: "P/L $" }, { key: "pl_pct", label: "% P/L" },
];

export function HoldingsTable({ holdings, total, selected, onSelect }:
  { holdings: Holding[]; total: number; selected?: number; onSelect?: (i: number) => void }) {
  const [sort, setSort] = useState<{ key: Key; dir: 1 | -1 }>({ key: "value", dir: -1 });
  const denom = total || 1;
  const rows = useMemo(() => {
    const withW = holdings.map((h, i) => ({ h, i, weight: h.value / denom }));
    const v = (r: typeof withW[number]) => (sort.key === "weight" ? r.weight : r.h[sort.key]);
    return withW.sort((a, b) => {
      const x = v(a), y = v(b);
      const c = typeof x === "string" && typeof y === "string" ? x.localeCompare(y) : Number(x) - Number(y);
      return c * sort.dir;
    });
  }, [holdings, sort, denom]);
  const toggle = (key: Key) => setSort((s) => (s.key === key ? { key, dir: s.dir === 1 ? -1 : 1 } : { key, dir: key === "ticker" ? 1 : -1 }));
  const sum = holdings.reduce((a, h) => ({ value: a.value + h.value, basis: a.basis + h.basis }), { value: 0, basis: 0 });
  const plPct = sum.basis ? (sum.value - sum.basis) / sum.basis : 0;

  if (holdings.length === 0) return <div className="empty">No open positions in this tab</div>;
  return (
    <div className="table-wrap">
      <table className="data num">
        <thead><tr>{COLS.map((c) => (
          <th key={c.key} className="sortable" onClick={() => toggle(c.key)} aria-sort={sort.key === c.key ? (sort.dir === 1 ? "ascending" : "descending") : "none"}>
            {c.label}{sort.key === c.key ? (sort.dir === 1 ? " ▲" : " ▼") : ""}
          </th>))}</tr></thead>
        <tbody>
          {rows.map(({ h, i, weight }) => (
            <tr key={h.ticker} className={`${onSelect ? "clickable " : ""}${selected === i ? "selected" : ""}`} onClick={() => onSelect?.(i)}>
              <td className="ticker">{h.ticker}</td>
              <td>{num(h.value)}</td>
              <td>{num(h.shares, 4)}</td>
              <td>{price(h.cost)}</td>
              <td>{price(h.price)}</td>
              <td><span className="weightbar"><i style={{ width: `${Math.min(100, weight * 100)}%` }} /></span>{pct(weight)}</td>
              <td className={h.pl >= 0 ? "good" : "bad"}>{signedMoney(h.pl)}</td>
              <td className={h.pl_pct >= 0 ? "good" : "bad"}>{signedPct(h.pl_pct)}</td>
            </tr>
          ))}
        </tbody>
        <tfoot><tr>
          <td>Total</td><td>{num(sum.value)}</td><td /><td /><td /><td>100%</td>
          <td className={sum.value - sum.basis >= 0 ? "good" : "bad"}>{signedMoney(sum.value - sum.basis)}</td>
          <td className={plPct >= 0 ? "good" : "bad"}>{signedPct(plPct)}</td>
        </tr></tfoot>
      </table>
      <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>Cost basis {money(sum.basis)} · click a header to sort</div>
    </div>
  );
}
