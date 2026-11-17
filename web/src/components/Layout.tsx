import { useEffect, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useIsFetching } from "@tanstack/react-query";
import { useHealth, useReload, useSnapshot } from "../api/hooks";
import { useAuth } from "../lib/auth";
import { useTheme } from "../lib/theme";
import { useKeys } from "../lib/keys";
import { ApiError } from "../api/client";

const NAV = [
  ["/", "Overview"], ["/etf", "ETF detail"], ["/positions", "Positions"], ["/groups", "Groups"],
  ["/orders", "Orders"], ["/sync", "Sync runs"], ["/accounts", "Accounts"], ["/settings", "Settings"],
] as const;

const KEYS: [string, string][] = [["r", "Reload"], ["o", "Overview / Detail"], ["←/→", "Prev / Next ETF"], ["↑/↓", "Move row"], ["t", "Theme"], ["?", "Help"]];

function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => { const id = setInterval(() => setNow(new Date()), 1000); return () => clearInterval(id); }, []);
  return <span className="clock">{now.toLocaleTimeString(undefined, { hour12: false })}</span>;
}

export function Layout() {
  const health = useHealth();
  const snap = useSnapshot();
  const reload = useReload();
  const fetching = useIsFetching();
  const { userName, signOut } = useAuth();
  const { theme, cycle } = useTheme();
  const nav = useNavigate();
  const loc = useLocation();

  useKeys((k) => {
    if (k === "r") reload();
    else if (k === "o") nav(loc.pathname.startsWith("/etf") ? "/" : "/etf");
    else if (k === "t") cycle();
    else if (k === "?") nav("/settings");
  }, [loc.pathname, cycle]);

  // A 401 anywhere means the token was rotated or removed: back to sign-in.
  useEffect(() => {
    if (snap.error instanceof ApiError && snap.error.unauthorized) signOut();
  }, [snap.error, signOut]);

  const h = health.data;
  const healthClass = health.isError ? "bad" : h?.status === "ok" ? "ok" : h ? "warn" : "";
  const themeIcon = theme === "dark" ? "☾" : theme === "light" ? "☀" : "◐";

  return (
    <div className="shell">
      <header className="topbar">
        <div className="topbar-inner">
          <NavLink to="/" className="brand" aria-label="KGB Capital Group"><span className="brand-kgb">KGB</span><span className="brand-sub">Capital Group</span></NavLink>
          <nav className="nav">
            {NAV.map(([to, label]) => <NavLink key={to} to={to} end={to === "/"} className={({ isActive }) => (isActive ? "active" : "")}>{label}</NavLink>)}
          </nav>
          <div className="topbar-right">
            <span className="health" title={health.isError ? "Server unreachable" : h ? `v${h.version} · db ${h.db}` : "checking…"}>
              <i className={`dot ${healthClass}`} />{health.isError ? "offline" : h?.status ?? "…"}
            </span>
            <button className="iconbtn" onClick={reload} title="Reload (r)" aria-label="Reload">{fetching ? "◌" : "↻"}</button>
            <button className="iconbtn" onClick={cycle} title={`Theme: ${theme} (t)`} aria-label="Toggle theme">{themeIcon}</button>
            {userName && <span className="nowrap" style={{ opacity: 0.85 }}>{userName}</span>}
            <Clock />
          </div>
        </div>
      </header>
      <div className="statusline">
        <span><kbd>r</kbd> reload · <kbd>o</kbd> overview/detail · <kbd>?</kbd> help</span>
        {fetching > 0 && <span>Refreshing…</span>}
        {snap.error && !(snap.error instanceof ApiError && snap.error.unauthorized) && <span className="bad">Error: {(snap.error as Error).message}</span>}
        {snap.data?.sources.some((s) => s.last_run?.status === "error") && <span className="warn">A broker's last sync failed, see Overview</span>}
      </div>
      <main className="main"><Outlet /></main>
      <footer className="footer">
        {KEYS.map(([k, label]) => <span className="key" key={k}><b>{k}</b>{label}</span>)}
        <span style={{ marginLeft: "auto" }}>kgbctl web{h ? ` · server v${h.version}` : ""}</span>
      </footer>
    </div>
  );
}
