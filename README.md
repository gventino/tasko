# tasko-fable

A Jira-style kanban task manager for your terminal, built with
[Ratatui](https://ratatui.rs). Multiple boards, subtasks, labels, priorities,
due dates, activity history, filtering and cross-board search — all keyboard
driven, all stored locally in SQLite.

```
 tasko-fable · Main Board [MB]
┏ To Do 3 ━━━━━━━━━━━━━━━━┓╭ In Progress 2 ─────────╮╭ Done 5 ────────────────╮
┃▶ MB-1 ⚑  ☰ 1/3          ┃│  MB-3 ▲                ││  MB-2 ◆                │
┃  Fix login redirect     ┃│  Cache board queries   ││  Write onboarding docs │
┃  ● ●  ⏱ Jun 07          ┃│                        ││                        │
┗━━━━━━━━━━━━━━━━━━━━━━━━━┛╰────────────────────────╯╰────────────────────────╯
 ←→↑↓ move · ⏎ open · n new · e edit · ⇧←→ move card · / filter · ? help
```

## Features

- **Multiple boards** with Jira-style task keys (`MB-42`), board switcher (`b`)
- **Columns** — create, rename, reorder, delete, per-column **WIP limits**
- **Tasks** — title, multi-line description, priority (Low→Urgent), due dates
  with overdue highlight, colored **labels**, **subtasks** (checklist with
  progress on the card), and a full **activity history**
- **Filter** the board live (`/`) with free text plus `p:<priority>` and
  `l:<label>` operators; **global search** across boards (`g`) with debounce
- Optimistic updates: every action lands instantly, persistence runs async

## Run

```sh
cargo run --release            # data in the platform data dir
TASKO_DB=/tmp/demo.db cargo run --release    # custom database file
cargo run --release -- --seed 100            # populate demo data
```

Press `?` inside the app for the full keymap.

## Design notes (resource usage)

- **Zero idle CPU** — no tick loop. The event loop `select!`s over the
  crossterm `EventStream` and an internal message channel, draining queued
  messages in a batch and rendering at most once per wake-up (dirty flag).
- **UI never blocks on I/O** — sqlx + tokio run every query off the UI loop;
  state updates optimistically and resyncs (with a toast) if a write fails.
- **O(1) card moves** — sparse `position` integers (gap 1024). Reorders swap
  two positions, cross-column moves append at `max+gap`: one `UPDATE` each,
  never a column rewrite.
- **Lean memory** — only the active board is held in memory (~16 MB RSS with
  5,000 tasks); rendering borrows from state, no clones on the hot path.
- **SQLite tuned** — WAL, `synchronous=NORMAL`, busy timeout, foreign keys,
  covering index on `(board_id, column_id, position)`, embedded migrations.

## Development

```sh
cargo test          # domain + repository (in-memory SQLite) + TestBackend UI tests
cargo clippy --all-targets
```

Architecture: Elm-style message loop (`src/app.rs`), pure render
(`src/ui/`), repositories (`src/db/`), domain types and position math
(`src/domain/`), key→message mapping per context (`src/input.rs`).
