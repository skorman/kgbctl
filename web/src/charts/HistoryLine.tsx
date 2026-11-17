import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid, ReferenceLine } from "recharts";
import { Tip } from "./Tip";
import { chartInk, axisTick } from "./theme";
import { dateTime, shortDate, axisMoney, axisPrice, money, price as fmtPrice } from "../lib/format";

export interface Point { t: number; v: number }

/** One series over time; a 2px line with a 10% wash. */
export function HistoryLine({ points, color, label, kind = "money", reference, height = 220 }:
  { points: Point[]; color: string; label: string; kind?: "money" | "price"; reference?: { v: number; label: string }; height?: number }) {
  const ink = chartInk();
  const fmt = kind === "money" ? money : (x: number) => fmtPrice(x);
  if (points.length === 0) return <div className="empty">No history yet</div>;
  const single = points.length === 1;
  const vals = points.map((p) => p.v).concat(reference ? [reference.v] : []);
  const lo = Math.min(...vals), hi = Math.max(...vals);
  const scaleMax = Math.max(Math.abs(lo), Math.abs(hi));
  const axisFmt = kind === "money" ? (x: number) => axisMoney(x, scaleMax) : (x: number) => axisPrice(x, hi - lo);
  const pad = (hi - lo || Math.abs(hi) || 1) * 0.12;
  return (
    <div className="chart" style={{ height }}>
      <ResponsiveContainer>
        <AreaChart data={points} margin={{ top: 10, right: 12, bottom: 0, left: 0 }}>
          <defs>
            <linearGradient id={`wash-${label.replace(/\W/g, "")}`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity={0.18} />
              <stop offset="100%" stopColor={color} stopOpacity={0.02} />
            </linearGradient>
          </defs>
          <CartesianGrid vertical={false} stroke={ink.grid} />
          <XAxis dataKey="t" type="number" domain={["dataMin", "dataMax"]} scale="time" tickFormatter={(t) => shortDate(t)}
            tick={axisTick(ink)} axisLine={{ stroke: ink.axis }} tickLine={false} minTickGap={40} />
          <YAxis domain={[lo - pad, hi + pad]} tickFormatter={axisFmt} tick={axisTick(ink)} axisLine={false} tickLine={false} width={60} />
          {reference && <ReferenceLine y={reference.v} stroke={ink.muted} strokeDasharray="3 3" label={{ value: reference.label, fill: ink.muted, fontSize: 11, position: "insideTopRight" }} />}
          <Tooltip cursor={{ stroke: ink.axis }} content={({ active, payload }) => {
            if (!active || !payload?.length) return null;
            const p = payload[0].payload as Point;
            return <Tip title={dateTime(new Date(p.t).toISOString())} rows={[{ k: label, v: fmt(p.v), color }]} />;
          }} />
          <Area type="monotone" dataKey="v" stroke={color} strokeWidth={2} fill={`url(#wash-${label.replace(/\W/g, "")})`}
            dot={single ? { r: 4, fill: color, stroke: ink.surface, strokeWidth: 2 } : false}
            activeDot={{ r: 5, fill: color, stroke: ink.surface, strokeWidth: 2 }} isAnimationActive={false} />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
