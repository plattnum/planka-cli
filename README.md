<p align="center">
  <img src="docs/images/hero.png" alt="plnk — CLI & TUI for Planka" width="100%">
</p>

<p align="center"><em>scriptable • live • hierarchical</em></p>

# plnk

[![CI](https://github.com/plattnum/planka-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/plattnum/planka-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black)](https://www.buymeacoffee.com/plattnum)

A deterministic, scriptable, hierarchy-aware CLI and SDK for [Planka](https://planka.app) kanban project management. Plus a live terminal TUI explorer built on the same stack.

> [!NOTE]
> Tested against self-hosted [Planka](https://planka.app) only. The cloud-hosted service hasn't been exercised yet — your mileage may vary.

## Two tools, one stack

| Tool | What it's for |
|------|---------------|
| [`plnk`](#plnk-cli) | Scriptable CLI for automation, CI/CD, and AI workflows |
| [`plnk-tui`](#plnk-tui) | Live terminal explorer with real-time websocket sync |

Both share config and auth — run `plnk init` once and both binaries are ready. Landing page at [plattnum.github.io/planka-cli](https://plattnum.github.io/planka-cli).

## Install

Requires Rust 1.87+. Prebuilt binaries are on the roadmap.

```bash
# From a checkout
cargo install --path crates/plnk-cli --force
cargo install --path crates/plnk-tui --force

# Or from git
cargo install --git https://github.com/plattnum/planka-cli plnk-cli
cargo install --git https://github.com/plattnum/planka-cli plnk-tui
```

## Quickstart

```bash
plnk init                 # interactive: server URL + API token
plnk auth status          # verify credentials resolve
plnk project list         # start driving Planka
plnk-tui                  # launch the TUI explorer
```

Walkthrough: [`docs/cli/examples.md`](docs/cli/examples.md).

## `plnk` (CLI)

Shape: `plnk <resource> <action> [target] [flags]`. Design principles:

- **Strict hierarchy** — `project → board → list → card → task/comment`. All `find`s are scoped. No global flat queries.
- **Typed exit codes** — `0` success · `2` validation · `3` auth · `4` not-found · `5` server.
- **Three outputs** — `table` for humans, `json` for scripts, `markdown` for reports.
- **Machine-readable help** — `plnk <cmd> --help --output json` returns a stable schema agents can bind to before running.
- **stdout is data, stderr is logs.**

Reference docs, one per resource:

- [Projects](docs/cli/projects.md) · [Boards](docs/cli/boards.md) · [Lists](docs/cli/lists.md) · [Cards](docs/cli/cards.md)
- [Tasks](docs/cli/tasks.md) · [Comments](docs/cli/comments.md) · [Labels](docs/cli/labels.md)
- [Attachments](docs/cli/attachments.md) · [Memberships](docs/cli/memberships.md) · [Users](docs/cli/users.md)
- [Authentication](docs/cli/auth.md) · [Grammar reference](docs/cli/grammar.md) · [Transport policy](docs/cli/transport.md)
- [Worked examples](docs/cli/examples.md)

## `plnk-tui`

A terminal-native explorer for the same hierarchy. Single-board websocket subscription means edits from the browser appear in your terminal in near real time.

```bash
plnk-tui --server http://your-planka-host --username you
# prompts for password
```

Navigate projects → boards → lists → cards with `↑↓→Enter`. Press `L` on any board to promote it to the live target. Edit titles inline with `e` or descriptions in `$EDITOR` with `E`.

Env pre-fills: `PLANKA_SERVER`, `PLANKA_USERNAME`, `PLANKA_PASSWORD`, `PLNK_TUI_BOARD`.

Docs: [`docs/tui/`](docs/tui/) — [overview](docs/tui/overview.md) · [keybindings](docs/tui/keybindings.md) · [live-target model](docs/tui/live-target.md) · [tree view reference](docs/tui/tree-view.md).

## Architecture

Three-crate Rust workspace:

- **`plnk-core`** — standalone [Planka](https://planka.app) SDK. HTTP client, domain models, API traits, auth, typed errors. Usable on its own.
- **`plnk-cli`** — the `plnk` binary. Clap grammar + output rendering over `plnk-core`.
- **`plnk-tui`** — the `plnk-tui` binary. Ratatui explorer + Engine.IO websocket.

API versioning lives behind traits. If Planka changes its API, only the `PlankaClientV1` implementation changes — domain models and the CLI layer are untouched.

## Building

```bash
cargo check
cargo clippy -- -D warnings
cargo fmt --check
cargo test
```

See [AGENTS.md](AGENTS.md) for the full design rules, API quirks, and contribution guidelines.

## Support

If this is useful to you, consider buying me a coffee.

[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black)](https://www.buymeacoffee.com/plattnum)

## License

MIT
