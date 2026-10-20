import type { AppState, BackupsInfo, CompleteResult, DayInfo, NewTask, RewardInput, Settings, Stats, TaskInfo } from "./types";

export interface Api {
  getState(): Promise<AppState>;
  createTask(task: NewTask): Promise<number>;
  complete(opId: string, occurrenceId: number): Promise<CompleteResult>;
  uncomplete(opId: string, occurrenceId: number): Promise<number>;
  skip(occurrenceId: number): Promise<void>;
  removeTask(taskId: number): Promise<void>;
  /** Returns the new balance. */
  buyRevive(opId: string): Promise<number>;
  /** Returns the restored streak. */
  useRevive(): Promise<number>;
  createReward(reward: RewardInput): Promise<number>;
  updateReward(id: number, reward: RewardInput): Promise<void>;
  archiveReward(id: number): Promise<void>;
  /** Returns the new balance. */
  redeemReward(opId: string, id: number): Promise<number>;
  listTasks(): Promise<TaskInfo[]>;
  updateTask(id: number, task: NewTask): Promise<void>;
  setTaskStatus(id: number, status: "active" | "paused"): Promise<void>;
  getHistory(): Promise<DayInfo[]>;
  getStats(): Promise<Stats>;
  updateSettings(settings: Settings): Promise<void>;
  getAutostart(): Promise<boolean>;
  /** Returns the actual state after the change. */
  setAutostart(enabled: boolean): Promise<boolean>;
  openDashboard(): Promise<void>;
  backupsInfo(): Promise<BackupsInfo>;
  /** Returns the new backup's file name. */
  backupNow(): Promise<string>;
  /** Opens a save dialog. Returns false if cancelled. */
  exportBackup(): Promise<boolean>;
  restoreBackup(name: string): Promise<void>;
  /** Opens a file picker. Returns false if cancelled. */
  restoreFromFile(): Promise<boolean>;
  /** Tray "Add task" was clicked. */
  onAddTask(cb: () => void): () => void;
  /** Called when data changed (any window) or the date rolled over. */
  onChanged(cb: () => void): () => void;
  hideWindow(): void;
}

export const inTauri = "__TAURI_INTERNALS__" in window;

async function tauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return {
    getState: () => invoke("get_state"),
    createTask: (task) => invoke("create_task", { task }),
    complete: (opId, occurrenceId) => invoke("complete_occurrence", { opId, occurrenceId }),
    uncomplete: (opId, occurrenceId) => invoke("uncomplete_occurrence", { opId, occurrenceId }),
    skip: (occurrenceId) => invoke("skip_occurrence", { occurrenceId }),
    removeTask: (taskId) => invoke("remove_task", { taskId }),
    buyRevive: (opId) => invoke("buy_revive", { opId }),
    useRevive: () => invoke("use_revive"),
    createReward: (reward) => invoke("create_reward", { reward }),
    updateReward: (id, reward) => invoke("update_reward", { id, reward }),
    archiveReward: (id) => invoke("archive_reward", { id }),
    redeemReward: (opId, id) => invoke("redeem_reward", { opId, id }),
    listTasks: () => invoke("list_tasks"),
    updateTask: (id, task) => invoke("update_task", { id, task }),
    setTaskStatus: (id, status) => invoke("set_task_status", { id, status }),
    getHistory: () => invoke("get_history"),
    getStats: () => invoke("get_stats"),
    updateSettings: (settings) => invoke("update_settings", { settings }),
    getAutostart: () => invoke("get_autostart"),
    setAutostart: (enabled) => invoke("set_autostart", { enabled }),
    openDashboard: () => invoke("open_dashboard"),
    backupsInfo: () => invoke("backups_info"),
    backupNow: () => invoke("backup_now"),
    exportBackup: () => invoke("export_backup"),
    restoreBackup: (name) => invoke("restore_backup", { name }),
    restoreFromFile: () => invoke("restore_from_file"),
    onAddTask(cb) {
      const un = listen("open-add-task", () => cb());
      return () => void un.then((f) => f());
    },
    onChanged(cb) {
      const un = listen("state-changed", () => cb());
      return () => void un.then((f) => f());
    },
    hideWindow: () => void getCurrentWindow().hide(),
  };
}

/** Real backend inside Tauri; an in-memory stand-in in a plain browser (UI preview only). */
export const loadApi = (): Promise<Api> =>
  inTauri ? tauriApi() : import("./mockApi").then((m) => m.createMockApi());

export const newOpId = () => crypto.randomUUID();
