import { useEffect } from "react";

/** Keyboard shortcuts, ignored while typing in a field. */
export function useKeys(handler: (key: string, e: KeyboardEvent) => void, deps: unknown[] = []) {
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable)) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      handler(e.key, e);
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
}
