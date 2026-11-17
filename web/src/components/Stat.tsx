import type { ReactNode } from "react";
import { money, signedMoney, signedPct } from "../lib/format";
import type { Totals } from "../api/types";

export function Stat({ label, value, delta, hero, small, tone }: {
  label: ReactNode; value: ReactNode; delta?: ReactNode; hero?: boolean; small?: boolean; tone?: "good" | "bad";
}) {
  return (
    <div className={`stat${hero ? " hero" : ""}`}>
      <div className="label">{label}</div>
      <div className={`value${small ? " sm" : ""}${tone ? ` ${tone}` : ""}`}>{value}</div>
      {delta && <div className="delta">{delta}</div>}
    </div>
  );
}

export const plTone = (x: number) => (x >= 0 ? "good" : "bad");

/** The three cards the TUI draws: value, P/L $, P/L %. */
export function TotalsCards({ totals, valueLabel, hero, spinner }: { totals: Totals; valueLabel: string; hero?: boolean; spinner?: ReactNode }) {
  const tone = plTone(totals.pl);
  return (
    <div className="grid cols-3">
      <div className="card"><Stat hero={hero} label={<>{valueLabel}{spinner}</>} value={money(totals.value)} delta={<>Cost basis {money(totals.basis)}</>} /></div>
      <div className="card"><Stat hero={hero} label="Total P/L $" value={signedMoney(totals.pl)} tone={tone} delta="Unrealized, vs cost basis" /></div>
      <div className="card"><Stat hero={hero} label="Total P/L %" value={signedPct(totals.pl_pct)} tone={tone} delta="Return on cost basis" /></div>
    </div>
  );
}
