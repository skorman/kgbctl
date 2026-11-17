import { PieChart, Pie, Cell, Tooltip, ResponsiveContainer } from "recharts";
import { Legend, Tip } from "./Tip";
import { chartInk } from "./theme";
import { money, pct } from "../lib/format";

export interface Slice { name: string; value: number; color: string }

/** Part-to-whole at a glance, <= 6 slices; the table beside it carries exact values. */
export function AllocationDonut({ slices, total, centerLabel }: { slices: Slice[]; total: number; centerLabel: string }) {
  const ink = chartInk();
  const data = slices.filter((s) => s.value > 0);
  return (
    <div>
      <div className="chart" style={{ height: 220 }}>
        <ResponsiveContainer>
          <PieChart>
            <Pie data={data} dataKey="value" nameKey="name" innerRadius="62%" outerRadius="92%"
              paddingAngle={2} stroke={ink.surface} strokeWidth={2} isAnimationActive={false}>
              {data.map((s) => <Cell key={s.name} fill={s.color} />)}
            </Pie>
            <Tooltip content={({ active, payload }) => {
              if (!active || !payload?.length) return null;
              const p = payload[0].payload as Slice;
              return <Tip title={p.name} rows={[
                { k: "Value", v: money(p.value) },
                { k: "Weight", v: pct(total ? p.value / total : 0) },
              ]} />;
            }} />
            <text x="50%" y="47%" textAnchor="middle" fill={ink.text} fontSize={18} fontWeight={650}>{money(total)}</text>
            <text x="50%" y="58%" textAnchor="middle" fill={ink.muted} fontSize={11.5}>{centerLabel}</text>
          </PieChart>
        </ResponsiveContainer>
      </div>
      <Legend items={data.map((d) => ({ label: `${d.name} ${pct(total ? d.value / total : 0)}`, color: d.color }))} />
    </div>
  );
}
