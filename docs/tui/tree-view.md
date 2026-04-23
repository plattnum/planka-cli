# TUI: Tree-view terminal UI for `plnk`

Status: exploratory design for an experimental branch.

## 1. Product intent

Build a terminal UI for Planka that behaves more like a project explorer than a kanban board.

The TUI should help a user:

- browse the Planka hierarchy quickly
- inspect details without leaving the keyboard
- edit card content in-place
- stay in sync with the server without turning the terminal into a second kanban board

This is explicitly **not** a board-visualization project. Planka's web UI already does kanban well. The TUI should focus on hierarchy, detail, and fast text-heavy workflows.

## 2. Goals

### Primary goals

- Show the full hierarchy as a collapsible tree:
  - Project
  - Board
  - List
  - Card
- Show details for the selected node in a right-hand pane.
- Make card inspection fast and pleasant for keyboard-first workflows.
- Support editing card title + description and saving back to Planka.
- Reuse `plnk-core` as the data/API layer.
- Keep the design deterministic and script-friendly in spirit, even though the UI is interactive.

### Secondary goals

- Show tasks, labels, assignees, comments, and attachments in the detail pane.
- Allow lightweight card operations from the TUI.
- Support refresh and eventual live updates.

## 3. Non-goals

- Recreate the kanban board layout in the terminal.
- Replace the existing CLI for automation.
- Attempt full parity with every Planka web feature in v1.
- Build a highly mouse-driven interface.
- Depend on websocket/live-sync for the first usable release.

## 4. Recommended experimental shape

For discovery work, use a separate experimental branch and keep the TUI isolated from the stable CLI path.

Recommended branch name:

- `experiment/tui-tree-view`

Recommended code shape during the experiment:

- add a new workspace crate: `crates/plnk-tui`
- let `plnk-tui` depend on `plnk-core`
- keep `plnk-cli` stable while the TUI evolves

Why this shape:

- isolates ratatui/crossterm/socket dependencies from the CLI binary
- lets the TUI move quickly without disturbing existing command grammar
- keeps the transport/auth/domain logic shared through `plnk-core`

If the experiment succeeds, there are two integration options later:

1. keep a separate binary during beta (`plnk-tui`)
2. fold the launcher into the main binary as `plnk tui`

Current recommendation: **start as a separate crate/binary**. That is the easiest way to ship two artifacts from one GitHub repository while keeping the existing CLI stable.

## 5. High-level UX

### Layout

A three-region terminal layout works well:

1. **Header/status bar**
   - server
   - authenticated user
   - current project/board path
   - connection state (`loaded`, `refreshing`, `offline`, `live`)
   - dirty state indicator when editing

2. **Main body**
   - **Left pane**: hierarchy tree
   - **Right pane**: detail/editor pane

3. **Footer/help bar**
   - contextual key hints
   - transient success/error messages

Suggested split:

- left pane: 30-40%
- right pane: 60-70%

### Tree pane behavior

The tree should feel like a file explorer.

Each node has:

- type icon/glyph
- label/name
- expand/collapse affordance
- selected state
- loading state
- optional badge counts

Example:

```text
▾ Project: planka-cli
  ▾ Board: Work
    ▸ List: Backlog (2)
    ▾ List: InProgress (1)
      • Card: TUI: Tree-view terminal UI for plnk
  ▸ Board: Design
```

### Detail pane behavior

The detail pane should change based on the selected node type.

#### Project selected

Show:
- name
- description
- board count
- managers
- created/updated timestamps

#### Board selected

Show:
- name
- project
- list count
- card count
- labels
- members

#### List selected

Show:
- name
- board
- card count
- list position
- quick card preview table

#### Card selected

This is the main use case.

Show:
- title
- description
- board/list path
- labels
- assignees
- due date / closed state
- tasks with completion state
- comments
- attachments
- created/updated metadata

For cards, the RHS should support **view mode** and **edit mode**.

## 6. Editing model

### v1 editable fields

Recommend v1 edit scope:

- card title
- card description

Optional v1.1:

- task complete/reopen
- add task
- add comment

Defer until later:

- drag-like reordering semantics
- attachment upload from inside the TUI
- label creation workflows
- cross-board move workflows

### Edit UX

Best hybrid model:

- short fields edited inline
- long description edited via a dedicated editor mode or `$EDITOR`

Recommended behavior:

- `e` on title -> inline edit widget
- `e` on description -> open multiline editor view
- `E` on description -> open external `$EDITOR` temp file flow
- `Ctrl-s` -> save dirty fields
- `Esc` -> cancel current edit
- `q` -> quit only when no unsaved changes, otherwise prompt

### Save semantics

Use explicit save, not auto-save.

Reason:

- safer in terminal UI
- easier to reason about dirty state
- avoids accidental updates from navigation
- simpler conflict handling

When saving:

- compute changed fields only
- call `UpdateCard` with just those fields
- refresh card snapshot after success
- clear dirty state
- show toast/status message

### Conflict handling

Minimum viable strategy:

- save local edits
- on success, re-fetch server snapshot
- if save fails due to remote conflict or validation, keep local buffer and show error

A better later strategy:

- track `updated_at` from the last fetch
- warn if remote `updated_at` changed before save
- offer reload/discard/overwrite flow

## 7. Navigation and key bindings

Suggested initial keymap:

### Global

- `q` quit
- `?` toggle help overlay
- `r` refresh selected scope
- `R` full refresh
- `/` filter/search within current tree scope
- `Tab` switch focus between tree and detail
- `Shift-Tab` reverse focus

### Tree pane

- `j` / `k` move selection
- `g` top
- `G` bottom
- `h` collapse node or move to parent
- `l` expand node or move to first child
- `Enter` select / expand / collapse
- `Space` toggle expand/collapse

### Detail pane

- `e` edit focused field
- `Ctrl-s` save
- `Esc` cancel edit
- `n` create item in context (later phase)
- `d` delete/archive in context (later phase, prompt required)
- `m` move card (later phase)

## 8. Data-loading strategy

The existing `plnk-core` snapshot methods are a strong fit for this UI.

### Initial load

At startup:

- `list_projects`

This populates root nodes only.

### Expand project

On first project expansion:

- `list_boards(project_id)`
- or directly `get_project_snapshot(project_id)` if project-level included data becomes useful

Current `plnk-core` already gets boards from the project snapshot, so project expansion can stay efficient.

### Expand board

On first board expansion:

- `get_board_snapshot(board_id)`

This is the key optimization. A single board snapshot can populate:

- lists
- cards
- labels
- memberships
- users

The TUI should cache the board snapshot and derive list/card nodes from it rather than making separate `list_lists` + `list_cards` calls.

### Select card

On first card selection or explicit refresh:

- `get_card_snapshot(card_id)`

This can populate the detail pane in one fetch:

- card item
- task lists
- tasks
- comments
- attachments
- labels/assignees via included relationships

### Cache policy

Use lazy loading with in-memory caches.

Recommended caches:

- project cache
- board snapshot cache
- card snapshot cache
- user lookup cache

Each cache entry should track:

- `loaded`
- `loading`
- `error`
- `fetched_at`
- raw snapshot/domain data

## 9. Internal architecture

## 9.1 Suggested crate structure

```text
crates/plnk-tui/
└── src/
    ├── main.rs          # runtime/bootstrap
    ├── app.rs           # event loop + top-level App struct
    ├── state.rs         # AppState / reducer-ish transitions
    ├── events.rs        # keyboard, resize, tick, network events
    ├── tree.rs          # tree node model + expansion/selection
    ├── data.rs          # async loaders over plnk-core
    ├── detail.rs        # detail-pane state + field focus
    ├── editor.rs        # inline/multiline/external editor flows
    ├── render/
    │   ├── mod.rs
    │   ├── tree.rs
    │   ├── detail.rs
    │   ├── status.rs
    │   └── help.rs
    └── live.rs          # optional websocket/polling adapter
```

## 9.2 State model

At minimum:

```text
AppState
- session/auth context
- focus (tree/detail/modal)
- tree state
- caches
- selected node id
- detail pane state
- dirty edit buffer
- background job status
- notifications/toasts
- live-sync state
```

### Tree node identity

Use stable typed node ids, not display text.

Example:

```rust
enum NodeId {
    Project(String),
    Board(String),
    List(String),
    Card(String),
}
```

This keeps selection stable across refreshes.

### Async event flow

Use a central event loop that merges:

- keyboard events
- terminal resize events
- periodic tick events
- async loader completions
- optional live-update events

A message-passing architecture is preferable to letting widgets call the network directly.

## 10. Reuse of `plnk-core`

The TUI should depend on `plnk-core` traits and models, not raw HTTP calls spread through the UI.

Good reuse points already present:

- auth resolution
- typed errors
- transport policy
- project/board/card snapshot methods
- update methods for cards/tasks/comments

One likely refactor worth doing before or during TUI work:

- extract shared client/bootstrap helpers so both `plnk-cli` and `plnk-tui` can construct a client the same way

That avoids duplicating:

- credential resolution
- transport override handling
- tracing init shape

## 11. Live updates: feasibility and risk

## 11.1 What looks promising

Planka's frontend bundle clearly uses Sails + Socket.IO and subscribes to events such as:

- `projectUpdate`
- `boardUpdate`
- `listCreate`
- `listUpdate`
- `cardCreate`
- `cardUpdate`
- `cardDelete`
- `taskCreate`
- `taskUpdate`
- `commentCreate`
- `attachmentCreate`
- and related membership/label events

So the product does appear to have a real-time event system that a TUI could potentially consume.

## 11.2 Main risk

The frontend also appears to use a cookie-backed access-token flow for the socket path:

- `/access-tokens?withHttpOnlyToken=true`
- socket path at `${BASE_PATH}/socket.io`

That matters because the current CLI/TUI auth model is API-key based via `X-API-Key`.

So the big unanswered question is not whether Planka has live events.
It does.

The real question was:

**Can a non-browser client authenticate to the socket channel using username/password login and then subscribe to live events?**

### Current spike result

Yes — this now looks **feasible**.

The working shape is:

1. log in with `POST /api/access-tokens` using `emailOrUsername + password`
2. if the instance requires terms acceptance, fetch `/api/terms` and complete `POST /api/access-tokens/accept-terms`
3. use the returned **Bearer access token** for API and socket request headers
4. connect to `/socket.io/` with Sails SDK query params:
   - `__sails_io_sdk_version=1.2.1`
   - `__sails_io_sdk_platform=node|rust`
   - `__sails_io_sdk_language=javascript|rust`
5. force **websocket transport** for the spike
6. include an `Origin` header for the opening websocket handshake
7. emit Sails-style socket requests like `get` with a request context payload and `Authorization: Bearer ...`
8. subscribe with `GET /api/boards/{id}?subscribe=true`

That is enough to receive events like `cardUpdate` from live board activity.

## 11.3 Recommendation

Because live sync is a **hard requirement**, websocket proof should be the **first implementation milestone**, not a later nice-to-have.

Recommended order now:

- prove username/password login + websocket board subscription first
- build a minimal TUI shell that renders something observable
- only then invest in the full tree/detail architecture

## 11.4 Practical fallback

A fallback still exists if some server environments behave differently:

- keep websocket as the target path
- use manual refresh only as a temporary debugging aid during development
- do not treat polling/manual refresh as an acceptable product substitute for v1

## 12. Suggested milestones

### Milestone 0: websocket-first spike

- scaffold `plnk-tui`
- log in with username/password
- complete terms-acceptance flow if required
- connect to `/socket.io/`
- subscribe to one board with `?subscribe=true`
- render visible projects + live event feed in a minimal TUI shell

Exit criteria:
- user can log in interactively
- TUI renders something meaningful
- remote board/card edits show up live in the terminal

### Milestone 1: hierarchy explorer shell

- root shows **all projects visible to the authenticated user**
- lazy loading for projects -> boards -> lists -> cards
- tree selection state
- archive lists hidden
- loading/errors surfaced in the UI

Exit criteria:
- usable read-only hierarchy browser with live updates still attached

### Milestone 2: RHS detail pane

- detail pane for project/board/list/card
- card is the primary focus
- read-only card detail first, still with live updates applied

Exit criteria:
- card inspection is comfortable enough for daily use

### Milestone 3: card editing v1

- edit card title
- edit card description
- long-form description editing defaults to `$EDITOR`
- explicit save/discard flow
- live-update/conflict behavior defined

Exit criteria:
- can replace common title/description edits without leaving terminal

### Milestone 4: richer workflows / v2 surface

- task toggle/add
- comment add
- labels/assignees work
- move card
- later: create/rename project, board, list, card

Exit criteria:
- supports broader maintenance workflows beyond simple card editing

## 13. Testing strategy

### Unit tests

- tree expansion/collapse logic
- selection movement
- reducer/state transitions
- dirty buffer behavior
- save/cancel/conflict flows

### Integration tests

- HTTP-backed loading with `wiremock` where practical
- snapshot parsing from realistic board/card payloads
- error rendering and retry states

### UI tests

- ratatui rendering snapshots for key screens
- keyboard interaction tests around navigation/editing

### Manual/live tests

- real Planka server against the `planka-cli` project
- verify large board/card behavior
- verify save correctness and refresh semantics
- later: verify socket reconnect behavior

## 14. Recommendation on v1 scope

Based on clarified product decisions, v1 should be:

- hierarchy tree on the left
- detail pane on the right
- root shows **all projects visible to the logged-in user**
- card title/description editing only
- long description editing defaults to **`$EDITOR`**
- archive lists hidden
- no kanban layout
- **websocket live sync proven early and kept in-scope for v1**

## 15. Product decisions captured so far

1. Root shows **all projects the authenticated user can see**.
2. V1 card editing is limited to **title + description**.
3. Long description editing should default to **`$EDITOR`**, ideally dropping the user into a familiar editor flow.
4. V1 does **not** include project/board/list editing.
5. V1 hides the archived list.
6. During the experiment, a separate **`plnk-tui`** binary is preferred because it is the easiest path.
7. **Manual refresh is not acceptable as the primary v1 model** — websocket sync must be proven early.

## 16. Bottom line

Yes, this looks very buildable.

The strongest fit is now:

- `ratatui` + `crossterm`
- separate experimental binary (`plnk-tui`)
- tree explorer rooted at all visible projects
- snapshot-driven loading via `plnk-core`
- `$EDITOR`-first description editing for v1
- websocket board subscriptions proven early via username/password login + Bearer token + Sails socket requests

The architecture already in `plnk-core` is still a good fit for the eventual explorer/editor, but the websocket/session-login spike justifies some early experimental code in `plnk-tui` first.
