# Task Widget

A small Windows widget for daily tasks. Tick a task to earn coins and XP, keep a streak, and level up. Everything is stored on your PC and works offline.

Full spec: [docs/PLAN.md](docs/PLAN.md).

## Status

| Phase | State |
|---|---|
| 1. Widget shell (bottom-right, frameless, tray, close hides) | Done |
| 2. Tasks + completion + animation + undo | Done |
| 3. Days + rollover (weekday / every-N schedules, ×0.8 penalty, skip, remove) | Done |
| 4. XP, streak, levels | Engine done (level bonus applies to coins); header shows level and streak |
| 5. Rewards shop + revives | Done |
| 6. Dashboard, autostart, remember position | Done |
| 7. Backup/restore, release checks | Backups done. Windows installer build and memory check still need a Windows PC |

## Run it on Windows

Needs: [Node.js 20+](https://nodejs.org), [Rust](https://rustup.rs) (MSVC toolchain), and the
"Desktop development with C++" workload from Visual Studio Build Tools. WebView2 ships with Windows 11.

```powershell
npm install
npm run tauri dev      # run the app with live reload
npm run tauri build    # make installers in src-tauri/target/release/bundle/ (msi + nsis)
```

## Other commands

```bash
cd src-tauri && cargo test   # rules + database tests
npm run build                # type check + build the UI
npm run dev                  # preview the UI in a browser at http://localhost:1420 (fake data)
```

## Where things are

| Path | What |
|---|---|
| `src-tauri/src/config.rs` | All tunable numbers (rewards, rollover factor, level table) |
| `src-tauri/src/domain.rs` | Pure rules: schedules, coin formula, streak, level |
| `src-tauri/src/service.rs` | Each user action as one database transaction; day rollover |
| `src-tauri/src/shop.rs` | Reward shop and revives |
| `src-tauri/src/db.rs` | SQLite schema and migrations |
| `src-tauri/src/lib.rs` | Window placement, tray, date watcher |
| `src/TaskList.tsx` | Task cards and the complete → swipe → DONE → move-to-bottom animation |
| `src-tauri/src/manage.rs` | Editing/pausing tasks, upcoming days, history, stats |
| `src-tauri/src/settings.rs` | Settings and saved widget position |
| `src/Dashboard.tsx` | Dashboard window (Tasks, History, Progress, Settings) |
| `src-tauri/src/backup.rs` | Backups, rolling cleanup, validated restore |
| `src/mockApi.ts` | Fake backend for browser preview only (`?view=dashboard` shows the dashboard) |

Run the app with `--dashboard` to open the dashboard right away (also works on a running app).

Data lives in `%APPDATA%\com.taskwidget.app\data.db`. Backups are in the `backups` folder next to it.

## Backups

- **Automatic:** once a day, before any restore, and before a database upgrade. The newest 7 daily backups are kept.
- **Dashboard → Settings → Backups:** back up now, export a copy anywhere, restore from the list or from a file.
- **Restoring** checks the file first (integrity, version, tables). A bad file is rejected and your data is not touched. Your current data is always backed up before a restore.
- If the data file can't be opened (for example it was saved by a newer version), the app leaves it untouched and shows a message instead of starting fresh.

## Install

`npm run tauri build` makes two installers in `src-tauri/target/release/bundle/`:
- `nsis/Task Widget_1.0.0_x64-setup.exe` (recommended): installs for your user only, no admin needed.
- `msi/Task Widget_1.0.0_x64_en-US.msi`

## How it behaves

- **Tick a task:** checkbox turns green, the card slides left and shows DONE, the coin count goes up, then the card moves to the bottom with a bounce.
- **Undo:** tap a green checkbox and confirm. Works until the day is closed, and only if you haven't spent the coins.
- **Upcoming tab:** what's planned for the next 7 days. New tasks can start today, tomorrow, or on a date you pick.
- **Click a card** to skip it for today or remove the task.
- **Missed tasks** roll over to the next day, worth ×0.8 coins per rollover (min 1).
- **Rewards tab:** make your own rewards and spend coins on them (with a confirm step). Coins can never go below zero.
- **Revives:** if yesterday broke your streak, you can revive it today only. One free revive per week (refills Monday), extra ones cost 100 coins.
- **Dashboard** (grid button in the widget, or the tray): edit, pause and remove tasks; see day-by-day history and progress toward the next level; settings.
- **Settings:** start with Windows (off by default, no admin needed), keep the widget on top, theme (system/light/dark), animations (system/on/off).
- The widget remembers where you dragged it. If that spot is off-screen (for example a monitor was unplugged), it goes back to the bottom-right corner.
- **Tray menu:** Show widget, Open dashboard, Add task, Quit.
- **Close (×)** hides the widget to the tray. Quit from the tray menu.
- Light or dark theme follows your Windows setting.
- Turning off Windows animations (Settings → Accessibility → Visual effects) turns off the motion.
