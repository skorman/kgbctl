import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";
import type { Broker, PositionSnapshot } from "./types";

/** The TUI reloads every 30s; so do we. */
export const REFRESH_MS = 30_000;

const base = { refetchInterval: REFRESH_MS, staleTime: 10_000 };

export const useSnapshot = () => useQuery({ queryKey: ["snapshot"], queryFn: api.snapshot, ...base });
export const useHealth = () =>
  useQuery({ queryKey: ["health"], queryFn: api.health, refetchInterval: 15_000, retry: false });
export const usePositions = (f: { broker?: Broker; account?: string } = {}) =>
  useQuery({ queryKey: ["positions", f], queryFn: () => api.positions(f), ...base });
export const usePosition = (id: number) =>
  useQuery({ queryKey: ["position", id], queryFn: () => api.position(id), ...base });
export const usePositionHistory = (id: number) =>
  useQuery({ queryKey: ["history", id], queryFn: () => api.positionHistory(id), ...base });
export const useAccounts = () => useQuery({ queryKey: ["accounts"], queryFn: api.accounts, ...base });
export const useGroups = () => useQuery({ queryKey: ["groups"], queryFn: api.groups, ...base });
export const useGroup = (name: string) =>
  useQuery({ queryKey: ["group", name], queryFn: () => api.group(name), enabled: name !== "", ...base });
export const useOpenOrders = () => useQuery({ queryKey: ["orders"], queryFn: api.openOrders, ...base });
export const useSyncRuns = (f: { broker?: Broker; limit?: number } = {}) =>
  useQuery({ queryKey: ["syncRuns", f], queryFn: () => api.syncRuns(f), ...base });

/** Histories for many positions at once (for the portfolio-over-time chart). */
export function useHistories(ids: number[]) {
  return useQueries({
    queries: ids.map((id) => ({
      queryKey: ["history", id],
      queryFn: () => api.positionHistory(id),
      staleTime: 60_000,
      refetchInterval: REFRESH_MS * 2,
    })),
    combine: (results) => ({
      loading: results.some((r) => r.isPending),
      data: results.map((r, i) => [ids[i], r.data ?? []] as [number, PositionSnapshot[]]),
    }),
  });
}

/** `r` in the TUI: refetch everything now. */
export function useReload() {
  const qc = useQueryClient();
  return () => qc.invalidateQueries();
}
