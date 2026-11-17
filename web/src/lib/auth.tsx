import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import { setTokenProvider } from "../api/client";

const KEY = "kgbctl.token";
const NAME_KEY = "kgbctl.user";

interface Auth {
  token: string | null;
  userName: string | null;
  signIn: (token: string, name?: string) => void;
  signOut: () => void;
}

const Ctx = createContext<Auth | null>(null);

const read = (k: string) => { try { return localStorage.getItem(k); } catch { return null; } };
const write = (k: string, v: string | null) => {
  try { v === null ? localStorage.removeItem(k) : localStorage.setItem(k, v); } catch { /* private mode */ }
};

export function AuthProvider({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(() => read(KEY));
  const [userName, setName] = useState<string | null>(() => read(NAME_KEY));

  // Installed during render, not in an effect: children's query effects run
  // before a parent's effect, and the very first request must carry the token.
  setTokenProvider(() => token);

  const signIn = useCallback((t: string, name?: string) => {
    write(KEY, t); setToken(t);
    if (name !== undefined) { write(NAME_KEY, name); setName(name); }
  }, []);
  const signOut = useCallback(() => {
    write(KEY, null); write(NAME_KEY, null); setToken(null); setName(null);
  }, []);

  const value = useMemo(() => ({ token, userName, signIn, signOut }), [token, userName, signIn, signOut]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useAuth(): Auth {
  const v = useContext(Ctx);
  if (!v) throw new Error("useAuth outside AuthProvider");
  return v;
}
