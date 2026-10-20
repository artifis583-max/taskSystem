// Mirrors the Rust types in src-tauri/src/service.rs and domain.rs.

export type Size = "small" | "medium" | "large";

export type Schedule =
  | { kind: "once" }
  | { kind: "daily" }
  | { kind: "weekdays"; days: number[] } // 0 = Monday
  | { kind: "every_n_days"; n: number };

export interface NewTask {
  title: string;
  details: string;
  size: Size;
  schedule: Schedule;
  startDate?: string;
}

export interface TaskCard {
  occurrenceId: number;
  taskId: number;
  title: string;
  details: string;
  size: Size;
  status: "pending" | "completed";
  rolloverCount: number;
  coins: number;
  xp: number;
  completedSeq: number | null;
}

export interface AppState {
  today: string;
  balance: number;
  xp: number;
  streak: number;
  level: number;
  nextLevel: { level: number; xp: number; streak: number } | null;
  cards: TaskCard[];
  revives: ReviveInfo;
  rewards: Reward[];
  /** Most recent first. */
  redemptions: Redemption[];
  upcoming: UpcomingDay[];
  settings: Settings;
}

export interface UpcomingDay {
  date: string;
  items: { taskId: number; title: string; details: string; size: Size }[];
}

export interface Settings {
  alwaysOnTop: boolean;
  theme: "system" | "light" | "dark";
  motion: "system" | "reduced" | "full";
}

export interface TaskInfo {
  id: number;
  title: string;
  details: string;
  size: Size;
  schedule: Schedule;
  startDate: string;
  status: "active" | "paused";
}

export interface DayInfo {
  date: string;
  eligible: number;
  completed: number;
  result: "qualified" | "neutral" | "broken" | "revived";
  streakAfter: number;
}

export interface BackupFile {
  name: string;
  size: number;
  createdAt: string;
  kind: string;
}

export interface BackupsInfo {
  dataDir: string;
  backupsDir: string;
  backups: BackupFile[];
}

export interface Stats {
  level: number;
  xp: number;
  streak: number;
  bestStreak: number;
  coinsEarned: number;
  coinsSpent: number;
  tasksCompleted: number;
  completion30d: number | null;
  levels: { level: number; xp: number; streak: number }[];
}

export interface ReviveInfo {
  freeAvailable: boolean;
  bought: number;
  price: number;
  /** Set when yesterday broke a streak that can still be revived today. */
  brokenDate: string | null;
  streakToRestore: number;
}

export interface Reward {
  id: number;
  name: string;
  note: string;
  price: number;
}

export interface RewardInput {
  name: string;
  note: string;
  price: number;
}

export interface Redemption {
  id: number;
  name: string;
  price: number;
  createdAt: string;
}

export interface CompleteResult {
  coins: number;
  xp: number;
  balance: number;
  levelBefore: number;
  levelAfter: number;
}
