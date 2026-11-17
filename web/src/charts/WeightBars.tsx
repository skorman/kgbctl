import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from "recharts";
import { Tip } from "./Tip";
import { chartInk, axisTick } from "./theme";
import { money, pct, compactMoney } from "../lib/format";

export interface WeightRow { name: string; value: number; weight: number }

/** Market value per holding in one series color (identity carried by the label). */
export function WeightBars({ rows, color, max = 15 }: { rows: WeightRow[]; color: string; max?: number }) {
  const ink = chartInk();
  const data = rows.slice(0, max);
  return (
    <div className="chart" style={{ height: 240 }}>
      <ResponsiveContainer>
        <BarChart data={data} margin={{ top: 8, right: 8, bottom: 0, left: 0 }} barSize={22}>
          <CartesianGrid vertical={false} stroke={ink.grid} />
          <XAxis dataKey="name" tick={{ ...axisTick(ink), fill: ink.text2, fontWeight: 600 }} axisLine={{ stroke: ink.axis }} tickLine={false} interval={0} angle={data.length > 10 ? -40 : 0} textAnchor={data.length > 10 ? "end" : "middle"} height={data.length > 10 ? 48 : 24} />
          <YAxis tickFormatter={compactMoney} tick={axisTick(ink)} axisLine={false} tickLine={false} width={52} />
          <Tooltip cursor={{ fill: ink.grid, opacity: 0.5 }} content={({ active, payload }) => {
            if (!active || !payload?.length) return null;
            const p = payload[0].payload as WeightRow;
            return <Tip title={p.name} rows={[{ k: "Market value", v: money(p.value) }, { k: "Weight", v: pct(p.weight) }]} />;
          }} />
          <Bar dataKey="value" fill={color} radius={[4, 4, 0, 0]} isAnimationActive={false} />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
