import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import { Tip, Legend } from "./Tip";
import { chartInk, axisTick } from "./theme";
import { dateTime, shortDate, compactMoney, money } from "../lib/format";

export interface Series { key: string; label: string; color: string }
export type StackRow = { t: number } & Record<string, number>;

/** Stacked value over time, one series per broker. */
export function StackedHistory({ rows, series, height = 240 }: { rows: StackRow[]; series: Series[]; height?: number }) {
  const ink = chartInk();
  if (rows.length === 0) return <div className="empty">No history yet. Run a sync to start recording snapshots.</div>;
  return (
    <div>
      <div className="chart" style={{ height }}>
        <ResponsiveContainer>
          <AreaChart data={rows} margin={{ top: 10, right: 12, bottom: 0, left: 0 }}>
            <CartesianGrid vertical={false} stroke={ink.grid} />
            <XAxis dataKey="t" type="number" domain={["dataMin", "dataMax"]} scale="time" tickFormatter={(t) => shortDate(t)}
              tick={axisTick(ink)} axisLine={{ stroke: ink.axis }} tickLine={false} minTickGap={40} />
            <YAxis tickFormatter={compactMoney} tick={axisTick(ink)} axisLine={false} tickLine={false} width={60} />
            <Tooltip cursor={{ stroke: ink.axis }} content={({ active, payload }) => {
              if (!active || !payload?.length) return null;
              const p = payload[0].payload as StackRow;
              const total = series.reduce((s, k) => s + (p[k.key] ?? 0), 0);
              return <Tip title={dateTime(new Date(p.t).toISOString())} rows={[
                ...series.map((s) => ({ k: s.label, v: money(p[s.key] ?? 0), color: s.color })),
                { k: <b>Total</b>, v: <b>{money(total)}</b> },
              ]} />;
            }} />
            {series.map((s) => (
              <Area key={s.key} type="monotone" dataKey={s.key} stackId="1" stroke={s.color} strokeWidth={2}
                fill={s.color} fillOpacity={0.12} isAnimationActive={false}
                dot={rows.length === 1 ? { r: 4, fill: s.color, stroke: ink.surface, strokeWidth: 2 } : false}
                activeDot={{ r: 5, fill: s.color, stroke: ink.surface, strokeWidth: 2 }} />
            ))}
          </AreaChart>
        </ResponsiveContainer>
      </div>
      <Legend items={series.map((s) => ({ label: s.label, color: s.color }))} />
    </div>
  );
}
