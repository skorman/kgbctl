import type {
  Account, Broker, Group, GroupDetail, Health, Order, Position, PositionSnapshot,
  RotatedToken, Snapshot, SyncRun, Tab, TabSummary,
} from "./types";

// Same origin by default: Vite proxies /api to the Rust server in dev, and a
// reverse proxy (or the tunnel) does the same in production. Override with
// VITE_API_BASE to talk to another host directly.
export const API_BASE = (import.meta.env.VITE_API_BASE as string | undefined)?.replace(/\/$/, "") ?? "";
const V1 = `${API_BASE}/api/v1`;

export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string) {
    super(message);
  }
  get unauthorized() { return this.status === 401; }
}

let tokenProvider: () => string | null = () => { try { return localStorage.getItem("kgbctl.token"); } catch { return null; } };
export function setTokenProvider(fn: () => string | null) { tokenProvider = fn; }

async function request<T>(path: string, init: RequestInit = {}, token?: string | null): Promise<T> {
  const t = token === undefined ? tokenProvider() : token;
  const headers = new Headers(init.headers);
  headers.set("Accept", "application/json");
  if (t) headers.set("Authorization", `Bearer ${t}`);
  let res: Response;
  try {
    res = await fetch(`${V1}${path}`, { ...init, headers });
  } catch (e) {
    throw new ApiError(0, "network", `Can't reach the server (${(e as Error).message})`);
  }
  if (!res.ok) {
    let code = "http_error";
    let message = `${res.status} ${res.statusText}`;
    try {
      const body = await res.json();
      if (body?.error) { code = body.error.code ?? code; message = body.error.message ?? message; }
    } catch { /* non-JSON error body */ }
    throw new ApiError(res.status, code, message);
  }
  return res.json() as Promise<T>;
}

const qs = (params: Record<string, string | number | undefined>) => {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) if (v !== undefined && v !== "") p.set(k, String(v));
  const s = p.toString();
  return s ? `?${s}` : "";
};

export const api = {
  health: () => request<Health>("/health", {}, null),
  snapshot: () => request<Snapshot>("/snapshot"),
  tabs: () => request<TabSummary[]>("/tabs"),
  tab: (name: string) => request<Tab>(`/tabs/${encodeURIComponent(name)}`),
  positions: (f: { broker?: Broker; account?: string } = {}) => request<Position[]>(`/positions${qs(f)}`),
  position: (id: number) => request<Position>(`/positions/${id}`),
  positionHistory: (id: number, range: { from?: string; to?: string } = {}) =>
    request<PositionSnapshot[]>(`/positions/${id}/history${qs(range)}`),
  accounts: () => request<Account[]>("/accounts"),
  groups: () => request<Group[]>("/groups"),
  group: (name: string) => request<GroupDetail>(`/groups/${encodeURIComponent(name)}`),
  openOrders: () => request<Order[]>("/orders/open"),
  syncRuns: (f: { broker?: Broker; limit?: number } = {}) => request<SyncRun[]>(`/sync/runs${qs(f)}`),
  rotateToken: () => request<RotatedToken>("/token/rotate", { method: "POST" }),
  /** Verify a token before storing it. */
  verify: (token: string) => request<TabSummary[]>("/tabs", {}, token),
};
