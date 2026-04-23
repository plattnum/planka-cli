# planka-cli

Rust CLI (`plnk`), SDK (`plnk-core`), and experimental TUI (`plnk-tui`) for [Planka](https://planka.app) kanban project management. Deterministic, scriptable, hierarchy-aware, machine-readable. Usable by humans, shell scripts, CI/CD, and AI agents.

## Workspace

```
crates/
  plnk-core/   library: HTTP client, domain models, API traits, auth, typed errors. Standalone SDK.
  plnk-cli/    binary `plnk`: clap command tree, output rendering (table/JSON/markdown), input handling.
  plnk-tui/    binary `plnk-tui`: experimental terminal UI with live websocket sync.
```

## Build & Test

```bash
cargo check                  # compile check
cargo clippy -- -D warnings  # lint (zero-warnings policy)
cargo fmt --check            # format check
cargo test                   # all tests
cargo run -- --help          # run the CLI
cargo run -p plnk-tui        # run the TUI
```

## Core Design Rules

- **Grammar:** `plnk <resource> <action> [target] [flags]`
- **Hierarchy:** `project → board → list → card → task/comment`
- **IDs are opaque strings:** `type ResourceId = String;` — no parsing, no `i64`, no `u32`.
- **`get` is ID-only, `find` is scoped search** — never mix. `get` with a non-ID fails validation; it does not fall back to searching.
- **All searches must be scoped.** `find` requires `--list`, `--board`, or `--project`. Sole exception: `project find` is unscoped (projects are root).
- **Three-tier matching:** exact case-sensitive → case-insensitive → substring. Stop at the first tier with results.
- **Traits define API capabilities.** `ProjectApi`, `BoardApi`, `CardApi`, etc. The CLI depends on traits, not implementations. Today's impl is `PlankaClientV1`.
- **Domain models own the truth.** Private wire response structs map to public domain models; wire-format changes stay isolated.
- **JSON output is a strict projection of serde.** Trimmed JSON is a subset of the full serde representation with identical keys, types, and nulls. Never translate field names or coerce types for output. `Tabular::trimmed_columns` returns `(serde_field, display_label)` pairs; labels exist for tables/markdown only and must never leak into JSON.
- **Errors are data.** `PlankaError` enum with typed variants, exit codes, and JSON-renderable error types. No `unwrap()` in library code. No `panic!` outside tests.
- **Stdout is data; stderr is logs and errors.** Always.
- **No `anyhow`.** Typed errors are required for exit codes and structured JSON output.

## Exit Codes

| Code | Meaning                          |
|------|----------------------------------|
| 0    | success                          |
| 2    | invalid arguments / validation   |
| 3    | auth failure                     |
| 4    | not found                        |
| 5    | remote API / server error        |

## Key Dependencies

`clap` (derive), `reqwest`, `tokio`, `serde`/`serde_json`, `thiserror`, `tracing`, `toml`, `dirs`, `comfy-table`, `async-trait`, `url`, `rpassword`.

## Auth

Planka uses an `X-API-Key` header (not `Bearer`, not `Authorization`).

Credential precedence:

1. CLI flags: `--server`, `--token`
2. Environment: `PLANKA_SERVER`, `PLANKA_TOKEN`
3. Config file: `~/.config/planka/config.toml` (mode `0600`)

Set credentials interactively:

```bash
plnk auth login                             # email + password → stored token
plnk auth token set <TOKEN> --server <URL>  # store an existing API token
plnk auth status                            # show active credential source
plnk auth whoami                            # verify against the server
plnk auth logout                            # remove stored credentials
```

`<TOKEN>` is a positional arg and will appear in shell history; read it from a secret store or prefix the command with a space if your shell honors `HIST_IGNORE_SPACE`.

## Planka API Quirks

Real-world wire format differences worth knowing when contributing:

- List creation requires `"type": "active"`.
- Card creation requires `"type": "project"`.
- Board creation requires `"type": "kanban"`.
- Position values are powers of 2 starting at `65536`.
- `GET /api/boards/{id}` returns a nested `included` object with lists, cards, tasks, labels, memberships, and users.

## Project Management

Task tracking lives in a Planka instance on the `planka-cli` project. Board and list names are not fixed — inspect actual state before acting. Do not assume milestone boards or canonical columns (`Backlog`, `In Progress`, etc.) exist.

Agents operating on Planka via `plnk` should use `skills/plnk-cli/SKILL.md` as the canonical guide. It covers scope rules, name-to-ID resolution, workflow-intent mapping, and safe mutation etiquette.
