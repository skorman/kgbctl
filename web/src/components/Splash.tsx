import { useEffect, useState } from "react";
import gif from "../assets/kgb_horse_loading_animation.gif";

/** The TUI's startup animation: play the horse once, any key or click skips it. */
export function Splash({ onDone, minMs = 2600 }: { onDone: () => void; minMs?: number }) {
  const [leaving, setLeaving] = useState(false);
  useEffect(() => {
    let finished = false;
    const finish = () => {
      if (finished) return;
      finished = true;
      setLeaving(true);
      setTimeout(onDone, 420);
    };
    const timer = setTimeout(finish, minMs);
    const onKey = () => finish();
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onKey);
    return () => { clearTimeout(timer); window.removeEventListener("keydown", onKey); window.removeEventListener("pointerdown", onKey); };
  }, [onDone, minMs]);
  return (
    <div className={`splash${leaving ? " leaving" : ""}`} role="presentation">
      <img src={gif} alt="KGB Capital Group" />
      <div className="hint">Press any key to continue</div>
    </div>
  );
}

export function Loading({ text = "Loading…" }: { text?: string }) {
  return (
    <div className="loading" role="status" aria-live="polite">
      <img src={gif} alt="" />
      <span>{text}</span>
    </div>
  );
}
