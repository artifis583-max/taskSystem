// Browser-only stand-in for the Rust backend so the UI can be previewed with `npm run dev`.
// Not used inside the app. Real rules live in src-tauri/src.
import type { Api } from "./api";
import type { AppState, DayInfo, Redemption, Reward, Settings, Size, TaskCard, TaskInfo } from "./types";

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
  let settings: Settings = { alwaysOnTop: false, theme: "system", motion: "system" };
  let autostart = false;
  const iso = (offset: number) => new Date(Date.now() + offset * 864e5).toISOString().slice(0, 10);
  let tasks: TaskInfo[] = [
    { id: 1, title: "Review lecture notes", details: "Chapters 4–5", size: "small", schedule: { kind: "weekdays", days: [0, 1, 2, 3, 4] }, startDate: iso(-20), status: "active" },
    { id: 2, title: "Finish DSA practice", details: "2 problems from the sheet", size: "medium", schedule: { kind: "daily" }, startDate: iso(-30), status: "active" },
    { id: 3, title: "Read 20 pages", details: "", size: "small", schedule: { kind: "every_n_days", n: 2 }, startDate: iso(-10), status: "active" },
    { id: 4, title: "Gym", details: "Leg day", size: "large", schedule: { kind: "weekdays", days: [0, 2, 4] }, startDate: iso(-30), status: "active" },
    { id: 5, title: "Dentist", details: "4:30 pm", size: "small", schedule: { kind: "once" }, startDate: iso(2), status: "active" },
    { id: 6, title: "Guitar practice", details: "", size: "medium", schedule: { kind: "daily" }, startDate: iso(-40), status: "paused" },
  ];
  const results: DayInfo["result"][] = ["broken", "qualified", "qualified", "qualified", "revived", "qualified", "qualified", "neutral", "qualified", "qualified", "broken", "qualified", "qualified", "qualified"];
  let run = 0;
  const history: DayInfo[] = results
    .slice()
    .reverse()
    .map((r, i) => {
      const eligible = r === "neutral" ? 0 : 3 + (i % 3);
      const completed = r === "broken" ? 1 : r === "neutral" ? 0 : r === "revived" ? 1 : eligible - (i % 2);
      run = r === "broken" ? 0 : r === "neutral" || r === "revived" ? run : run + 1;
      return { date: iso(-(results.length - i)), eligible, completed, result: r, streakAfter: run };
    })
    .reverse();
  let streak = 0;
  let broken = true; // preview: yesterday broke a 6-day streak
  let freeAvailable = true;
  let bought = 0;
  let rewardId = 10;
  let rewards: Reward[] = [
    { id: 1, name: "Gaming session", note: "1 hour, no guilt", price: 120 },
    { id: 2, name: "Movie night", note: "", price: 250 },
    { id: 3, name: "Special meal", note: "Order from the good place", price: 400 },
  ];
  const redemptions: Redemption[] = [
    { id: 1, name: "Gaming session", price: 120, createdAt: new Date(Date.now() - 2 * 864e5).toISOString() },
  ];
  const ordered = () => [
    ...cards.filter((c) => c.status === "pending"),
    ...cards.filter((c) => c.status === "completed").sort((a, b) => (a.completedSeq ?? 0) - (b.completedSeq ?? 0)),
  ];
  const level = () => (xp >= 300 ? 3 : xp >= 100 ? 2 : 1);
  const state = (): AppState => ({
    today: new Date().toISOString().slice(0, 10),
    balance, xp, streak, level: level(),
    nextLevel: { level: level() + 1, xp: 700, streak: 14 },
    cards: structuredClone(ordered()),
    revives: {
      freeAvailable, bought, price: 100,
      brokenDate: broken ? new Date(Date.now() - 864e5).toISOString().slice(0, 10) : null,
      streakToRestore: broken ? 6 : 0,
    },
    rewards: structuredClone(rewards),
    redemptions: structuredClone(redemptions.slice(0, 5)),
    upcoming: [1, 2, 3, 4, 5, 6, 7].map((i) => ({
      date: new Date(Date.now() + i * 864e5).toISOString().slice(0, 10),
      items: [
        ...(i % 2 ? [{ taskId: 4, title: "Gym", details: "Leg day", size: "large" as Size }] : []),
        ...(i < 6 ? [{ taskId: 2, title: "Finish DSA practice", details: "2 problems from the sheet", size: "medium" as Size }] : []),
        ...(i === 2 ? [{ taskId: 9, title: "Dentist", details: "4:30 pm", size: "small" as Size }] : []),
      ],
    })),
    settings: structuredClone(settings),
  });
  const wait = (ms = 120) => new Promise((r) => setTimeout(r, ms));
  const changed = () => listeners.forEach((l) => l());
  const done = new Set<string>();
  return {
    async getState() { await wait(30); return state(); },
    async createTask(t) {
      await wait();
      const c = mk(t.title.trim(), t.details.trim(), t.size);
      cards.push(c);
      tasks.push({ id: c.taskId + 100, title: c.title, details: c.details, size: t.size, schedule: t.schedule, startDate: t.startDate ?? iso(0), status: "active" });
      changed(); return c.taskId;
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
    async buyRevive() {
      await wait();
      if (balance < 100) throw "A revive costs 100 coins. You don't have enough yet.";
      balance -= 100; bought++; changed(); return balance;
    },
    async useRevive() {
      await wait();
      if (!broken) throw "There's no broken streak to revive right now.";
      if (freeAvailable) freeAvailable = false;
      else if (bought > 0) bought--;
      else throw "You have no revives left. Buy one for 100 coins.";
      broken = false; streak = 6; changed(); return streak;
    },
    async createReward(r) { await wait(); rewards.push({ id: rewardId++, ...r, name: r.name.trim() }); rewards.sort((a, b) => a.price - b.price); changed(); return rewardId; },
    async updateReward(id, r) { await wait(); rewards = rewards.map((x) => (x.id === id ? { id, ...r } : x)); changed(); },
    async archiveReward(id) { await wait(); rewards = rewards.filter((x) => x.id !== id); changed(); },
    async redeemReward(_op, id) {
      await wait();
      const r = rewards.find((x) => x.id === id);
      if (!r) throw "Reward not found.";
      if (balance < r.price) throw `You need ${r.price - balance} more coins for this.`;
      balance -= r.price;
      redemptions.unshift({ id: Date.now(), name: r.name, price: r.price, createdAt: new Date().toISOString() });
      changed(); return balance;
    },
    async listTasks() { await wait(30); return structuredClone(tasks); },
    async updateTask(id, t) { await wait(); tasks = tasks.map((x) => (x.id === id ? { ...x, ...t, startDate: t.startDate ?? x.startDate } : x)); changed(); },
    async setTaskStatus(id, status) { await wait(); tasks = tasks.map((x) => (x.id === id ? { ...x, status } : x)); changed(); },
    async getHistory() { await wait(30); return structuredClone(history); },
    async getStats() {
      await wait(30);
      return {
        level: level(), xp, streak, bestStreak: 9, coinsEarned: 1180, coinsSpent: 940, tasksCompleted: 74, completion30d: 78,
        levels: [[1, 0, 0], [2, 100, 3], [3, 300, 7], [4, 700, 14], [5, 1500, 30], [6, 3000, 60], [7, 6000, 100]].map(([level, xp, streak]) => ({ level, xp, streak })),
      };
    },
    async updateSettings(s) { await wait(); settings = { ...s }; changed(); },
    async getAutostart() { return autostart; },
    async setAutostart(v) { await wait(); autostart = v; return autostart; },
    async openDashboard() { window.open("?view=dashboard", "_blank"); },
    onAddTask() { return () => {}; },
    onChanged(cb) { listeners.add(cb); return () => listeners.delete(cb); },
    hideWindow() { alert("In the app this hides the widget to the tray."); },
  };
}
