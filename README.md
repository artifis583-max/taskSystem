# Task Widget

A small Windows widget for daily tasks. Tick a task to earn coins and XP, keep a streak, and level up. Everything is stored on your PC and works offline.

Full spec: [docs/PLAN.md](docs/PLAN.md).

## Status

| Phase | State |
|---|---|
| 1. Widget shell (bottom-right, frameless, tray, close hides) | Done |
| 2. Tasks + completion + animation + undo | Done |
| 3. Days + rollover (weekday / every-N schedules, ×0.8 penalty, skip, remove) | Engine done; no "Upcoming" tab yet |
| 4. XP, streak, levels | Engine done (level bonus applies to coins); header shows level and streak |
| 5. Rewards shop + revives | Not started (tables exist) |
| 6. Dashboard, autostart, remember position | Not started |
| 7. Backup/restore, release checks | Not started |

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
| `src-tauri/src/db.rs` | SQLite schema and migrations |
| `src-tauri/src/lib.rs` | Window placement, tray, date watcher |
| `src/TaskList.tsx` | Task cards and the complete → swipe → DONE → move-to-bottom animation |
| `src/mockApi.ts` | Fake backend for browser preview only |

Data lives in `%APPDATA%\com.taskwidget.app\data.db`.

## How it behaves

- **Tick a task:** checkbox turns green, the card slides left and shows DONE, the coin count goes up, then the card moves to the bottom with a bounce.
- **Undo:** tap a green checkbox and confirm. Works until the day is closed, and only if you haven't spent the coins.
- **Click a card** to skip it for today or remove the task.
- **Missed tasks** roll over to the next day, worth ×0.8 coins per rollover (min 1).
- **Close (×)** hides the widget to the tray. Quit from the tray menu.
- Turning off Windows animations (Settings → Accessibility → Visual effects) turns off the motion.
