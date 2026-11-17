import { useState, type FormEvent } from "react";
import { api, ApiError } from "../api/client";
import { useAuth } from "../lib/auth";
import { useHealth } from "../api/hooks";
import gif from "../assets/kgb_horse_loading_animation.gif";

export function Login() {
  const { signIn } = useAuth();
  const health = useHealth();
  const [token, setToken] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const t = token.trim();
    if (!t) return;
    setBusy(true); setError(null);
    try {
      await api.verify(t);
      signIn(t, name.trim() || undefined);
    } catch (err) {
      setError(err instanceof ApiError && err.unauthorized ? "That token isn't recognised." : (err as Error).message);
    } finally { setBusy(false); }
  };

  return (
    <div className="login">
      <div className="login-stack">
        <img className="login-hero" src={gif} alt="KGB Capital Group" />
        <div className="card">
        <h1>Sign in</h1>
        <p className="muted" style={{ margin: 0 }}>Paste the API token you were issued. It's stored only in this browser.</p>
        <form onSubmit={submit}>
          <label className="muted" style={{ fontSize: 12.5 }}>API token
            <input className="input mono" type="password" autoComplete="off" autoFocus value={token} onChange={(e) => setToken(e.target.value)} placeholder="kgb_…" required />
          </label>
          <label className="muted" style={{ fontSize: 12.5 }}>Your name <span style={{ opacity: 0.7 }}>(optional, display only)</span>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Sam" />
          </label>
          {error && <div className="banner bad" role="alert">{error}</div>}
          <button className="btn" disabled={busy || !token.trim()}>{busy ? "Checking…" : "Sign in"}</button>
        </form>
        <p className="muted" style={{ fontSize: 12.5, marginBottom: 0 }}>
          Server: {health.data ? <span className="good">online · v{health.data.version}</span> : health.isError ? <span className="bad">unreachable, is <code>kgbctl -c serve</code> running?</span> : "checking…"}<br />
          Need a token? <code>kgbctl -c user-add --name You</code>
        </p>
        </div>
      </div>
    </div>
  );
}
