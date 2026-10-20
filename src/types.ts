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
}

export interface CompleteResult {
  coins: number;
  xp: number;
  balance: number;
  levelBefore: number;
  levelAfter: number;
}
