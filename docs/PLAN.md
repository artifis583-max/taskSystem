# Task Widget — Build Plan v2

Windows 10/11 desktop widget · Tauri 2 + React + TypeScript + Rust (rusqlite) · offline, local-only

> **For the coding agent:** this file is the source of truth. Build it in the phases in §9.
> Each phase must end with an app you can run and use. Inspect any existing repo first and keep what works.
> Report the commands you ran and their real results. Never claim a check passed if you didn't run it.

---

## 0. What changed from v1 (and why)

| v1 plan | v2 plan | Why |
|---|---|---|
| UI built in phase 4, after all the engines | Widget UI built **first**, engines added slice by slice | You can see and use the app from week 1; bugs in the feel show up early |
| Completed card swipes away and disappears | Card swipes left → shows **DONE** → moves to the **bottom** of the list and stays, with a green check (per mockups) | Matches the mockups; keeps finished work visible |
| 5-second undo toast | Tap the green check again to un-complete (until the day is finalized) | Simpler, no timer, and fits the "card stays visible" design |
| Business logic in TS services + DB behind commands | All rules **and** all writes live in Rust; each user action = one Rust command = one SQL transaction | One place for rules, real transactions, no way for UI to corrupt data |
| 11 tables, separate coin/XP ledgers, cached balance option | 8 tables; one `ledger` table for coins and XP; balance is always `SUM()` | Less to keep in sync |
| Two conflicting rollover descriptions (−20% flat vs ×0.8 compounding) | One rule: ×0.8 per rollover (compounding) | The v1 text and formula disagreed |
| Level formula written as `B × 1.15L − 1` | `B × 1.15^(L−1)` | Typo fix; Level 1 = no bonus |
| Everything in v1 scope (reminders, calendar, selected dates, shortcuts, notifications…) | Core first; extras moved to §10 "Later" | Ship a solid core before adding surface area |

---

## 1. Product in one paragraph

A small always-available widget in the bottom-right corner of the screen that shows today's tasks. You tick a task, it slides left, says **DONE**, bounces down to the bottom of the list, and you earn coins and XP. Doing at least half of each day's tasks builds a streak. Your level needs both XP and a streak. Missed tasks roll over to the next day and are worth less each time. Coins buy rewards you define yourself (a movie, a gaming session). Everything is stored on your PC and works offline.

---

## 2. Widget UI (from the mockups)

### 2.1 Layout

```
┌──────────────────────────────────────┐  white panel, radius 16, soft shadow
│                    [COINS][LEVEL] (x)│  header: yellow pills + round close button
│ [ tab1 ]   [ tab2 ]   [ tab3 ]       │  3 equal-width gray tabs
│ ┌──────────────────────────────┐     │
│ │ TASK 2                   [ ] │     │  pending tasks first, in user order
│ │ DETAILS                      │     │
│ └──────────────────────────────┘     │
│ ┌──────────────────────────────┐     │
│ │ TASK 1                   [■] │     │  completed tasks last, green check
│ │ DETAILS                      │     │
│ └──────────────────────────────┘     │
└──────────────────────────────────────┘
```

- Window: frameless, transparent background, ~455 × 500 px at 100% scaling, anchored to the bottom-right of the monitor **work area** (above the taskbar) with ~6 px margin. Drag by the header. Remember position; if it ends up off-screen (monitor unplugged), snap back to bottom-right.
- Header pills show live values: `🪙 240` and `LV 3`. Clicking the level pill shows streak + XP-to-next-level.
- Close (x) hides the widget to the tray. It does not quit.
- Task card: title (bold), details line (smaller), checkbox on the right. If a task has rolled over, show a small `↻2` badge and its current coin value.
- List scrolls when it overflows; header and tabs stay fixed.

### 2.2 Tabs

The mockups show three placeholder tabs. Default meaning (change in config if wanted):

| Tab | Shows |
|---|---|
| **Today** | Today's tasks + carried-over tasks. Add-task button at the bottom. |
| **Upcoming** | Next 7 days, read-only, grouped by date. |
| **Rewards** | Reward shop, revive count, buy/use revive. |

### 2.3 Visual design

The mockups set the **layout and motion only**. Their gray cards, yellow pills and display font were wireframe styling and are not the app's theme.

Theme: clean Windows 11 style, light by default, dark when Windows is in dark mode. All colors are CSS variables in `src/styles.css` (`:root` for light, `prefers-color-scheme: dark` for dark).

| Token | Light | Dark |
|---|---|---|
| Panel surface | `#F6F6F7` | `#202024` |
| Card | `#FFFFFF` + 1px hairline border | `#2B2B30` |
| Text / muted | `#1B1B1F` / `#6B6B73` | `#F2F2F4` / `#A4A4AD` |
| Controls (tabs track, chips) | `#EBEBEE` | `#2B2B30` |
| Accent (done, progress) | `#16A34A` | `#3FB950` |
| Coin | `#F2B01E` | same |
| Font | Segoe UI Variable → Segoe UI → system-ui (no bundled font) | |
| Radius | panel 12, card 10, controls 8 | |

Header: weekday + date on the left; coin balance, level/streak chip and hide button on the right. Tabs are a segmented control with a count of unfinished tasks on Today. A thin progress bar shows "n/m done" above the list. Completed cards show the title struck through with a green check.

---|---|
| Panel bg | `#FFFFFF` |
| Card / tab bg | `#666666` |
| Card text | `#FFFFFF` |
| Checkbox empty | `#FFFFFF`, radius 6 |
| Success green | `#16A316` (checkbox fill), `#0B6B0B` (DONE text) |
| Pill bg | `#F5B400`, text white, radius 999 |
| Card radius | 12 px; gap between cards 10 px |
| Font | A rounded, heavy display font bundled locally (e.g. *Lilita One* or *Bungee*, OFL licensed). Titles uppercase. No web font fetches. |

Light theme only in v1. Respect Windows display scaling.

---

## 3. Completion animation (exact spec)

Uses the FLIP technique (measure → reorder DOM → invert with transform → play). Only animate `transform` and `opacity`. Bounce easing: `cubic-bezier(0.34, 1.56, 0.64, 1)`.

| # | Step | What the user sees | Time |
|---|---|---|---|
| 1 | **Tick** | Checkbox fills green with a check. The `complete_occurrence` command is called right away. | 100 ms |
| 2 | **Swipe left + bounce** | Card slides left ~33% of its width (as in the mockup); its text fades with a small overshoot, uncovering a green **DONE** label on the right. | 300 ms |
| 3 | **Hold** | Card stays in the DONE state so the user sees it. Coin pill counts up to the new balance and pulses once. | 350 ms |
| 4 | **Move to bottom** | Card slides back to full width and moves to the last position. Cards below slide up to fill the gap. | 350 ms |
| 5 | **Land + bounce** | Card lands at the bottom with a small vertical bounce (overshoot ~6 px, settle). | 250 ms |

Rules:
- The coin pill only updates after the command succeeds. If the command fails, reverse the checkbox and show a short error line; no animation.
- Newest completed task goes to the very bottom.
- Ticking several tasks quickly: each gets its own steps 1–3 right away; the reorders (steps 4–5) are queued and run one after another so cards never cross over each other.
- If the window hides or closes mid-animation, nothing replays on next open. The list just renders in its final order.
- Level-up or streak milestones: after step 5, the level pill flashes and a one-line banner shows for 2 s.
- **Reduced motion** (setting, and default when Windows "Show animations" is off): checkbox turns green and the card jumps to the bottom. No sliding or bouncing.
- **Un-complete:** tap a green checkbox → confirm "Undo this task? −24 coins, −20 XP" → card animates back to its original slot (reverse of step 4). Refused if the day is already finalized, or if coins already spent make the balance go below zero (tell the user why).

---

## 4. Rules

All numbers below live in one config file (`src-tauri/src/config.rs`) and are not hard-coded anywhere else.

### 4.1 Tasks and occurrences

- **Task** = the template: title (1–120 chars), details (optional), category, size (S/M/L), optional coin/XP override, schedule, status (`active` / `paused` / `archived`).
- **Occurrence** = one dated instance of a task: `original_date`, `due_date`, `status`, `rollover_count`, and on completion the awarded coins/XP.
- v1 schedules: **one-time**, **daily**, **selected weekdays**, **every N days**. Each has a start date and optional end date.
- No times of day. A task is due for the whole local calendar day.
- Editing a task's schedule affects future occurrences only. Paused/archived tasks stop generating new occurrences; existing open ones stay until resolved.
- Occurrence statuses: `pending`, `completed`, `skipped`, `cancelled`. (Rollover is tracked by `rollover_count > 0`, not a separate status.)

### 4.2 Rollover

- When a day ends, each `pending` occurrence from that day moves its `due_date` to the next day and `rollover_count += 1`. Same row, same id. No copies.
- A recurring task never has two open occurrences. If a carried-over one exists, today's new one is not generated.
- Coin value of a rolled-over task: `coins = max(1, round(level_coins × 0.8^rollover_count))`. XP is not reduced.
- Rolled-over tasks always come back to "today" (v1 has no "next eligible weekday" logic, because it's simpler and keeps missed work visible).
- Skip = "I'm not doing this": counts as incomplete for the streak, no reward, removed from the list. Cancel = "this no longer applies": excluded from the streak math.

### 4.3 Coins and XP

| Size | Coins | XP |
|---|---|---|
| Small | 10 | 10 |
| Medium | 20 | 20 |
| Large | 30 | 30 |

- Coin formula at completion: `level_coins = round(base × 1.15^(level − 1))`, then apply the rollover factor from 4.2, then clamp to ≥ 1.
- XP formula: base XP, no multipliers.
- The awarded values are stored on the occurrence and in the ledger. Changing config later never changes past rewards.
- Coin balance = `SUM(ledger.coins)`. It can never go below 0; any spend that would do that is refused inside the transaction.

### 4.4 Streak

- A day **qualifies** if `completed / eligible ≥ 50%`. Eligible = occurrences due that day that were not cancelled (skipped ones count as eligible).
- A day with 0 eligible tasks is **neutral**: streak neither grows nor breaks.
- Below 50% = **break**: current streak resets to 0.
- A carried-over task counts as eligible on each day it is due. (Missing a task twice hurts twice. This is intended.)
- Days are **finalized** when the app first sees a later date (on launch, on wake from sleep, or at the midnight timer). Finalizing is done once per date; a `days` row with `finalized_at` prevents doing it twice.

### 4.5 Levels

| Level | Lifetime XP | Current streak |
|---|---|---|
| 1 | 0 | 0 |
| 2 | 100 | 3 |
| 3 | 300 | 7 |
| 4 | 700 | 14 |
| 5 | 1,500 | 30 |
| 6 | 3,000 | 60 |
| 7 | 6,000 | 100 |

Active level = highest row where both columns are met. Level is computed on the fly from XP and streak; it is not stored. A streak break drops you to Level 1; XP and coins are kept.

### 4.6 Revives

- 1 free revive per week, refilled Monday 00:00 local time. Free revives don't stack.
- Extra revives can be bought for 100 coins and do stack.
- A revive can be used only on the **most recent** break, and only until the end of the day after that break (after that, the window closes and it can't be used).
- Using a revive marks the break day as `revived` (treated as neutral) and restores the streak value saved at the break. It gives no XP or coins.
- Always ask for confirmation. Free revive is used before purchased ones.

### 4.7 Reward shop

- User creates rewards: name, optional note, price in coins. Rewards can be archived, never deleted.
- Redeem → confirm → one transaction: check balance, write ledger row, write redemption row.
- Coins have no cash value. No real-money purchases.

---

## 5. Architecture

```
┌──────────── React (TypeScript) ────────────┐
│ Widget window          Dashboard window    │   display + input only
│        └──── invoke("command", args) ──────┤
└───────────────────────┬────────────────────┘
                        │  typed commands
┌──────────── Rust core ┴────────────────────┐
│ commands.rs   one fn per user action       │
│ domain/       pure rules: schedule, reward │   unit tested, no I/O
│               streak, level, revive        │
│ db/           rusqlite, migrations         │   each command = 1 transaction
│ day_clock.rs  finalize dates on launch,    │
│               wake, and midnight timer     │
└────────────────────────────────────────────┘
          SQLite in %APPDATA%\<app>\data.db
```

- After any write, Rust emits a `state-changed` event; both windows re-fetch `get_state()`. That's how widget and dashboard stay in sync. No separate client-side copies of the rules.
- Every write command takes an `op_id` (UUID made by the UI). A unique index on `ledger.op_id` makes double-clicks harmless.
- Tauri plugins used: `window-state`, `autostart`, `single-instance`, tray (core). Capabilities are restricted to the app's own commands. No shell, no fs, no http plugins. Strict CSP.
- Time: timestamps stored as UTC; dates stored as `YYYY-MM-DD` local dates plus the time-zone id at the time.
- Midnight: one `setTimeout`-style timer in Rust set for the next local midnight, re-armed after each fire and after wake from sleep. No polling loop.

### 5.1 Commands

```
get_state(date)                 -> { tasks, occurrences, balance, xp, streak, level, revives, rewards }
create_task / update_task / set_task_status
complete_occurrence(op_id, id)  -> { coins, xp, new_balance, level_up, streak_milestone }
uncomplete_occurrence(op_id, id)
skip_occurrence / cancel_occurrence
buy_revive(op_id) / use_revive(op_id)
create_reward / update_reward / archive_reward / redeem_reward(op_id, id)
get_settings / update_settings
export_backup(path) / import_backup(path)
```

### 5.2 Tables

| Table | Key columns |
|---|---|
| `tasks` | id, title, details, category, size, coin_override, xp_override, schedule_json, start_date, end_date, status, sort_order |
| `occurrences` | id, task_id, original_date, due_date, status, rollover_count, completed_at, awarded_coins, awarded_xp, completed_seq |
| `ledger` | id, op_id (unique), kind (`task`, `undo`, `revive_buy`, `redeem`, `correction`), coins, xp, occurrence_id, created_at |
| `days` | date (pk), eligible, completed, result (`qualified`/`neutral`/`broken`/`revived`), streak_after, finalized_at |
| `revives` | id, source (`free`/`bought`), week_of, used_for_date, used_at |
| `rewards` | id, name, note, price, archived |
| `redemptions` | id, reward_id, price_paid, ledger_id, created_at |
| `settings` | key, value |

Plus `PRAGMA user_version` for migrations. Ledger rows are never updated or deleted; corrections are new rows.

---

## 6. Day processing (the important bit)

Run `finalize_through(today)` on launch, on resume from sleep, at the midnight timer, and before any write command:

```
last = MAX(date) FROM days WHERE finalized_at IS NOT NULL   (or install date)
for d in (last + 1) .. (today − 1), in order:
    BEGIN
      generate occurrences for d that don't exist yet (respect no-overlap rule)
      count eligible / completed for d → result, streak_after
      move d's pending occurrences to d+1, rollover_count += 1
      insert days(d, ..., finalized_at = now)
    COMMIT
generate today's occurrences
```

- If the clock goes **backwards** (today < last finalized date): do nothing destructive. Show today's list for the current date, but don't finalize or generate anything for already-finalized dates. Log it.
- If the app was closed for 10 days, this loop runs 10 times. Each day is its own transaction, so a crash mid-way resumes cleanly.

---

## 7. Windows behavior

- Close button (x): hide to tray. Tray menu: Show widget, Open dashboard, Add task, Quit.
- Launch at sign-in: off by default, toggle in settings, per-user (no admin).
- Single instance: launching again just shows the widget.
- Optional "always on top" toggle in settings.

---

## 8. Dashboard (separate window, opened from tray or widget)

Keep it simple in v1: **Tasks** (full create/edit with schedule), **History** (list of past days with result and streak), **Progress** (XP, level table, current/best streak, coins earned/spent), **Settings** (startup, always-on-top, reduced motion, backup/restore).

---

## 9. Build phases

Each phase ends with a runnable app and passing tests.

| Phase | Build | Done when |
|---|---|---|
| **1. Widget shell** | Tauri 2 + React project. Frameless transparent widget at bottom-right. Static UI matching §2 with fake data. Tray with Show/Quit. Close hides. | Looks like the mockup; runs with `npm run tauri dev`; packaged `.msi` builds. |
| **2. Real tasks + completion** | SQLite + migrations. Create one-time and daily tasks. `get_state`, `complete_occurrence`, `uncomplete_occurrence`. Coins in ledger. Full animation from §3 incl. reduced motion. | Tick a task → animation → balance goes up → restart app → still there. Double-click never pays twice (test). |
| **3. Days + rollover** | Weekday and every-N-days schedules. `finalize_through`, midnight timer, wake handling, rollover with ×0.8. Skip/cancel. | Unit tests for every row in §11. Manually changing the PC date forward 3 days rolls tasks correctly. |
| **4. XP, streak, level** | XP in ledger, `days` results, streak, level calc, level-multiplied coins, level/streak feedback in header. | Tests: 49% breaks, 50% qualifies, 0 tasks neutral, level needs both XP and streak. |
| **5. Rewards + revives** | Rewards tab, redeem, buy/use revive, weekly free revive. | Tests: no negative balance, revive window, Monday refill, one revive per break. |
| **6. Dashboard + settings** | Dashboard window and pages from §8. Autostart, always-on-top, window-state, single instance. | Widget and dashboard stay in sync when both are open. |
| **7. Backup + release** | Export/import with validation (schema version + integrity check before replacing). Auto backup before migrations; keep last 7. README. Measure idle RAM/CPU. | Corrupt import is rejected and live data is untouched. Report real RAM numbers (target < 100 MB idle). |

---

## 10. Later (not in v1)

Monthly / selected-dates / custom schedules · reminders and notifications · global show/hide hotkey · calendar view · dark theme · categories filter · coin-fly particle animation · drag to reorder tasks.

---

## 11. Required tests

**Rust unit tests (domain):**
- Small task, Level 1, no rollover → 10 coins, 10 XP.
- Medium task, Level 3 → `round(20 × 1.15²)` = 26 coins.
- Medium task, 2 rollovers, Level 1 → `round(20 × 0.64)` = 13 coins. 10 rollovers on a Small task → 1 coin (clamp).
- 1 of 2 done = qualifies; 0 of 1 done = break; 0 eligible = neutral; skipped counts as eligible; cancelled doesn't.
- Level: 1,500 XP with 5-day streak → Level 2. 100 XP with 40-day streak → Level 2.
- Revive: usable on break day + 1, refused on break day + 2, refused twice for the same break. Free revive refills on Monday, not on Sunday.
- No-overlap: daily task missed Monday → Tuesday has exactly one open occurrence for it.

**Rust integration tests (real SQLite file):**
- Same `op_id` sent twice → one ledger row.
- App "closed" 5 days → `finalize_through` produces 5 `days` rows; running it again changes nothing.
- Clock moved back 2 days → no new occurrences, no new ledger rows.
- Redeem with too few coins → refused, balance unchanged.
- Un-complete after spending the coins → refused, nothing changed.
- Failed import → original DB file byte-identical.

**Manual checklist on a packaged build:** startup at sign-in, hide/show from tray, sleep over midnight then wake, unplug second monitor, reduced-motion toggle, idle RAM/CPU readout.

---

## 12. Open decisions (defaults chosen; change if you disagree)

1. **Tabs** are Today / Upcoming / Rewards.
2. **Rollover** always lands on the next day, even for weekday-only tasks.
3. **Un-complete** replaces the 5-second undo toast.
4. **Revive window** is the day after a break only.
5. **Newest completed task** goes to the very bottom (not top of the completed group).
