import { useCallback, useEffect, useRef, useState } from "react";
import type { Api } from "./api";
import type { AppState, CompleteResult } from "./types";
import { CountUp } from "./CountUp";
import { TaskList } from "./TaskList";
import { AddTaskForm } from "./AddTaskForm";

type Tab = "today" | "upcoming" | "rewards";
const TABS: { id: Tab; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "upcoming", label: "Upcoming" },
  { id: "rewards", label: "Rewards" },
];

export default function App({ api }: { api: Api }) {
  const [state, setState] = useState<AppState | null>(null);
  const [balance, setBalance] = useState(0);
  const [tab, setTab] = useState<Tab>("today");
  const [adding, setAdding] = useState(false);
  const [showLevel, setShowLevel] = useState(false);
  const [banner, setBanner] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const errTimer = useRef<number>();

  const refresh = useCallback(async () => {
    try {
      const s = await api.getState();
      setState(s);
      setBalance(s.balance);
    } catch (e) {
      showError(String(e));
    }
  }, [api]);

  function showError(msg: string) {
    setError(msg);
    window.clearTimeout(errTimer.current);
    errTimer.current = window.setTimeout(() => setError(null), 3500);
  }

  useEffect(() => {
    refresh();
    const off = api.onChanged(refresh);
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => {
      off();
      window.removeEventListener("focus", onFocus);
    };
  }, [api, refresh]);

  const onReward = (r: CompleteResult) => setBalance(r.balance);
  const onLanded = (r: CompleteResult) => {
    if (r.levelAfter > r.levelBefore) {
      setBanner(`Level up! LV ${r.levelAfter}`);
      window.setTimeout(() => setBanner(null), 2000);
    }
  };

  return (
    <div className="panel">
      <header className="header" data-tauri-drag-region>
        <span className="pill" title="Coins">
          <i className="coin" aria-hidden />
          <CountUp value={balance} />
        </span>
        <button className="pill" onClick={() => setShowLevel((v) => !v)} title="Level and streak">
          LV {state?.level ?? 1}
        </button>
        <button className="close" onClick={() => api.hideWindow()} aria-label="Hide widget">
          <svg viewBox="0 0 16 16" aria-hidden>
            <path d="M4 4l8 8M12 4l-8 8" />
          </svg>
        </button>
        {showLevel && state && (
          <div className="popover" onClick={() => setShowLevel(false)}>
            <div>STREAK {state.streak} {state.streak === 1 ? "DAY" : "DAYS"}</div>
            <div>{state.xp} XP</div>
            {state.nextLevel ? (
              <div className="muted">
                LV {state.nextLevel.level}: {state.nextLevel.xp} XP + {state.nextLevel.streak}-DAY STREAK
              </div>
            ) : (
              <div className="muted">MAX LEVEL</div>
            )}
          </div>
        )}
      </header>

      <nav className="tabs" role="tablist">
        {TABS.map((t) => (
          <button
            key={t.id}
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "tab active" : "tab"}
            onClick={() => {
              setTab(t.id);
              setAdding(false);
            }}
          >
            {t.label}
          </button>
        ))}
      </nav>

      <main className="body">
        {banner && <div className="banner">{banner}</div>}
        {tab === "today" &&
          (adding ? (
            <AddTaskForm
              onCancel={() => setAdding(false)}
              onSave={async (t) => {
                try {
                  await api.createTask(t);
                  setAdding(false);
                  refresh();
                } catch (e) {
                  showError(String(e));
                }
              }}
            />
          ) : state ? (
            <TaskList
              api={api}
              cards={state.cards}
              onReward={onReward}
              onLanded={onLanded}
              onSettled={refresh}
              onError={showError}
              onAdd={() => setAdding(true)}
            />
          ) : null)}
        {tab === "upcoming" && <p className="placeholder">Upcoming days are coming in a later build.</p>}
        {tab === "rewards" && <p className="placeholder">The reward shop is coming in a later build.</p>}
      </main>

      {error && (
        <div className="toast" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
