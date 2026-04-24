# `plnk` worked examples

Runnable sessions showing the command grammar in action against a live Planka instance. Every block below is real output captured from the CLI. IDs are from the internal `planka-cli` project board so the flow reads top-to-bottom as a story.

The grammar is always `plnk <resource> <action> [target] [flags]` — see the [README](../../README.md) for the summary.

## Identity and auth

`plnk auth status` answers "where am I pointed, and does the server still accept my token?" — safe to run anytime, always exits `0`.

```console
$ plnk auth status
Server: http://planka.example.com/
Source: config file
User: Admin (admin)
```

`Source` is one of `CLI flags`, `environment variables`, or `config file`, reflecting the resolution chain (flags → env → `~/.config/plnk/config.toml`).

## Navigating the hierarchy

Find a project by name. `project find` is the one place `find` can be unscoped — projects are the root resource.

```console
$ plnk project find --name "planka-cli"
+---------------------+------------+
| ID                  | Name       |
+==================================+
| 1753611015817266606 | planka-cli |
+---------------------+------------+
```

Take that ID and drill into boards.

```console
$ plnk board list --project 1753611015817266606
+---------------------+----------+---------------------+----------+
| ID                  | Name     | Project             | Position |
+=================================================================+
| 1753722473917973936 | Inbox    | 1753611015817266606 | 65536.0  |
|---------------------+----------+---------------------+----------|
| 1755277449706342200 | Archived | 1753611015817266606 | 73728.0  |
|---------------------+----------+---------------------+----------|
| 1753736765077718454 | Design   | 1753611015817266606 | 81920.0  |
|---------------------+----------+---------------------+----------|
| 1755070544673244884 | Bugs     | 1753611015817266606 | 98304.0  |
|---------------------+----------+---------------------+----------|
| 1755733092435231757 | Work     | 1753611015817266606 | 163840.0 |
+---------------------+----------+---------------------+----------+
```

Every `find`/`list` that crosses levels demands an explicit parent — no global queries.

## Searching

`card find` supports the three-tier match (exact case-sensitive → exact case-insensitive → substring) and always requires a scope flag.

```console
$ plnk card find --title "TUI-012" --board 1755733092435231757
+---------------------+----------------------------------------------------------------+---------------------+----------+--------+
| ID                  | Name                                                           | List                | Position | Closed |
+================================================================================================================================+
| 1759453749463484058 | TUI-012 Startup: make --board optional, start on projects view | 1755734266848740373 | 65536.0  | no     |
+---------------------+----------------------------------------------------------------+---------------------+----------+--------+
```

## Structured output for scripts and agents

Every command accepts `--output json`. The JSON is a strict projection of the same data the table renders — identical keys, types, and nulls.

```console
$ plnk card find --title "TUI" --board 1755733092435231757 --output json | jq '.data[] | {id, name}'
{
  "id": "1756729740611290618",
  "name": "TUI-007 Live target: switch websocket board from explorer"
}
{
  "id": "1756732425871820287",
  "name": "TUI-008 Explorer: filter tree nodes by text/pattern"
}
{
  "id": "1756653083196130723",
  "name": "TUI-005 Edit cards: title + description with $EDITOR"
}
{
  "id": "1759453749463484058",
  "name": "TUI-012 Startup: make --board optional, start on projects view"
}
```

The envelope is always `{success, data, meta?, error?}` so scripts can branch on `success` without parsing.

## Card detail

Cards support the full output triad. `--output markdown` is handy for piping into notes, PR descriptions, or docs.

```console
$ plnk card get 1756505635064644838 --output markdown
**ID:** 1756505635064644838
**Name:** CFG-001 Init: interactive config bootstrap
**List:** 1755734271915459606
**Position:** 8192.0
**Closed:** yes
```

Add `--full` for the complete field set (description, timestamps, creator, subscription state).

## Exit codes for script branching

Every command exits with a typed status code so shell scripts can branch without parsing stderr.

```console
$ plnk card get 1234567890123456789
Error: HTTP 404 on GET /api/cards/1234567890123456789
  Server message: Card not found

$ echo $?
4
```

The table:

| Code | Meaning |
|------|---------|
| `0` | success |
| `2` | invalid arguments / validation |
| `3` | auth failure |
| `4` | not found |
| `5` | remote API / server error |

## Agent-friendly help

Every `--help` accepts `--output json` so agents can introspect the command surface without parsing ANSI-formatted text.

```console
$ plnk card find --help --output json | jq '{resource, action, summary, flags: (.options | keys)}'
{
  "resource": "card",
  "action": "find",
  "summary": "Find cards by title and/or label within a scope",
  "flags": [
    "--board",
    "--full",
    "--label",
    "--list",
    "--no-color",
    "--output",
    "--project",
    "--quiet",
    "--server",
    "--title",
    "--token",
    "--verbose",
    "--yes"
  ]
}
```

The schema is stable: every subcommand's `--help --output json` returns `{resource, action, summary, args, options}`, where each option has `{type, required, description}`. Agents can bind to the grammar before executing.
