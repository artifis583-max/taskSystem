import { useState } from "react";
import type { NewTask, Schedule, Size, TaskInfo } from "./types";

const DAYS = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
type Repeat = Schedule["kind"];
type Start = "today" | "tomorrow" | "date";
type Ends = "never" | "date";

function addDays(iso: string, n: number) {
  const d = new Date(`${iso}T00:00:00`);
  d.setDate(d.getDate() + n);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function isoDay(offset = 0) {
  const d = new Date();
  d.setDate(d.getDate() + offset);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

interface Props {
  /** Editing an existing task (dashboard). */
  initial?: TaskInfo;
  onSave(t: NewTask): void | Promise<void>;
  onCancel(): void;
}

export function AddTaskForm({ initial, onSave, onCancel }: Props) {
  const [title, setTitle] = useState(initial?.title ?? "");
  const [details, setDetails] = useState(initial?.details ?? "");
  const [size, setSize] = useState<Size>(initial?.size ?? "small");
  const [repeat, setRepeat] = useState<Repeat>(initial?.schedule.kind ?? "once");
  const [days, setDays] = useState<number[]>(initial?.schedule.kind === "weekdays" ? initial.schedule.days : [0, 1, 2, 3, 4]);
  const [n, setN] = useState(initial?.schedule.kind === "every_n_days" ? initial.schedule.n : 2);
  const initialStart: Start = !initial || initial.startDate === isoDay() ? "today" : initial.startDate === isoDay(1) ? "tomorrow" : "date";
  const [start, setStart] = useState<Start>(initialStart);
  const [date, setDate] = useState(initial?.startDate ?? isoDay(2));
  const [ends, setEnds] = useState<Ends>(initial?.endDate ? "date" : "never");
  const [endDate, setEndDate] = useState(initial?.endDate ?? addDays(isoDay(), 30));
  const [saving, setSaving] = useState(false);

  const schedule: Schedule =
    repeat === "weekdays" ? { kind: "weekdays", days } : repeat === "every_n_days" ? { kind: "every_n_days", n } : { kind: repeat };
  const startDate = start === "today" ? isoDay() : start === "tomorrow" ? isoDay(1) : date;
  const hasEnd = repeat !== "once" && ends === "date";
  const endOk = !hasEnd || (/^\d{4}-\d{2}-\d{2}$/.test(endDate) && endDate >= startDate);
  const valid = title.trim().length > 0 && (repeat !== "weekdays" || days.length > 0) && /^\d{4}-\d{2}-\d{2}$/.test(startDate) && endOk;

  return (
    <form
      className="form"
      onSubmit={async (e) => {
        e.preventDefault();
        if (!valid || saving) return;
        setSaving(true);
        // When editing, keep a past start date as it was.
        const keepStart = initial && start === "date" && date === initial.startDate;
        await onSave({
          title,
          details,
          size,
          schedule,
          startDate: keepStart ? initial.startDate : startDate,
          endDate: hasEnd ? endDate : null,
        });
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
              {s}
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
      <div className="field">
        {repeat === "once" ? "On" : "Starts"}
        <div className="seg">
          {(
            [
              ["today", "Today"],
              ["tomorrow", "Tomorrow"],
              ["date", "Pick date"],
            ] as [Start, string][]
          ).map(([k, label]) => (
            <button type="button" key={k} className={start === k ? "on" : ""} onClick={() => setStart(k)}>
              {label}
            </button>
          ))}
        </div>
        {start === "date" && (
          <input type="date" value={date} min={initial?.startDate && initial.startDate < isoDay() ? initial.startDate : isoDay()} onChange={(e) => setDate(e.target.value)} />
        )}
      </div>
      {repeat !== "once" && (
        <div className="field">
          Ends
          <div className="seg">
            {(
              [
                ["never", "Never"],
                ["date", "On a date"],
              ] as [Ends, string][]
            ).map(([k, label]) => (
              <button
                type="button"
                key={k}
                className={ends === k ? "on" : ""}
                onClick={() => {
                  setEnds(k);
                  if (k === "date" && endDate < startDate) setEndDate(addDays(startDate, 30));
                }}
              >
                {label}
              </button>
            ))}
          </div>
          {ends === "date" && (
            <>
              <input type="date" value={endDate} min={startDate} onChange={(e) => setEndDate(e.target.value)} />
              {!endOk && <span className="field-error">End date must be on or after the start date.</span>}
            </>
          )}
        </div>
      )}
      <div className="form-actions">
        <button type="button" className="mini ghost" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="mini" disabled={!valid || saving}>
          {initial ? "Save" : "Add task"}
        </button>
      </div>
    </form>
  );
}
