# planka-cli

Rust CLI (`plnk`) and SDK (`plnk-core`) for Planka kanban project management. Deterministic, scriptable, hierarchy-aware, machine-readable. Usable by humans, shell scripts, CI/CD, and AI planners.

## What We're Building

A two-crate Rust workspace:

- **`plnk-core`** — library crate: HTTP client, domain models, API traits, auth system, error types. This is a standalone Planka SDK — no CLI dependency.
- **`plnk-cli`** — binary crate: clap command tree, output rendering (table/JSON/markdown), input handling, tracing. Thin shell over `plnk-core`.

Binary name: `plnk`

### Target Workspace Layout

```
planka-cli/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── plnk-core/                # library: API client, models, auth
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── models/           # domain models (the truth)
│   │       ├── api/              # traits + v1 impl + response structs
│   │       ├── client/           # HTTP transport (reqwest wrapper)
│   │       ├── auth/             # credential resolution, config file
│   │       └── error.rs          # PlankaError enum
│   └── plnk-cli/                 # binary: the `plnk` executable
│       └── src/
│           ├── main.rs           # entry point, tokio runtime
│           ├── app.rs            # clap app, global flags
│           ├── commands/         # one module per resource
│           ├── output/           # rendering (table, json, markdown)
│           └── input.rs          # stdin/file/literal text resolution
├── tests/                        # integration + E2E tests
└── docs/
    ├── design-specification.md   # CLI contract spec (v0.4)
    ├── architecture.md           # Rust implementation architecture
    └── prd/
        ├── PRD-Overview.md
        └── task-01..20.md        # task cards with full specs
```

## Build & Test

```bash
cargo check                       # compile check
cargo clippy -- -D warnings       # lint (zero warnings policy)
cargo fmt --check                 # format check
cargo test                        # all tests
cargo run -- --help               # run the CLI
```

## Core Design Rules

- **Grammar:** `plnk <resource> <action> [target] [flags]`
- **Hierarchy:** `project -> board -> list -> card -> task/comment`
- **IDs are opaque strings:** `type ResourceId = String;` — no parsing, no i64, no u32
- **`get` = ID only, `find` = scoped search** — never mix. `get` with a non-ID must fail validation, not search.
- **All searches must be scoped** — no global flat queries. `find` requires `--list`, `--board`, or `--project`. Sole exception: `project find` is unscoped because projects are the root resource and have no parent.
- **Three-tier matching:** exact case-sensitive -> case-insensitive -> substring. Stop at first tier with results.
- **Traits define API capabilities** — `ProjectApi`, `BoardApi`, `CardApi`, etc. CLI depends on traits, not implementations. Today's impl is `PlankaClientV1`.
- **Domain models own the truth** — API response structs (private) map to domain models (public). API wire format changes stay isolated.
- **JSON output = strict projection of serde** — trimmed JSON contains a subset of the full serde representation with identical keys, types, and nulls. Never translate field names or coerce types for output. `Tabular::trimmed_columns` returns `(serde_field, display_label)` pairs; labels exist for tables/markdown only and must never leak into JSON.
- **Errors are data** — `PlankaError` enum with typed variants, exit codes, and JSON-renderable error types. No `unwrap()` in library code. No `panic!` outside tests.
- **Stdout = data, stderr = logs/errors.** Always.
- **No `anyhow`** — we need typed errors for exit codes and structured JSON output.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | success |
| 2 | invalid arguments / validation |
| 3 | auth failure |
| 4 | not found |
| 5 | remote API/server error |

## Key Dependencies

clap (derive), reqwest, tokio, serde/serde_json, thiserror, tracing, toml, dirs, comfy-table, async-trait, url, rpassword

## Auth

Header: `X-API-Key` (not Bearer, not Authorization). Credential precedence: CLI flags > env vars (`PLANKA_SERVER`, `PLANKA_TOKEN`) > config file (`~/.config/planka/config.toml`). Config file permissions `0600`.

## Planka Instance (Live Test Server)

- Base URL: `http://storm-front:3002`
- Claude API Key: `tNub244N_MBnBqhLH7PE2fjwQD9w2w69t6f3uCrPM` (username: `claude`, role: editor)
- Admin API Key: `yIwkmdaE_12qnXmshnnrRG3aAl4d697lmJ17aAdOM`
- Project: "planka-cli" (id: `1753611015817266606`)
- API calls: `curl -s -H "X-API-Key: <key>" http://storm-front:3002/api/...`
- Existing MCP (`kanban-planka-v2`) has bugs — prefer direct API calls via curl when MCP fails

### Planka API Quirks

- List creation requires `"type": "active"`
- Card creation requires `"type": "project"`
- Board creation requires `"type": "kanban"`
- Position values: powers of 2 starting at 65536
- Board snapshot (`GET /api/boards/{id}`) returns nested `included` with lists, cards, tasks, labels, memberships, users

## Project Management (Planka Boards)

All task tracking lives in Planka on the planka-cli project:

| Board | Purpose | Cards |
|-------|---------|-------|
| Design | Reference specs (not kanban) | 12 design cards |
| m-0: Foundation | Workspace, models, HTTP, auth, CLI framework | PLNK-001..005 |
| m-1: Auth Commands | Auth command group | PLNK-006 |
| m-2: Core Hierarchy | User, project, board, list, card resources | PLNK-007..011 |
| m-3: Supporting Resources | Task, comment, label, assignee, attachment, membership | PLNK-012..017 |
| m-4: Polish & Release | Aliases, tests, CI/CD | PLNK-018..020 |

### Workflow

- Read Design board at session start to load product context.
- Move card to In Progress when starting work.
- Comment on the card with what you're doing or completed.
- Move to Review when ready for human review.
- Never move to Done without human approval.
- Prefix blocker comments with `BLOCKED:`.
- Cards move left-to-right. Moving backward requires a comment explaining why.

### Dependency Order

```
PLNK-001 (workspace)
  ├── PLNK-002 (models) ─── PLNK-005 (CLI framework)
  │   └── PLNK-003 (HTTP) ──────────┐
  │       └── PLNK-004 (auth) ──────┤
  │           └── PLNK-006 (auth cmds) ← both deps
  │               ├── PLNK-007 (user)
  │               ├── PLNK-008 (project)
  │               │   ├── PLNK-009 (board)
  │               │   │   └── PLNK-010 (list)
  │               │   │       └── PLNK-011 (card)
  │               │   │           ├── PLNK-012..016
  │               │   └── PLNK-017 (membership)
  │               └── PLNK-018 (aliases)
  └── PLNK-019 (tests) → PLNK-020 (CI/CD)
```
