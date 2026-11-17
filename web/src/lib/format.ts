// Number formatting, matching src/tui/format.rs.

const grouped = (x: number, decimals: number) =>
  Math.abs(x).toLocaleString("en-US", { minimumFractionDigits: decimals, maximumFractionDigits: decimals });

/** `1234.5` -> `1,234.50` */
export function num(x: number, decimals = 2): string {
  const s = grouped(x, decimals);
  const nonZero = /[1-9]/.test(s);
  return (x < 0 && nonZero ? "-" : "") + s;
}

/** Prices under 1 get four decimals so small crypto prices stay readable. */
export const price = (x: number) => num(x, Math.abs(x) >= 1 ? 2 : 4);

/** `-$1,234.56` */
export const money = (x: number) => `${x < 0 ? "-" : ""}$${grouped(x, 2)}`;

/** `+$1,234.56` / `-$1,234.56` */
export const signedMoney = (x: number) => (x >= 0 ? `+${money(x)}` : money(x));

/** Fraction as a signed percentage: `0.0523` -> `+5.23%` */
export const signedPct = (x: number) => `${x >= 0 ? "+" : ""}${(x * 100).toFixed(2)}%`;

/** Fraction as a percentage: `0.256` -> `25.6%` */
export const pct = (x: number, decimals = 1) => `${(x * 100).toFixed(decimals)}%`;

/** Compact money for axes: `$4.2K`, `$1.3M`. */
export function compactMoney(x: number): string {
  const a = Math.abs(x);
  const sign = x < 0 ? "-" : "";
  if (a >= 1e9) return `${sign}$${(a / 1e9).toFixed(1)}B`;
  if (a >= 1e6) return `${sign}$${(a / 1e6).toFixed(1)}M`;
  if (a >= 1e4) return `${sign}$${(a / 1e3).toFixed(0)}K`;
  if (a >= 1e3) return `${sign}$${(a / 1e3).toFixed(1)}K`;
  return `${sign}$${a.toFixed(0)}`;
}

/** Axis money that stays readable for sub-dollar values. */
export const axisMoney = (x: number, scaleMax: number) =>
  Math.abs(scaleMax) < 10 ? `${x < 0 ? "-" : ""}$${Math.abs(x).toFixed(Math.abs(scaleMax) < 1 ? 3 : 2)}` : compactMoney(x);

/** Axis price with enough decimals to tell neighbouring ticks apart. */
export function axisPrice(x: number, span: number): string {
  const d = span <= 0 ? 2 : Math.max(2, Math.min(6, Math.ceil(-Math.log10(span / 4)) + 1));
  return x.toLocaleString("en-US", { minimumFractionDigits: d, maximumFractionDigits: d });
}

export const shares = (x: number) => num(x, Number.isInteger(x) ? 0 : 4);

/** `12s ago`, `3m ago`, `2h ago`, `4d ago` */
export function ago(iso: string, now = Date.now()): string {
  const secs = Math.max(0, Math.floor((now - Date.parse(iso)) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ago`;
  return `${Math.floor(secs / 86400)}d ago`;
}

export const dateTime = (iso: string) =>
  new Date(iso).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

export const timeOnly = (iso: string) =>
  new Date(iso).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

export const shortDate = (iso: string | number) =>
  new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric" });

export const duration = (startIso: string, endIso: string | null) => {
  if (!endIso) return "…";
  const ms = Date.parse(endIso) - Date.parse(startIso);
  return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
};

export const titleCase = (s: string) => s.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
