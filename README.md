# plnk

[![CI](https://github.com/plattnum/planka-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/plattnum/planka-cli/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A deterministic, scriptable, hierarchy-aware CLI and SDK for [Planka](https://planka.app) kanban project management. Built for humans, shell scripts, CI/CD pipelines, and AI planners.

## Features

- **Full Planka coverage** -- projects, boards, lists, cards, tasks, comments, labels, attachments, memberships, auth
- **Strict hierarchy** -- `project > board > list > card > task/comment`. All searches are scoped. No global flat queries.
- **Three output formats** -- `table` (default), `json` (structured envelope), `markdown`
- **Machine-readable everything** -- JSON output, structured errors with typed exit codes, machine-readable help (`--help --output json`)
- **Scriptable** -- stdin/file input (`--description -`, `--text @file.md`), `--yes` for non-interactive use, `--quiet` for silent operation
- **Two-crate architecture** -- `plnk-core` is a standalone Planka SDK usable by other Rust tools. `plnk-cli` is a thin shell over it.

## Installation

### From source

```bash
cargo install --git https://github.com/plattnum/planka-cli plnk-cli
```

### From binary release

Download the latest release for your platform from [Releases](https://github.com/plattnum/planka-cli/releases).

| Platform | Target | Archive |
|----------|--------|---------|
| Linux x64 | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | `.tar.gz` |
| macOS Intel | `x86_64-apple-darwin` | `.tar.gz` |
| macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
| Windows x64 | `x86_64-pc-windows-msvc` | `.zip` |

Extract and place `plnk` in your `$PATH`.

## Authentication

Three ways to authenticate, checked in this order (first match wins):

| Priority | Method | Server | Token |
|----------|--------|--------|-------|
| 1 | CLI flags | `--server <url>` | `--token <token>` |
| 2 | Environment | `PLANKA_SERVER` | `PLANKA_TOKEN` |
| 3 | Config file | `~/.config/planka/config.toml` | `~/.config/planka/config.toml` |

### Interactive login (stores token in config)

```bash
plnk auth login --server https://planka.example.com
# Prompts for email and password
```

### Direct token (for CI or pre-existing API keys)

```bash
plnk auth token set <token> --server https://planka.example.com
```

### Environment variables (stateless, for CI)

```bash
export PLANKA_SERVER=https://planka.example.com
export PLANKA_TOKEN=your-api-key
plnk project list
```

### Auth commands

```bash
plnk auth login [--server <url>] [--email <email>] [--password <pass>]
plnk auth token set <token> [--server <url>]
plnk auth whoami                    # show current user
plnk auth status                    # show credential source + validity
plnk auth logout                    # delete stored credentials
```

## Grammar

```
plnk <resource> <action> [target] [flags]
```

Resources: `project`, `board`, `list`, `card`, `task`, `comment`, `label`, `attachment`, `membership`, `user`, `auth`

### Global flags

| Flag | Description |
|------|-------------|
| `--server <url>` | Planka server URL |
| `--token <token>` | API token |
| `--output table\|json\|markdown` | Output format (default: `table`) |
| `-v` / `-vv` / `-vvv` | Verbosity: info / debug / trace (logs to stderr) |
| `--quiet` | Suppress all output |
| `--no-color` | Disable colored output |
| `--yes` | Skip confirmation prompts |
| `--full` | Show all fields (default is trimmed) |

### Plural aliases

Shortcuts for listing resources. Hidden from `--help`, identical output to canonical form.

```bash
plnk boards --project <id>         # plnk board list --project <id>
plnk lists --board <id>            # plnk list list --board <id>
plnk cards --list <id>             # plnk card list --list <id>
plnk cards --board <id>            # plnk card list --board <id>
plnk tasks --card <id>             # plnk task list --card <id>
plnk comments --card <id>          # plnk comment list --card <id>
plnk labels --board <id>           # plnk label list --board <id>
```

## Quick start

```bash
# Authenticate
plnk auth login --server https://planka.example.com

# Browse the hierarchy
plnk project list
plnk board list --project <projectId>
plnk list list --board <boardId>
plnk card list --list <listId>
plnk card list --board <boardId>
plnk card list --board <boardId> --label <labelId|name>

# Create a card
plnk card create --list <listId> --title "Fix auth bug"

# Add a description from a file
plnk card update <cardId> --description @spec.md

# Pipe description from stdin
pbpaste | plnk card update <cardId> --description -

# Find cards across a board
plnk card find --board <boardId> --title "auth"

# Find cards on a board by label only
plnk card find --board <boardId> --label "urgent"

# Label names are board-scoped; if a name is ambiguous, use the label ID instead
plnk label list --board <boardId>
plnk card list --board <boardId> --label <labelId>

# Find a project by name (the only unscoped find)
plnk project find --name "platform"

# Full snapshot (item + everything included) in one call — JSON only
plnk project snapshot <projectId> --output json
plnk board snapshot <boardId> --output json
plnk card snapshot <cardId> --output json

# Move a card (same board)
plnk card move <cardId> --to-list <listId> --position top

# Move a card across boards
plnk card move <cardId> --to-board <boardId> --to-list <listId>

# Add a checklist item
plnk task create --card <cardId> --title "Write tests"

# JSON output for scripting
plnk project list --output json
```

## Hierarchy

```
project
  board
    list
      card
        task
        comment
        attachment
    label
  membership
```

All scoped queries follow this hierarchy. You can't list cards without specifying a list (or board/project for `find`). You can't list tasks without a card. This is by design. Sole exception: `project find` is unscoped because projects are the root.

## Output formats

**Table** (default) -- human-readable, trimmed to essential fields:
```
plnk project list
```

**JSON** -- structured envelope for scripting:
```
plnk project list --output json
```
```json
{
  "success": true,
  "data": [{"id": "123", "name": "Platform"}],
  "meta": {"count": 1}
}
```

**Markdown** -- for reports and documentation:
```
plnk project get <id> --output markdown
```

### JSON errors

All errors produce structured JSON when `--output json` is set:
```json
{
  "success": false,
  "error": {
    "type": "ResourceNotFound",
    "message": "Resource not found: card 999"
  }
}
```

### Machine-readable help

```bash
plnk card create --help --output json
```

Returns structured JSON describing arguments, options, types, required/optional status, and examples.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 2 | Invalid arguments / validation error |
| 3 | Authentication failure |
| 4 | Resource not found |
| 5 | Remote API / server error |

## Text input

For `--description`, `--text`, and similar text flags:

| Syntax | Meaning |
|--------|---------|
| `"literal text"` | Inline text |
| `-` | Read from stdin |
| `@file.md` | Read from file |

## Search matching

`find` commands use three-tier matching (stops at first tier with results):

1. Exact case-sensitive match
2. Exact case-insensitive match
3. Substring case-insensitive match

`find` always returns a collection, never an error for multiple results.

## Command reference

Full command reference with examples for each resource is in [`docs/`](docs/):

- [Projects](docs/projects.md)
- [Boards](docs/boards.md)
- [Lists](docs/lists.md)
- [Cards](docs/cards.md)
- [Tasks](docs/tasks.md)
- [Comments](docs/comments.md)
- [Labels](docs/labels.md)
- [Attachments](docs/attachments.md)
- [Memberships](docs/memberships.md)
- [Users](docs/users.md)

## Architecture

Two-crate Rust workspace:

- **`plnk-core`** -- standalone Planka SDK. HTTP client, domain models, API traits (`ProjectApi`, `BoardApi`, `CardApi`, etc.), auth system, typed errors. Usable independently by other Rust tools, MCP servers, or TUI apps.
- **`plnk-cli`** -- the `plnk` binary. Clap command tree, output rendering, input handling. Thin shell over `plnk-core`.

API versioning is handled through traits. If Planka changes its API, only the implementation (`PlankaClientV1`) changes. Domain models and the CLI layer are untouched.

## Building

```bash
cargo check                       # compile check
cargo clippy -- -D warnings       # lint (zero warnings policy)
cargo fmt --check                 # format check
cargo test                        # all tests
cargo run -- --help               # run the CLI
```

## License

MIT
