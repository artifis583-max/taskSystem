import type { AppState } from "./types";

export function UpcomingTab({ state }: { state: AppState }) {
  const days = state.upcoming.filter((d) => d.items.length > 0);
  if (days.length === 0) {
    return <p className="placeholder">Nothing planned for the next 7 days.</p>;
  }
  return (
    <div className="list upcoming">
      {days.map((d) => (
        <section key={d.date} className="day-group">
          <h3 className="section">
            {dayLabel(d.date, state.today)}
            <span className="day-count">{d.items.length}</span>
          </h3>
          <div className="day-items">
            {d.items.map((t) => (
              <div className="up-row" key={t.taskId}>
                <span className={`size-dot ${t.size}`} aria-hidden />
                <span className="up-title">{t.title}</span>
                {t.details && <span className="up-details">{t.details}</span>}
              </div>
            ))}
          </div>
        </section>
      ))}
      <p className="hint small">Anything you don't finish today also moves to tomorrow.</p>
    </div>
  );
}

function dayLabel(iso: string, today: string): string {
  const d = new Date(`${iso}T00:00:00`);
  const diff = Math.round((d.getTime() - new Date(`${today}T00:00:00`).getTime()) / 864e5);
  if (diff === 1) return "Tomorrow";
  return d.toLocaleDateString(undefined, { weekday: "long", month: "short", day: "numeric" });
}
