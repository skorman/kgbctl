import type { Broker } from "../api/types";

/** Read a CSS token at render time so charts follow the theme toggle. */
export function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Tab (ETF) colors follow the entity, never its rank: fixed by name. */
const TAB_SLOT: Record<string, string> = {
  zeus: "--s1", ares: "--s2", prometheus: "--s3", gaia: "--s4", other: "--s5", crypto: "--s6",
};
const FALLBACK = ["--s1", "--s2", "--s3", "--s4", "--s5", "--s6"];

export function tabColor(name: string, index = 0): string {
  const slot = TAB_SLOT[name.toLowerCase()] ?? FALLBACK[index % FALLBACK.length];
  return cssVar(slot);
}

export const brokerColor = (b: Broker) => cssVar(b === "ibkr" ? "--s-ibkr" : "--s-kraken");

export function chartInk() {
  return {
    grid: cssVar("--grid"), axis: cssVar("--axis"), muted: cssVar("--muted"),
    text: cssVar("--text"), text2: cssVar("--text-2"), surface: cssVar("--surface"),
    good: cssVar("--good"), bad: cssVar("--bad"), onFill: cssVar("--on-fill") || "#ffffff", accent: cssVar("--brand-sage") || cssVar("--accent"),
  };
}

export const axisTick = (ink: ReturnType<typeof chartInk>) => ({ fill: ink.muted, fontSize: 11.5 });
