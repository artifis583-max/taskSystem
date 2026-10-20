// Browser-only stand-in for the Rust backend so the UI can be previewed with `npm run dev`.
// Not used inside the app. Real rules live in src-tauri/src.
import type { Api } from "./api";
import type { AppState, Size, TaskCard } from "./types";

const BASE: Record<Size, number> = { small: 10, medium: 20, large: 30 };

export function createMockApi(): Api {
  let nextId = 1;
  let seq = 0;
  let balance = 240;
  let xp = 230;
  const listeners = new Set<() => void>();
  const mk = (title: string, details: string, size: Size, rolloverCount = 0): TaskCard => {
    const id = nextId++;
    const coins = Math.max(1, Math.round(BASE[size] * Math.pow(0.8, rolloverCount)));
    return { occurrenceId: id, taskId: id, title, details, size, status: "pending", rolloverCount, coins, xp: BASE[size], completedSeq: null };
  };
  let cards: TaskCard[] = [
    mk("Review lecture notes", "Chapters 4–5", "small"),
    mk("Finish DSA practice", "2 problems from the sheet", "medium"),
    mk("Read 20 pages", "", "small", 2),
    mk("Gym", "Leg day", "large"),
  ];
  const ordered = () => [
    ...cards.filter((c) => c.status === "pending"),
    ...cards.filter((c) => c.status === "completed").sort((a, b) => (a.completedSeq ?? 0) - (b.completedSeq ?? 0)),
  ];
  const level = () => (xp >= 300 ? 3 : xp >= 100 ? 2 : 1);
  const state = (): AppState => ({
    today: new Date().toISOString().slice(0, 10),
    balance, xp, streak: 4, level: level(),
    nextLevel: { level: level() + 1, xp: 700, streak: 14 },
    cards: structuredClone(ordered()),
  });
  const wait = (ms = 120) => new Promise((r) => setTimeout(r, ms));
  const changed = () => listeners.forEach((l) => l());
  const done = new Set<string>();
  return {
    async getState() { await wait(30); return state(); },
    async createTask(t) {
      await wait();
      const c = mk(t.title.trim(), t.details.trim(), t.size);
      cards.push(c); changed(); return c.taskId;
    },
    async complete(opId, id) {
      await wait();
      const c = cards.find((x) => x.occurrenceId === id);
      if (!c || c.status !== "pending" || done.has(opId)) throw "This task is already done.";
      done.add(opId);
      const before = level();
      c.status = "completed"; c.completedSeq = ++seq; balance += c.coins; xp += c.xp;
      changed();
      return { coins: c.coins, xp: c.xp, balance, levelBefore: before, levelAfter: level() };
    },
    async uncomplete(_op, id) {
      await wait();
      const c = cards.find((x) => x.occurrenceId === id);
      if (!c || c.status !== "completed") throw "This task isn't done.";
      if (balance < c.coins) throw "You've already spent these coins, so this can't be undone.";
      c.status = "pending"; c.completedSeq = null; balance -= c.coins; xp -= c.xp;
      changed(); return balance;
    },
    async skip(id) { await wait(); cards = cards.filter((c) => c.occurrenceId !== id); changed(); },
    async removeTask(id) { await wait(); cards = cards.filter((c) => c.taskId !== id); changed(); },
    onChanged(cb) { listeners.add(cb); return () => listeners.delete(cb); },
    hideWindow() { alert("In the app this hides the widget to the tray."); },
  };
}
