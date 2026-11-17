import { useCallback, useEffect, useState } from "react";

export type Theme = "dark" | "light" | "system";
const KEY = "kgbctl.theme";

function apply(theme: Theme) {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

export function useTheme() {
  const [theme, set] = useState<Theme>(() => {
    try { return (localStorage.getItem(KEY) as Theme) || "dark"; } catch { return "dark"; }
  });
  useEffect(() => { apply(theme); }, [theme]);
  const setTheme = useCallback((t: Theme) => {
    try { localStorage.setItem(KEY, t); } catch { /* ignore */ }
    set(t);
  }, []);
  const cycle = useCallback(() => {
    setTheme(theme === "dark" ? "light" : theme === "light" ? "system" : "dark");
  }, [theme, setTheme]);
  return { theme, setTheme, cycle };
}

/** Whether the page currently renders dark (for chart ink/surfaces). */
export function useIsDark(): boolean {
  const get = () => {
    const t = document.documentElement.getAttribute("data-theme");
    if (t === "dark") return true;
    if (t === "light") return false;
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  };
  const [dark, setDark] = useState(get);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const update = () => setDark(get());
    mq.addEventListener("change", update);
    const obs = new MutationObserver(update);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => { mq.removeEventListener("change", update); obs.disconnect(); };
  }, []);
  return dark;
}
