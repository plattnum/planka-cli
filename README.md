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
| `--http-max-in-flight <n>` | Max in-flight HTTP requests per process |
| `--http-rate-limit <rps>` | Sustained HTTP request rate limit |
| `--http-burst <n>` | HTTP rate-limit burst size |
| `--retry-attempts <n>` | Retry attempts after the initial request |
| `--retry-base-delay-ms <ms>` | Base retry delay |
| `--retry-max-delay-ms <ms>` | Max retry delay |
| `--no-retry` | Disable automatic HTTP retries |

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

# Fetch multiple exact cards in one ordered collection
plnk card get-many --id <cardIdA> --id <cardIdB> --output json
plnk card get-many --id <cardIdA> --id <cardIdB> --concurrency 1
plnk card get-many --id <cardIdA> --id <missingId> --allow-missing --output json

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
- [Transport policy](docs/transport.md)

## Architecture

Two-crate Rust workspace:

- **`plnk-core`** -- standalone Planka SDK. HTTP client, domain models, API traits (`ProjectApi`, `BoardApi`, `CardApi`, etc.), auth system, typed errors. Usable independently by other Rust tools, MCP servers, or TUI apps.
- **`plnk-cli`** -- the `plnk` binary. Clap command tree, output rendering, input handling. Thin shell over `plnk-core`.

API versioning is handled through traits. If Planka changes its API, only the implementation (`PlankaClientV1`) changes. Domain models and the CLI layer are untouched.

## HTTP transport policy

`plnk-core` now has a shared transport policy model for whole-stack HTTP behavior.

Current status:

- every `HttpClient` carries a shared `TransportPolicy`
- all requests now flow through one common transport runtime hook
- shared concurrency caps, rate limiting, and safe-method retries are active now
- SDK callers can already set an explicit policy
- CLI, environment variables, and config file can now tune transport settings

Default policy values:

| Field | Default |
|------|---------|
| `max_in_flight` | `8` |
| `rate_limit_per_second` | `Some(10)` |
| `burst_size` | `Some(10)` |
| `retry_attempts` | `2` |
| `retry_base_delay_ms` | `250` |
| `retry_max_delay_ms` | `2000` |
| `retry_jitter` | `true` |
| `retry_safe_methods_only` | `true` |

### How to set transport settings today

**CLI users:** use global flags such as:

```bash
plnk --http-max-in-flight 4 --http-rate-limit 20 --retry-attempts 1 project list
plnk --no-retry project list
```

**Environment variables:**

```bash
export PLNK_HTTP_MAX_IN_FLIGHT=4
export PLNK_HTTP_RATE_LIMIT=20
export PLNK_HTTP_BURST=20
export PLNK_RETRY_ATTEMPTS=1
export PLNK_RETRY_BASE_DELAY_MS=250
export PLNK_RETRY_MAX_DELAY_MS=2000
```

**Config file:**

```toml
server = "https://planka.example.com"
token = "your-api-token"

[http]
max_in_flight = 8
rate_limit = 10
burst = 10
retry_attempts = 2
retry_base_delay_ms = 250
retry_max_delay_ms = 2000
```

Precedence is:
- CLI flags
- environment variables
- config file
- built-in defaults

**SDK users:** create `HttpClient` with an explicit policy:

```rust
use plnk_core::client::HttpClient;
use plnk_core::transport::TransportPolicy;
use url::Url;

let server = Url::parse("https://planka.example.com")?;
let policy = TransportPolicy {
    max_in_flight: 4,
    retry_attempts: 1,
    ..TransportPolicy::default()
};
let http = HttpClient::with_policy(server, "api-token", policy)?;
```

Current retry behavior:

- `GET`/`HEAD`/`OPTIONS` retry automatically by default
- `429`, `502`, `503`, and `504` are retryable
- `Retry-After` is honored when present
- `POST`/`PATCH`/`DELETE` are not retried automatically by default

For the full transport write-up, including validation rules and the current rollout status, see [docs/transport.md](docs/transport.md).

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
