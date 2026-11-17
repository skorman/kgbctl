import type { ReactNode } from "react";

export function Tip({ title, rows }: { title?: ReactNode; rows: { k: ReactNode; v: ReactNode; color?: string }[] }) {
  return (
    <div className="chart-tip">
      {title && <div className="t">{title}</div>}
      {rows.map((r, i) => (
        <div className="row" key={i}>
          <span className="k">{r.color && <i className="swatch" style={{ background: r.color }} />}{r.k}</span>
          <span className="num">{r.v}</span>
        </div>
      ))}
    </div>
  );
}

export function Legend({ items }: { items: { label: string; color: string }[] }) {
  return (
    <div className="legend" role="list">
      {items.map((i) => (
        <span key={i.label} role="listitem"><i className="swatch" style={{ background: i.color }} />{i.label}</span>
      ))}
    </div>
  );
}
