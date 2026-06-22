# tasko

A Jira-style kanban task manager for your terminal, built with
[Ratatui](https://ratatui.rs). Multiple boards, subtasks, labels, priorities,
due dates, activity history, filtering and cross-board search — all keyboard
driven, all stored locally in SQLite.

```
 tasko · Main Board [MB]
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

## Install

One-line install (clones the repo, builds the optimized release binary and
installs it to `~/.cargo/bin/tasko`):

```sh
curl -fsSL https://raw.githubusercontent.com/gventino/tasko/main/install.sh | bash
```

The script installs Rust via [rustup](https://rustup.rs) if it is missing
(requires Rust ≥ 1.88). Make sure `~/.cargo/bin` is on your `PATH`, then run
`tasko`.

Already cloned the repo? Just run `./install.sh` from inside it.

## Run

```sh
tasko                                        # data in the platform data dir
TASKO_DB=/tmp/demo.db tasko                   # custom database file
tasko --seed 100                             # populate demo data
```

Or run straight from a checkout without installing:

```sh
cargo run --release
```

Press `?` inside the app for the full keymap.

## API mode

Run tasko as a local HTTP REST server instead of the TUI — handy for
importing/exporting data, scripting, and integrating with other tools. It
serves JSON CRUD endpoints for every entity plus an interactive Swagger UI, all
backed by the same SQLite database.

```sh
tasko serve                  # bind 127.0.0.1:8080
tasko serve --port 9000      # custom port (or set TASKO_API_PORT)
```

Open <http://127.0.0.1:8080/swagger-ui> for the interactive reference (the raw
spec lives at `/openapi.json`). The server binds to localhost only and has **no
authentication**, so don't expose it directly to a public network.

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/health` | Liveness probe |
| `GET` `POST` | `/boards` | List / create boards |
| `GET` `PATCH` `DELETE` | `/boards/:id` | Read / rename / delete a board |
| `GET` `POST` | `/boards/:id/columns` | List / create columns |
| `GET` `PATCH` `DELETE` | `/columns/:id` | Read / update (name, WIP limit) / delete |
| `GET` `POST` | `/boards/:id/tasks` | List (`?top_level=`, `?parent_id=`) / create tasks |
| `GET` `PATCH` `DELETE` | `/tasks/:id` | Read / patch content, status or position / delete |
| `GET` | `/tasks/:id/subtasks` | List a task's subtasks |
| `GET` `PUT` | `/tasks/:id/labels` | List / replace the task's label set |
| `GET` | `/tasks/:id/activities` | Read a task's activity history (read-only) |
| `GET` `POST` | `/boards/:id/labels` | List / create labels |
| `GET` `PATCH` `DELETE` | `/labels/:id` | Read / update / delete a label |

Successful `POST` create endpoints return `201 Created` and a `Location` header
for the new resource (for example, `Location: /tasks/42`). Error responses are
always JSON: `{ "error": "<message>" }`, with `400` for invalid input (including
bad JSON bodies and bad path params), `404` for not found, `409` for conflicts
and `500` for internal errors. Internal error details are logged server-side;
clients receive a generic message.

`PATCH /tasks/:id` applies content, status and move changes in one transaction:
all requested changes succeed or none do. Moves are constrained to the task's own
board; cross-board moves are rejected with `400`. `PUT /tasks/:id/labels`
replaces the full label set and de-duplicates `label_ids`.

```sh
curl -i -s localhost:8080/boards -H 'content-type: application/json' \
  -d '{"name":"Main Board"}'
curl -i -s localhost:8080/boards/1/tasks -H 'content-type: application/json' \
  -d '{"column_id":1,"title":"First task","priority":"high"}'
```

Notes: `DELETE` returns `404` when the resource does not exist (not idempotent,
for clearer feedback). List response pagination and API versioning (for example,
a `/v1` prefix) are not implemented yet.

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

Common tasks are wrapped in a `Makefile` — run `make` (or `make help`) to list
them, e.g. `make build`, `make install`, `make test`, `make ci`,
`make seed N=500`, `make run DB=/tmp/demo.db`, `make serve PORT=9000`.

Architecture: Elm-style message loop (`src/app.rs`), pure render
(`src/ui/`), repositories (`src/db/`), domain types and position math
(`src/domain/`), key→message mapping per context (`src/input.rs`), HTTP REST
API (`src/api/`).
