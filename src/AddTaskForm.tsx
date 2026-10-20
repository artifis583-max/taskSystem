import { useState } from "react";
import type { NewTask, Schedule, Size } from "./types";

const DAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
type Repeat = Schedule["kind"];

export function AddTaskForm({ onSave, onCancel }: { onSave(t: NewTask): void; onCancel(): void }) {
  const [title, setTitle] = useState("");
  const [details, setDetails] = useState("");
  const [size, setSize] = useState<Size>("small");
  const [repeat, setRepeat] = useState<Repeat>("once");
  const [days, setDays] = useState<number[]>([0, 1, 2, 3, 4]);
  const [n, setN] = useState(2);
  const [saving, setSaving] = useState(false);

  const schedule: Schedule =
    repeat === "weekdays" ? { kind: "weekdays", days } : repeat === "every_n_days" ? { kind: "every_n_days", n } : { kind: repeat };
  const valid = title.trim().length > 0 && (repeat !== "weekdays" || days.length > 0);

  return (
    <form
      className="form"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!valid || saving) return;
        setSaving(true);
        await onSave({ title, details, size, schedule });
        setSaving(false);
      }}
    >
      <label>
        Task
        <input autoFocus maxLength={120} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Read 20 pages" />
      </label>
      <label>
        Details
        <input maxLength={500} value={details} onChange={(e) => setDetails(e.target.value)} placeholder="Optional" />
      </label>
      <div className="field">
        Size
        <div className="seg">
          {(["small", "medium", "large"] as Size[]).map((s) => (
            <button type="button" key={s} className={size === s ? "on" : ""} onClick={() => setSize(s)}>
              {s} · {s === "small" ? 10 : s === "medium" ? 20 : 30}
            </button>
          ))}
        </div>
      </div>
      <div className="field">
        Repeat
        <div className="seg">
          {(
            [
              ["once", "Once"],
              ["daily", "Daily"],
              ["weekdays", "Days"],
              ["every_n_days", "Every N"],
            ] as [Repeat, string][]
          ).map(([k, label]) => (
            <button type="button" key={k} className={repeat === k ? "on" : ""} onClick={() => setRepeat(k)}>
              {label}
            </button>
          ))}
        </div>
        {repeat === "weekdays" && (
          <div className="seg days">
            {DAYS.map((d, i) => (
              <button
                type="button"
                key={d}
                className={days.includes(i) ? "on" : ""}
                onClick={() => setDays((cur) => (cur.includes(i) ? cur.filter((x) => x !== i) : [...cur, i].sort()))}
              >
                {d}
              </button>
            ))}
          </div>
        )}
        {repeat === "every_n_days" && (
          <div className="every">
            Every
            <input type="number" min={1} max={365} value={n} onChange={(e) => setN(Math.min(365, Math.max(1, Number(e.target.value) || 1)))} />
            days
          </div>
        )}
      </div>
      <div className="form-actions">
        <button type="button" className="mini ghost" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="mini" disabled={!valid || saving}>
          Add task
        </button>
      </div>
    </form>
  );
}
