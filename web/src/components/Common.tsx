import type { ReactNode } from "react";
import { ApiError } from "../api/client";
import type { SyncRun, Broker } from "../api/types";
import { ago } from "../lib/format";

export function ErrorBanner({ error }: { error: unknown }) {
  if (!error) return null;
  const msg = error instanceof ApiError ? `${error.message} (${error.code})` : String((error as Error).message ?? error);
  return <div className="banner bad" role="alert">⚠ {msg}</div>;
}

export function Card({ title, sub, children, className, right }: { title?: ReactNode; sub?: ReactNode; children: ReactNode; className?: string; right?: ReactNode }) {
  return (
    <section className={`card${className ? ` ${className}` : ""}`}>
      {(title || right) && <div className="card-head"><h2>{title}</h2>{right ?? (sub && <span className="sub">{sub}</span>)}</div>}
      {children}
    </section>
  );
}

export function SyncPill({ run }: { run: SyncRun | null }) {
  if (!run) return <span className="pill bad">● unavailable</span>;
  const when = run.finished_at ?? run.started_at;
  if (run.status === "ok") return <span className="pill good" title={when}>✓ synced {ago(when)}</span>;
  if (run.status === "running") return <span className="pill warn">◌ sync running since {ago(run.started_at)}</span>;
  return <span className="pill bad" title={run.error ?? ""}>✕ last sync failed {ago(when)}</span>;
}

export function BrokerPill({ broker }: { broker: Broker }) {
  return <span className="pill outline"><i className="swatch" style={{ background: `var(--s-${broker})` }} />{broker === "ibkr" ? "IBKR" : "Kraken"}</span>;
}

export function PL({ v, fmt }: { v: number | null | undefined; fmt: (x: number) => string }) {
  if (v === null || v === undefined) return <span className="muted">—</span>;
  return <span className={v >= 0 ? "good" : "bad"}>{fmt(v)}</span>;
}
