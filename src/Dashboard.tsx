import { useCallback, useEffect, useState } from "react";
import type { Api } from "./api";
import type { AppState, DayInfo, Schedule, Settings, Stats, TaskInfo } from "./types";
import { AddTaskForm } from "./AddTaskForm";
import { applyAppearance } from "./motion";

type Page = "tasks" | "history" | "progress" | "settings";
const PAGES: { id: Page; label: string; icon: string }[] = [
  { id: "tasks", label: "Tasks", icon: "M3 4.5h10M3 8h10M3 11.5h6" },
  { id: "history", label: "History", icon: "M8 3.5a4.5 4.5 0 1 1-4.5 4.5M8 5.5V8l2 1.5M3.5 3.5v2.5H6" },
  { id: "progress", label: "Progress", icon: "M3 12.5V9M6.3 12.5V6M9.7 12.5V7.5M13 12.5V3.5" },
  { id: "settings", label: "Settings", icon: "M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4zM8 1.8v1.7M8 12.5v1.7M2.6 4.9l1.5.9M11.9 10.2l1.5.9M2.6 11.1l1.5-.9M11.9 5.8l1.5-.9" },
];

export function Dashboard({ api }: { api: Api }) {
  const [page, setPage] = useState<Page>("tasks");
  const [state, setState] = useState<AppState | null>(null);
  const [tasks, setTasks] = useState<TaskInfo[]>([]);
  const [history, setHistory] = useState<DayInfo[]>([]);
  const [stats, setStats] = useState<Stats | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [s, t, h, st] = await Promise.all([api.getState(), api.listTasks(), api.getHistory(), api.getStats()]);
      setState(s);
      setTasks(t);
      setHistory(h);
      setStats(st);
    } catch (e) {
      setError(String(e));
    }
  }, [api]);

  useEffect(() => {
    load();
    return api.onChanged(load);
  }, [api, load]);

  useEffect(() => {
    if (state) applyAppearance(state.settings.theme, state.settings.motion);
  }, [state?.settings.theme, state?.settings.motion]);

  const run = async (fn: () => Promise<unknown>) => {
    try {
      await fn();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="dash">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden>
            <svg viewBox="0 0 20 16">
              <path d="M3 8.5l4.5 4.5L17 3.5" />
            </svg>
          </span>
          Task Widget
        </div>
        <nav>
          {PAGES.map((p) => (
            <button key={p.id} className={page === p.id ? "nav-item active" : "nav-item"} onClick={() => setPage(p.id)}>
              <svg viewBox="0 0 16 16" aria-hidden>
                <path d={p.icon} />
              </svg>
              {p.label}
            </button>
          ))}
        </nav>
        {state && (
          <div className="side-summary">
            <span className="stat">
              <i className="coin" aria-hidden />
              {state.balance}
            </span>
            <span className="stat">
              Lv {state.level}
              {state.streak > 0 && <span className="streak">· {state.streak}d</span>}
            </span>
          </div>
        )}
      </aside>
      <main className="content">
        {error && (
          <div className="dash-error" role="alert">
            {error}
            <button className="icon-btn" onClick={() => setError(null)} aria-label="Dismiss">
              <svg viewBox="0 0 16 16" aria-hidden>
                <path d="M4 4l8 8M12 4l-8 8" />
              </svg>
            </button>
          </div>
        )}
        {page === "tasks" && <TasksPage api={api} tasks={tasks} run={run} />}
        {page === "history" && <HistoryPage history={history} />}
        {page === "progress" && stats && <ProgressPage stats={stats} />}
        {page === "settings" && state && <SettingsPage api={api} settings={state.settings} run={run} />}
      </main>
    </div>
  );
}

// ---------- tasks ----------

function TasksPage({ api, tasks, run }: { api: Api; tasks: TaskInfo[]; run(fn: () => Promise<unknown>): Promise<void> }) {
  const [editing, setEditing] = useState<TaskInfo | "new" | null>(null);
  const [confirmRemove, setConfirmRemove] = useState<number | null>(null);
  const active = tasks.filter((t) => t.status === "active").length;
  const paused = tasks.length - active;

  return (
    <>
      <header className="page-head">
        <div>
          <h1>Tasks</h1>
          <p className="muted">
            {active} active{paused > 0 && ` · ${paused} paused`}
          </p>
        </div>
        {!editing && (
          <button className="mini big" onClick={() => setEditing("new")}>
            + New task
          </button>
        )}
      </header>
      {editing && (
        <div className="dash-card form-card">
          <h2>{editing === "new" ? "New task" : "Edit task"}</h2>
          <AddTaskForm
            key={editing === "new" ? "new" : editing.id}
            initial={editing === "new" ? undefined : editing}
            onCancel={() => setEditing(null)}
            onSave={(t) =>
              run(async () => {
                if (editing === "new") await api.createTask(t);
                else await api.updateTask(editing.id, t);
                setEditing(null);
              })
            }
          />
          {editing !== "new" && <p className="hint small left">Changes apply from today on. Finished days and earned coins stay as they were.</p>}
        </div>
      )}
      {!editing && tasks.length === 0 && <p className="empty">No tasks yet. Add one here or from the widget.</p>}
      {!editing && tasks.length > 0 && (
        <div className="dash-list">
          {tasks.map((t) => (
            <div key={t.id} className={t.status === "paused" ? "task-row paused" : "task-row"}>
              {confirmRemove === t.id ? (
                <div className="confirm-row">
                  <span>Remove “{t.title}”? Its history and earned coins are kept.</span>
                  <button className="mini danger" onClick={() => run(() => api.removeTask(t.id)).then(() => setConfirmRemove(null))}>
                    Remove
                  </button>
                  <button className="mini ghost" onClick={() => setConfirmRemove(null)}>
                    Cancel
                  </button>
                </div>
              ) : (
                <>
                  <div className="task-main">
                    <span className="title">
                      <span className="title-text">{t.title}</span>
                      {t.status === "paused" && <span className="paused-tag">Paused</span>}
                    </span>
                    {t.details && <span className="details">{t.details}</span>}
                  </div>
                  <span className="sched">{scheduleLabel(t.schedule, t.startDate)}</span>
                  <span className={`size-tag ${t.size}`}>{t.size}</span>
                  <div className="row-actions">
                    <button className="mini ghost" onClick={() => setEditing(t)}>
                      Edit
                    </button>
                    <button
                      className="mini ghost"
                      onClick={() => run(() => api.setTaskStatus(t.id, t.status === "active" ? "paused" : "active"))}
                    >
                      {t.status === "active" ? "Pause" : "Resume"}
                    </button>
                    <button className="mini ghost" onClick={() => setConfirmRemove(t.id)}>
                      Remove
                    </button>
                  </div>
                </>
              )}
            </div>
          ))}
        </div>
      )}
    </>
  );
}

const DAY_NAMES = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const shortDate = (iso: string) => new Date(`${iso}T00:00:00`).toLocaleDateString(undefined, { month: "short", day: "numeric" });

export function scheduleLabel(s: Schedule, start: string): string {
  const today = new Date();
  const todayIso = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;
  const from = start > todayIso ? ` · from ${shortDate(start)}` : "";
  switch (s.kind) {
    case "once":
      return `Once · ${shortDate(start)}`;
    case "daily":
      return `Every day${from}`;
    case "every_n_days":
      return `Every ${s.n} days${from}`;
    case "weekdays": {
      const d = [...s.days].sort().join(",");
      const label = d === "0,1,2,3,4" ? "Weekdays" : d === "5,6" ? "Weekends" : s.days.length === 7 ? "Every day" : s.days.map((i) => DAY_NAMES[i]).join(", ");
      return label + from;
    }
  }
}

// ---------- history ----------

const RESULT_LABEL: Record<DayInfo["result"], string> = {
  qualified: "Streak kept",
  neutral: "No tasks",
  broken: "Streak broken",
  revived: "Revived",
};

function HistoryPage({ history }: { history: DayInfo[] }) {
  return (
    <>
      <header className="page-head">
        <div>
          <h1>History</h1>
          <p className="muted">A day keeps your streak when at least half its tasks are done.</p>
        </div>
      </header>
      {history.length === 0 ? (
        <p className="empty">No finished days yet. Each day shows up here after midnight.</p>
      ) : (
        <div className="dash-card table-card">
          <table className="history-table">
            <thead>
              <tr>
                <th>Day</th>
                <th>Done</th>
                <th>Result</th>
                <th className="num">Streak</th>
              </tr>
            </thead>
            <tbody>
              {history.map((d) => (
                <tr key={d.date}>
                  <td>{new Date(`${d.date}T00:00:00`).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })}</td>
                  <td>
                    {d.eligible > 0 ? (
                      <span className="done-cell">
                        <span className="mini-track">
                          <span style={{ width: `${(d.completed / d.eligible) * 100}%` }} />
                        </span>
                        {d.completed}/{d.eligible}
                      </span>
                    ) : (
                      <span className="muted">—</span>
                    )}
                  </td>
                  <td>
                    <span className={`result ${d.result}`}>
                      <ResultIcon result={d.result} />
                      {RESULT_LABEL[d.result]}
                    </span>
                  </td>
                  <td className="num">{d.streakAfter}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

function ResultIcon({ result }: { result: DayInfo["result"] }) {
  const d =
    result === "qualified" ? "M3.5 8.5l3 3 6-7" : result === "broken" ? "M4.5 4.5l7 7M11.5 4.5l-7 7" : result === "revived" ? "M12.5 8a4.5 4.5 0 1 1-1.3-3.2M12.5 3v2.5H10" : "M4 8h8";
  return (
    <svg viewBox="0 0 16 16" aria-hidden>
      <path d={d} />
    </svg>
  );
}

// ---------- progress ----------

function ProgressPage({ stats }: { stats: Stats }) {
  const next = stats.levels.find((l) => l.level === stats.level + 1);
  const tiles: [string, string, string?][] = [
    ["Level", String(stats.level)],
    ["Current streak", `${stats.streak}`, stats.streak === 1 ? "day" : "days"],
    ["Best streak", `${stats.bestStreak}`, stats.bestStreak === 1 ? "day" : "days"],
    ["Lifetime XP", stats.xp.toLocaleString()],
    ["Tasks done", stats.tasksCompleted.toLocaleString()],
    ["Last 30 days", stats.completion30d == null ? "—" : `${stats.completion30d}%`, stats.completion30d == null ? undefined : "done"],
    ["Coins earned", stats.coinsEarned.toLocaleString()],
    ["Coins spent", stats.coinsSpent.toLocaleString()],
  ];
  return (
    <>
      <header className="page-head">
        <div>
          <h1>Progress</h1>
          <p className="muted">Your level needs both lifetime XP and a current streak.</p>
        </div>
      </header>
      <div className="tiles">
        {tiles.map(([label, value, unit]) => (
          <div className="tile" key={label}>
            <span className="tile-label">{label}</span>
            <span className="tile-value">
              {value}
              {unit && <span className="tile-unit"> {unit}</span>}
            </span>
          </div>
        ))}
      </div>

      {next && (
        <div className="dash-card next-level">
          <h2>Next: level {next.level}</h2>
          <Meter label="XP" value={stats.xp} goal={next.xp} />
          <Meter label="Streak" value={stats.streak} goal={next.streak} unit="days" />
        </div>
      )}

      <div className="dash-card table-card">
        <table className="levels-table">
          <thead>
            <tr>
              <th>Level</th>
              <th className="num">XP needed</th>
              <th className="num">Streak needed</th>
              <th>Coin bonus</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {stats.levels.map((l) => (
              <tr key={l.level} className={l.level === stats.level ? "current" : undefined}>
                <td>Level {l.level}</td>
                <td className="num">{l.xp.toLocaleString()}</td>
                <td className="num">{l.streak} days</td>
                <td>+{Math.round((Math.pow(1.15, l.level - 1) - 1) * 100)}%</td>
                <td>{l.level === stats.level ? <span className="you">You're here</span> : null}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

function Meter({ label, value, goal, unit }: { label: string; value: number; goal: number; unit?: string }) {
  const pct = goal <= 0 ? 100 : Math.min(100, (value / goal) * 100);
  const done = value >= goal;
  return (
    <div className="meter" role="meter" aria-valuemin={0} aria-valuemax={goal} aria-valuenow={Math.min(value, goal)} aria-label={label}>
      <div className="meter-head">
        <span>{label}</span>
        <span className="meter-value">
          {done ? "Done · " : ""}
          {value.toLocaleString()} / {goal.toLocaleString()}
          {unit ? ` ${unit}` : ""}
        </span>
      </div>
      <div className="track">
        <span style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

// ---------- settings ----------

function SettingsPage({ api, settings, run }: { api: Api; settings: Settings; run(fn: () => Promise<unknown>): Promise<void> }) {
  const [autostart, setAutostart] = useState<boolean | null>(null);
  useEffect(() => {
    api.getAutostart().then(setAutostart, () => setAutostart(false));
  }, [api]);
  const save = (patch: Partial<Settings>) => run(() => api.updateSettings({ ...settings, ...patch }));

  return (
    <>
      <header className="page-head">
        <div>
          <h1>Settings</h1>
          <p className="muted">Everything is stored on this PC only.</p>
        </div>
      </header>

      <div className="dash-card settings">
        <SettingRow title="Start with Windows" hint="Open the widget when you sign in.">
          <Toggle
            on={!!autostart}
            disabled={autostart === null}
            label="Start with Windows"
            onChange={(v) => run(async () => setAutostart(await api.setAutostart(v)))}
          />
        </SettingRow>
        <SettingRow title="Keep widget on top" hint="Show the widget above other windows.">
          <Toggle on={settings.alwaysOnTop} label="Keep widget on top" onChange={(v) => save({ alwaysOnTop: v })} />
        </SettingRow>
        <SettingRow title="Theme">
          <Segmented
            value={settings.theme}
            options={[
              ["system", "System"],
              ["light", "Light"],
              ["dark", "Dark"],
            ]}
            onChange={(v) => save({ theme: v as Settings["theme"] })}
          />
        </SettingRow>
        <SettingRow title="Animations" hint="System follows Windows › Accessibility › Visual effects.">
          <Segmented
            value={settings.motion}
            options={[
              ["system", "System"],
              ["full", "On"],
              ["reduced", "Off"],
            ]}
            onChange={(v) => save({ motion: v as Settings["motion"] })}
          />
        </SettingRow>
      </div>
    </>
  );
}

function SettingRow({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="setting-row">
      <div className="setting-text">
        <span className="title">{title}</span>
        {hint && <span className="details">{hint}</span>}
      </div>
      {children}
    </div>
  );
}

function Toggle({ on, onChange, label, disabled }: { on: boolean; onChange(v: boolean): void; label: string; disabled?: boolean }) {
  return (
    <button role="switch" aria-checked={on} aria-label={label} disabled={disabled} className={on ? "toggle on" : "toggle"} onClick={() => onChange(!on)}>
      <span />
    </button>
  );
}

function Segmented({ value, options, onChange }: { value: string; options: [string, string][]; onChange(v: string): void }) {
  return (
    <div className="seg compact" role="radiogroup">
      {options.map(([v, label]) => (
        <button key={v} role="radio" aria-checked={value === v} className={value === v ? "on" : ""} onClick={() => onChange(v)}>
          {label}
        </button>
      ))}
    </div>
  );
}
