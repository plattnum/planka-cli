# `plnk-tui` — terminal explorer for Planka

`plnk-tui` is the experimental terminal companion to [`plnk`](../cli/). Where `plnk` is a scripted, imperative CLI for operating on individual resources, `plnk-tui` is a stateful, live tree explorer — projects → boards → lists → cards — rendered in a two-pane ratatui layout with a websocket channel to the server so edits from the browser appear in your terminal in near real time.

## Status

Experimental. Tested against a self-hosted Planka instance. Scope is intentionally narrow:

- Browse the project hierarchy
- Inspect cards (metadata, description, comments)
- Edit card title inline and description in `$EDITOR`
- Watch a single board live over the websocket
- Copy the selected node's ID hierarchy to the system clipboard for handing off to an AI agent (`y` / `Y`)

It is not a replacement for the web UI — drag-and-drop, permission management, attachments upload, and the like all still live in the browser. `plnk-tui` is for the read-heavy / quick-edit case.

## Install

### From a checkout

```bash
cargo install --path crates/plnk-tui --force
```

This installs the `plnk-tui` binary into `~/.cargo/bin/`.

### From git

```bash
cargo install --git https://github.com/plattnum/planka-cli plnk-tui
```

### Build-only (dev)

```bash
cargo run -p plnk-tui -- --server http://your-planka-host --username you
```

## Run

Minimum flags:

```bash
plnk-tui --server http://your-planka-host --username you
```

You will be prompted for a password. The TUI authenticates over the REST API (same endpoint as `plnk auth login`) and then opens the explorer.

Environment variables (clap honors them automatically):

| Env var | What it fills |
|---------|---------------|
| `PLANKA_SERVER` | `--server` |
| `PLANKA_USERNAME` | `--username` |
| `PLANKA_PASSWORD` | `--password` (skip the prompt) |
| `PLNK_TUI_BOARD` | `--board` (optional, see [live-target.md](live-target.md)) |

## First-run experience

The TUI lands on the projects view with no live subscription active. Expand a project with `→` or `Enter`, pick a board, and either explore it read-only or press `L` to make it the live target — from that point on, edits on that board stream in. Press `L` again on the same board to unsubscribe and return to idle. Press `r` on the selected node when you want to refetch that slice of the hierarchy on demand, or `/` to filter the current explorer view client-side by substring or glob pattern.

For the detailed live-sync model, see [live-target.md](live-target.md). For the full key map, see [keybindings.md](keybindings.md). For the tree view's data-model and rendering contract, see [tree-view.md](tree-view.md). For the agent-handoff clipboard feature, see [fast-copy.md](fast-copy.md).

## Related

- [`plnk` CLI](../cli/) — the scriptable sibling
- [AGENTS.md](../../AGENTS.md) — design rules for the whole repo
