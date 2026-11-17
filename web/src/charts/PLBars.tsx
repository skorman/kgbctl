import { BarChart, Bar, Cell, XAxis, YAxis, Tooltip, ResponsiveContainer, ReferenceLine, LabelList } from "recharts";

/** Outside the bar end for gains; losses go inside when the bar is wide enough, else outside-left. */
function PctLabel(props: { x?: number; y?: number; width?: number; height?: number; value?: unknown; ink: { text2: string; onFill: string } }) {
  const { x = 0, y = 0, width = 0, height = 0, value, ink } = props;
  const v = Number(value);
  const text = signedPct(v);
  const cy = y + height / 2 + 4;
  if (v >= 0) return <text x={x + width + 6} y={cy} fontSize={11} fill={ink.text2}>{text}</text>;
  const inside = Math.abs(width) >= 60;
  const left = Math.min(x, x + width);
  return inside
    ? <text x={left + 6} y={cy} fontSize={11} fontWeight={600} fill={ink.onFill}>{text}</text>
    : <text x={left - 6} y={cy} fontSize={11} fill={ink.text2} textAnchor="end">{text}</text>;
}
import { Tip } from "./Tip";
import { chartInk, axisTick } from "./theme";
import { money, signedMoney, signedPct, compactMoney } from "../lib/format";

export interface PLRow { name: string; pl: number; pl_pct: number; value: number }

/**
 * Horizontal bars of unrealized P/L per holding. Polarity -> status colors
 * (gain / loss), the only thing color encodes here. Sorted largest gain first.
 */
export function PLBars({ rows, max = 12 }: { rows: PLRow[]; max?: number }) {
  const ink = chartInk();
  const sorted = [...rows].sort((a, b) => b.pl - a.pl);
  const data = sorted.length > max
    ? [...sorted.slice(0, Math.ceil(max / 2)), ...sorted.slice(-Math.floor(max / 2))]
    : sorted;
  const h = Math.max(140, data.length * 26 + 30);
  return (
    <div className="chart" style={{ height: h }}>
      <ResponsiveContainer>
        <BarChart data={data} layout="vertical" margin={{ top: 4, right: 70, bottom: 4, left: 4 }} barSize={14}>
          <XAxis type="number" tickFormatter={compactMoney} tick={axisTick(ink)} axisLine={{ stroke: ink.axis }} tickLine={false} />
          <YAxis type="category" dataKey="name" width={58} tick={{ ...axisTick(ink), fill: ink.text2, fontWeight: 600 }} axisLine={false} tickLine={false} />
          <ReferenceLine x={0} stroke={ink.axis} />
          <Tooltip cursor={{ fill: ink.grid, opacity: 0.5 }} content={({ active, payload }) => {
            if (!active || !payload?.length) return null;
            const p = payload[0].payload as PLRow;
            return <Tip title={p.name} rows={[
              { k: "Unrealized P/L", v: signedMoney(p.pl) },
              { k: "Return", v: signedPct(p.pl_pct) },
              { k: "Market value", v: money(p.value) },
            ]} />;
          }} />
          <Bar dataKey="pl" radius={[0, 4, 4, 0]} isAnimationActive={false}>
            {data.map((d) => <Cell key={d.name} fill={d.pl >= 0 ? ink.good : ink.bad} />)}
            <LabelList dataKey="pl_pct" content={(p) => <PctLabel {...(p as object)} ink={ink} />} />
          </Bar>
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
