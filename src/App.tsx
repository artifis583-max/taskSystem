import { useCallback, useEffect, useRef, useState } from "react";
import { CoinFly, type Flight } from "./CoinFly";
import { applyAppearance, prefersReducedMotion } from "./motion";
import type { Api } from "./api";
import type { AppState, CompleteResult } from "./types";
import { CountUp } from "./CountUp";
import { TaskList } from "./TaskList";
import { AddTaskForm } from "./AddTaskForm";
import { RewardsTab } from "./RewardsTab";
import { UpcomingTab } from "./UpcomingTab";

type Tab = "today" | "upcoming" | "rewards";
const TABS: { id: Tab; label: string }[] = [
  { id: "today", label: "Today" },
  { id: "upcoming", label: "Upcoming" },
  { id: "rewards", label: "Rewards" },
];

export default function App({ api }: { api: Api }) {
  const [state, setState] = useState<AppState | null>(null);
  // Shown balance. While a coin chip is flying it waits, so the total only changes when the coin lands.
  const [balance, setBalance] = useState(0);
  const latestBalance = useRef(0);
  const [flights, setFlights] = useState<Flight[]>([]);
  const flying = useRef(0);
  const flightId = useRef(0);
  const coinTarget = useRef<HTMLSpanElement>(null);
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
      latestBalance.current = s.balance;
      if (flying.current === 0) setBalance(s.balance);
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
    if (state) applyAppearance(state.settings.theme, state.settings.motion);
  }, [state?.settings.theme, state?.settings.motion]);

  useEffect(
    () =>
      api.onAddTask(() => {
        setTab("today");
        setAdding(true);
      }),
    [api],
  );

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

  const onReward = (r: CompleteResult, from: DOMRect | null) => {
    latestBalance.current = Math.max(latestBalance.current, r.balance);
    const to = coinTarget.current?.getBoundingClientRect();
    if (!from || !to || prefersReducedMotion() || r.coins <= 0) {
      if (flying.current === 0) setBalance(latestBalance.current);
      return;
    }
    flying.current++;
    setFlights((f) => [...f, { id: ++flightId.current, coins: r.coins, from, to, balance: r.balance }]);
  };
  const onFlightLanded = (f: Flight) => {
    flying.current--;
    setFlights((all) => all.filter((x) => x.id !== f.id));
    setBalance(flying.current === 0 ? latestBalance.current : f.balance);
  };
  const bannerTimer = useRef<number>();
  function showBanner(msg: string) {
    setBanner(msg);
    window.clearTimeout(bannerTimer.current);
    bannerTimer.current = window.setTimeout(() => setBanner(null), 2200);
  }
  const onLanded = (r: CompleteResult) => {
    if (r.levelAfter > r.levelBefore) showBanner(`Level up! You're now level ${r.levelAfter}`);
  };

  const cards = state?.cards ?? [];
  const done = cards.filter((c) => c.status === "completed").length;
  const progress = { done, total: cards.length, left: cards.length - done };
  const day = formatDay(state?.today);

  return (
    <div className="panel">
      <header className="header" data-tauri-drag-region>
        <div className="heading" data-tauri-drag-region>
          <span className="weekday" data-tauri-drag-region>{day.weekday}</span>
          <span className="date" data-tauri-drag-region>{day.date}</span>
        </div>
        <span className="stat" title="Coins" ref={coinTarget}>
          <i className="coin" aria-hidden />
          <CountUp value={balance} />
        </span>
        <button
          className={showLevel ? "stat level open" : "stat level"}
          onClick={() => setShowLevel((v) => !v)}
          aria-expanded={showLevel}
          title="Level and streak"
        >
          Lv {state?.level ?? 1}
          {state && state.streak > 0 && <span className="streak">· {state.streak}d</span>}
        </button>
        <button className="icon-btn" onClick={() => api.openDashboard().catch((e) => showError(String(e)))} aria-label="Open dashboard" title="Dashboard">
          <svg viewBox="0 0 16 16" aria-hidden className="grid-icon">
            <rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1" />
            <rect x="9" y="2.5" width="4.5" height="4.5" rx="1" />
            <rect x="2.5" y="9" width="4.5" height="4.5" rx="1" />
            <rect x="9" y="9" width="4.5" height="4.5" rx="1" />
          </svg>
        </button>
        <button className="icon-btn" onClick={() => api.hideWindow()} aria-label="Hide widget" title="Hide to tray">
          <svg viewBox="0 0 16 16" aria-hidden>
            <path d="M4 4l8 8M12 4l-8 8" />
          </svg>
        </button>
        {showLevel && state && (
          <div className="popover" onClick={() => setShowLevel(false)}>
            <div className="pop-row">
              <span>Streak</span>
              <strong>
                {state.streak} {state.streak === 1 ? "day" : "days"}
              </strong>
            </div>
            <div className="pop-row">
              <span>Lifetime XP</span>
              <strong>{state.xp}</strong>
            </div>
            <div className="pop-note">
              {state.nextLevel
                ? `Level ${state.nextLevel.level} needs ${state.nextLevel.xp} XP and a ${state.nextLevel.streak}-day streak.`
                : "You're at the top level."}
            </div>
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
            {t.id === "today" && progress.left > 0 && <span className="badge">{progress.left}</span>}
          </button>
        ))}
      </nav>

      {tab === "today" && !adding && progress.total > 0 && (
        <div className="progress" aria-label={`${progress.done} of ${progress.total} done`}>
          <div className="track">
            <span style={{ width: `${(progress.done / progress.total) * 100}%` }} />
          </div>
          <span className="progress-text">
            {progress.done}/{progress.total} done
          </span>
        </div>
      )}

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
            <>
            {state.revives.brokenDate && (
              <button className="alert slim" onClick={() => setTab("rewards")}>
                <span>Your {state.revives.streakToRestore}-day streak broke yesterday.</span>
                <strong>Save it →</strong>
              </button>
            )}
            <TaskList
              api={api}
              cards={state.cards}
              onReward={onReward}
              onLanded={onLanded}
              onSettled={refresh}
              onError={showError}
              onAdd={() => setAdding(true)}
            />
            </>
          ) : null)}
        {tab === "upcoming" && state && <UpcomingTab state={state} />}
        {tab === "rewards" && state && (
          <RewardsTab api={api} state={state} onError={showError} onBanner={showBanner} />
        )}
      </main>

      {flights.map((f) => (
        <CoinFly key={f.id} flight={f} onLanded={onFlightLanded} />
      ))}

      {error && (
        <div className="toast" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}

function formatDay(iso?: string) {
  const d = iso ? new Date(`${iso}T00:00:00`) : new Date();
  return {
    weekday: d.toLocaleDateString(undefined, { weekday: "long" }),
    date: d.toLocaleDateString(undefined, { month: "short", day: "numeric" }),
  };
}
