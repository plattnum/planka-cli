# TUI tree view — the as-delivered design

This is a reference for `plnk-tui`'s explorer layout and data model as it ships today. For usage docs see [overview.md](overview.md), [keybindings.md](keybindings.md), and [live-target.md](live-target.md).

## Product intent

`plnk-tui` is a **project explorer**, not a terminal kanban board. Planka's web UI already does kanban well. The TUI focuses on hierarchy, detail, and fast text-heavy workflows:

- browse the Planka hierarchy quickly
- inspect card detail without leaving the keyboard
- edit card title inline and description in `$EDITOR`
- stay loosely in sync with the server via a single-board live subscription

Scope is intentionally narrow. Drag-and-drop, permissions management, attachments upload, bulk operations — all still live in the browser.

## Layout

Three horizontal regions, laid out top to bottom:

1. **Session header** (4 rows, rounded border, titled `session`)
2. **Body** — two-column split, ~44% left / 56% right
3. **Keys footer** (1 row, dim gray, no border)

The body splits further:

```
┌─ session ─────────────────────────────────────────────────┐
│ plnk-tui explorer  •  <status>                            │
│ server: <url> | login: <user> | current user: <name> …    │
│ visible projects: N | current user id: … | explorer view… │
└───────────────────────────────────────────────────────────┘
┌─ explorer • <view> ─────────┐ ┌─ details ────────────────┐
│                             │ │                          │
│   (tree rows)               │ │  (selected node detail)  │
│                             │ │                          │
│                             │ ├─ live sync ──────────────┤
│                             │ │  websocket: <state>      │
│                             │ │  live target: …          │
│                             │ │  latest event: …         │
│                             │ │  notice: …               │
└─────────────────────────────┘ └──────────────────────────┘
↑/↓ nav • / filter • →/Enter expand • r refresh • v toggle view • L live on/off • …
```

### Session header

- Row 1: the word `plnk-tui explorer`, a bullet, then one of a small set of status chips: `READ-ONLY`, `DIRTY`, `REMOTE CHANGED`, `SAVING`, or the websocket connection label.
- Row 2: `server: <url> | login: <user> | current user: <name> (<username>)` — identifies who is connected to where.
- Row 3: visible project count, current user id, explorer view mode, current filter text (`none` when inactive), and the live target board id (or `none (press L on a board)` when idle).

### Explorer pane

Renders a collapsible tree in one of two views, toggled with `v`:

- **hierarchy** — project → board → list → card. This is the default.
- **labels** — project → board → list → label group → card. Groups cards by the labels applied to them on a given list.

Press `/` to enter a client-side filter mode for the current explorer view. Plain text uses case-insensitive substring matching; queries containing `*` or `?` use case-insensitive glob matching. Matching descendants keep their ancestor context visible.

See [data model](#data-model) below for the underlying types.

### Details pane

Shows the selected node's detail. For cards (the rich case), this includes:

- title + status chips (`ACTIVE` / `CLOSED`, due date summary, subscription state)
- card id (dim, copyable)
- `— Context —` list, board, project breadcrumb with IDs
- `— Metadata —` creator, labels, assignees, comment count
- `— Description —` the card body, rendered as plain text
- `— Tasks —` when present, a checklist
- `— Comments —` when loaded, a scrollable thread

Projects, boards, and lists get a narrower pane showing their own metadata and counts.

### Live-sync pane

A fixed 7-row block beneath details:

- `websocket: <state>` — one of `no live target`, `loading`, `connecting raw websocket`, `live websocket connected`, or `error: <reason>`
- `live target: <board>` — the currently subscribed board, or `none — select a board and press L to promote it` when idle
- project rows may temporarily show `refreshing hierarchy…` while a manual refresh is in flight
- when a filter is active, the explorer title and session header show the active query
- `latest event: <name>` — short summary of the last `socket.io` event applied
- `notice: <message>` — transient status messages (save progress, edit outcomes)

### Keys footer

A single dim-gray line with the most relevant keybindings for the current mode. Mode-aware:

- Default: navigation, filter, manual refresh, view toggle, live on/off, edit, debug, quit.
- Title edit mode: the title-editing key set.
- Saving mode: controls paused until the server responds.

## Data model

Every tree row carries a `TreeKey` identifying the node and a `TreeKind` marking its kind:

```text
TreeKey::Project(String)
TreeKey::Board(String)
TreeKey::List(String)
TreeKey::LabelGroup { board_id, list_id, label_id: Option<String> }
TreeKey::Card(String)
TreeKey::GroupedCard { group_key, card_id }
```

`ExplorerView::Hierarchy` uses Project/Board/List/Card keys. `ExplorerView::Labels` uses Project/Board/List/LabelGroup/GroupedCard keys. The explorer renders the subset of rows whose ancestors are in the expanded set.

## Data loading

Two tiers:

1. **Projects + boards** are fetched eagerly at startup via `GET /api/projects` and `GET /api/projects/{id}` so the top levels of the tree are always navigable.
2. **Board snapshots** (`GET /api/boards/{id}`) are lazy. A board row renders as `unloaded • press → to hydrate` until the user expands it. Hydration loads lists, cards, tasks, labels, memberships, and users in one round trip.

The live subscription streams deltas against the snapshot for whichever board is the live target. See [live-target.md](live-target.md) for the subscription model.

## Editing model

- **Title** — `e` enters an inline editor in the details pane. `Enter` saves via `PATCH /api/cards/{id}`; `Esc` discards.
- **Description** — `E` shells out to `$EDITOR` with the current description in a temp file. On editor exit, the TUI saves if the content changed.

The save is optimistic in the UI sense (the edit is sent immediately) but pessimistic in the UX sense (the whole TUI freezes into a `SAVING` mode until the server responds, with only `Ctrl-c` honored).

## Architecture

`plnk-tui` is its own workspace crate. It depends on `plnk-core` for domain types and nothing from `plnk-cli`. Its additional runtime dependencies:

- `ratatui` + `crossterm` — terminal rendering
- `tokio-tungstenite` — raw websocket for the Planka Engine.IO / Socket.IO protocol
- `rpassword` — password prompt during REST login

State lives in a single `AppState` struct, mutated on the event loop by `apply(event)`. Events come from two sources:

1. **Keyboard** via `crossterm`, producing app-local actions.
2. **Websocket** via a background `tokio::spawn`'d listener that pushes `AppEvent` values through a `std::sync::mpsc::Sender`.

Rendering is pure: `fn draw(frame, &AppState)` reads state, produces widgets, returns. No mutation during draw.

## What the TUI deliberately does not do

- No kanban board layout. Cards are listed vertically under their list, not arranged in columns.
- No drag-and-drop card moves. Move cards via the CLI (`plnk card move`) or the web UI.
- No full parity with every Planka web feature — attachments upload, permissions UI, etc. remain browser-only.
- No multi-board live subscriptions. One board live at a time; see [live-target.md](live-target.md).
- No second persistent-state layer. The TUI's auth token and config come from `plnk-core` via the shared `~/.config/plnk/config.toml`.

## Related

- [overview.md](overview.md) — install and run
- [keybindings.md](keybindings.md) — complete key map
- [live-target.md](live-target.md) — websocket subscription model
