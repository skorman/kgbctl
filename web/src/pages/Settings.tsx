import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { api, API_BASE } from "../api/client";
import { useHealth } from "../api/hooks";
import { useAuth } from "../lib/auth";
import { useTheme } from "../lib/theme";
import { Card, ErrorBanner } from "../components/Common";

export function Settings() {
  const { userName, signIn, signOut } = useAuth();
  const { theme, setTheme } = useTheme();
  const health = useHealth();
  const [revealed, setRevealed] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const rotate = useMutation({
    mutationFn: api.rotateToken,
    onSuccess: (r) => { signIn(r.token, r.name); setRevealed(r.token); },
  });
  const copy = async () => {
    if (!revealed) return;
    try { await navigator.clipboard.writeText(revealed); setCopied(true); setTimeout(() => setCopied(false), 1500); } catch { /* no clipboard */ }
  };

  return (
    <div>
      <div className="page-head"><div><h1>Settings</h1><p>Your API token, the server, and appearance.</p></div></div>
      <div className="grid cols-2">
        <Card title="API token">
          <dl className="kv">
            <dt>Signed in as</dt><dd><b>{userName ?? "unknown user"}</b></dd>
            <dt>Token</dt><dd className="muted">stored in this browser only (localStorage)</dd>
          </dl>
          <p className="muted" style={{ fontSize: 12.5 }}>
            Rotating issues a fresh token for your user. The current one stops working immediately, on every device using it. The browser switches to the new token automatically; copy it anywhere else you use it (curl, scripts, the TUI machine).
          </p>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button className="btn" onClick={() => { if (confirm("Rotate your token? The current one stops working immediately.")) rotate.mutate(); }} disabled={rotate.isPending}>
              {rotate.isPending ? "Rotating…" : "Rotate token"}
            </button>
            <button className="btn ghost" onClick={signOut}>Sign out</button>
          </div>
          <ErrorBanner error={rotate.error} />
          {revealed && (
            <div style={{ marginTop: 14 }}>
              <div className="banner good">New token issued. It's shown once; store it now.</div>
              <div className="token-box">{revealed}</div>
              <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
                <button className="btn sm" onClick={copy}>{copied ? "Copied ✓" : "Copy"}</button>
                <button className="btn ghost sm" onClick={() => setRevealed(null)}>Hide</button>
              </div>
            </div>
          )}
        </Card>
        <div style={{ display: "grid", gap: 14 }}>
          <Card title="Server">
            <dl className="kv">
              <dt>API</dt><dd className="mono">{API_BASE || window.location.origin}/api/v1</dd>
              <dt>Status</dt><dd>{health.data ? <span className={`pill ${health.data.status === "ok" ? "good" : "warn"}`}>{health.data.status}</span> : health.isError ? <span className="pill bad">unreachable</span> : "…"}</dd>
              <dt>Version</dt><dd>{health.data?.version ?? "—"}</dd>
              <dt>Database</dt><dd>{health.data?.db ?? "—"}</dd>
            </dl>
            <p className="muted" style={{ fontSize: 12.5, marginBottom: 0 }}>
              Locally the Vite dev server proxies <code className="mono">/api</code> to <code className="mono">127.0.0.1:8080</code>. Point it elsewhere with <code className="mono">VITE_API_PROXY</code>, or set <code className="mono">VITE_API_BASE</code> to call a host directly.
            </p>
          </Card>
          <Card title="Appearance">
            <div style={{ display: "flex", gap: 8 }}>
              {(["dark", "light", "system"] as const).map((t) => (
                <button key={t} className={`btn sm ${theme === t ? "" : "ghost"}`} onClick={() => setTheme(t)}>{t[0].toUpperCase() + t.slice(1)}</button>
              ))}
            </div>
          </Card>
          <Card title="Keyboard">
            <dl className="kv">
              <dt><kbd>o</kbd></dt><dd>Overview / Detail</dd>
              <dt><kbd>←</kbd> <kbd>→</kbd></dt><dd>Previous / next ETF (Detail)</dd>
              <dt><kbd>↑</kbd> <kbd>↓</kbd> / <kbd>j</kbd> <kbd>k</kbd></dt><dd>Move row (Detail)</dd>
              <dt><kbd>r</kbd></dt><dd>Reload from the server</dd>
              <dt><kbd>t</kbd></dt><dd>Toggle theme</dd>
              <dt><kbd>?</kbd></dt><dd>This page</dd>
            </dl>
          </Card>
        </div>
      </div>
    </div>
  );
}
