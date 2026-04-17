---
name: plnk CLI — Planka Board Management
description: >
  This skill should be used when the user asks to "manage planka boards",
  "move a card", "create a task", "list boards", "find cards", "update card description",
  "add a comment to the card", "check what's on the board", "upload attachment",
  "assign a user", "add a label", or any Planka kanban board operation via CLI.
  Also triggers when working in a project that uses Planka for task tracking
  (indicated by CLAUDE.md referencing plnk, Planka, or board/card/task management).
---

# plnk CLI — Planka Board Management

`plnk` is a CLI and SDK for [Planka](https://planka.app) kanban project management. Use it to manage projects, boards, lists, cards, tasks, comments, labels, attachments, and memberships from the terminal.

## Grammar

```
plnk <resource> <action> [target] [flags]
```

Resources: `project`, `board`, `list`, `card`, `task`, `comment`, `label`, `attachment`, `membership`, `user`, `auth`

## Hierarchy

All operations follow a strict hierarchy. Never attempt unscoped queries.

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

To list cards, a list ID is required. To list tasks, a card ID is required. To find cards, scope to a list, board, or project. There are no global flat queries.

## Authentication

Credential precedence (first match wins):

1. CLI flags: `--server <url>` and `--token <token>`
2. Environment: `PLANKA_SERVER` and `PLANKA_TOKEN`
3. Config file: `~/.config/planka/config.toml`

For scripting and CI, prefer environment variables:

```bash
export PLANKA_SERVER=https://planka.example.com
export PLANKA_TOKEN=your-api-key
```

For interactive setup:

```bash
plnk auth login --server https://planka.example.com
```

Verify auth with `plnk auth whoami` or `plnk auth status`.

## Output Formats

Three formats controlled by `--output`:

| Flag | Use |
|------|-----|
| `--output table` | Default. Human-readable. |
| `--output json` | Structured envelope for scripting. Always has `success`, `data`, `meta`. |
| `--output markdown` | For reports and documentation. |

Use `--full` to include all fields (default output is trimmed).

### JSON envelope structure

Success:
```json
{"success": true, "data": [...], "meta": {"count": N}}
```

Error:
```json
{"success": false, "error": {"type": "ResourceNotFound", "message": "..."}}
```

### Capturing IDs from JSON output

```bash
ID=$(plnk card create --list 789 --title "New" --output json | jq -r '.data.id')
```

## Common Workflows

### Browse the hierarchy

```bash
plnk project list
plnk board list --project <projectId>
plnk list list --board <boardId>
plnk card list --list <listId>
```

Plural aliases exist for listing: `plnk boards --project X`, `plnk cards --list X`, `plnk tasks --card X`, `plnk comments --card X`, `plnk labels --board X`, `plnk lists --board X`.

### Card lifecycle

```bash
plnk card create --list <listId> --title "Fix auth"
plnk card update <cardId> --description @spec.md
plnk card move <cardId> --to-list <listId> --position top
plnk card archive <cardId>
```

### Find by name/title

Scoped search with three-tier matching (exact > case-insensitive > substring). Always returns a collection; multiple results are normal.

```bash
plnk project find --name "Platform"                            # unscoped — projects are root
plnk board find --project <projectId> --name "Sprint"
plnk list find --board <boardId> --name "Backlog"
plnk card find --list <listId> --title "auth"
plnk card find --board <boardId> --title "auth"
plnk card find --project <projectId> --title "auth"
plnk label find --board <boardId> --name "urgent"
```

### Snapshot (bulk fetch in one call)

Return the full `GET /api/<resource>/{id}` response verbatim — `item` plus every related resource Planka includes. Useful when a programmatic consumer needs all nested state in one round trip (e.g. a TUI rendering a full board). Nothing is dropped, including resources the CLI doesn't formally model (custom fields, notification services, stopwatch, etc.).

```bash
plnk project snapshot <projectId> --output json
plnk board snapshot <boardId> --output json
plnk card snapshot <cardId> --output json
```

JSON only. `--output table` / `--output markdown` fail with exit code 2.

### Tasks (checklists)

```bash
plnk task create --card <cardId> --title "Write tests"
plnk task complete <taskId>
plnk task reopen <taskId>
plnk task list --card <cardId>
```

### Comments

```bash
plnk comment create --card <cardId> --text "Starting work"
plnk comment create --card <cardId> --text @notes.md
echo "status update" | plnk comment create --card <cardId> --text -
```

### Labels

Create labels on a board, then apply to cards:

```bash
plnk label create --board <boardId> --name "urgent" --color berry-red
plnk card label add <cardId> <labelId>
plnk card label remove <cardId> <labelId>
```

### Attachments

```bash
plnk attachment upload --card <cardId> ./file.pdf
plnk attachment list --card <cardId>
plnk attachment download <attachmentId> --card <cardId>
plnk attachment download <attachmentId> --card <cardId> --out ./renamed.pdf
```

Download uses the real filename from Planka when `--out` is omitted.

### Memberships

```bash
plnk membership list --project <projectId>
plnk membership add --board <boardId> --user <userId> --role editor
plnk membership remove --project <projectId> --user <userId>
```

## Text Input

For `--description`, `--text`, and similar flags:

| Syntax | Source |
|--------|--------|
| `"literal"` | Inline |
| `-` | stdin |
| `@file.md` | File |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 2 | Invalid arguments |
| 3 | Auth failure |
| 4 | Not found |
| 5 | Server error |

## Global Flags

`--server`, `--token`, `--output table|json|markdown`, `-v`/`-vv`/`-vvv` (verbosity to stderr), `--quiet`, `--no-color`, `--yes` (skip confirmations), `--full` (all fields).

## Machine-Readable Help

Get structured JSON describing any command's arguments, options, and examples:

```bash
plnk card create --help --output json
```

## Key Rules

- **IDs are opaque strings.** Pass them through as-is. Never parse or cast to integers.
- **`get` requires an ID.** Never pass a name to `get`. Use `find` for name-based search.
- **`find` requires a scope.** Always provide `--list`, `--board`, or `--project`.
- **`find` returns collections.** Multiple results are expected, not errors.
- **stdout = data, stderr = logs.** Verbose logging (`-v`) never corrupts pipeable output.

## Additional Resources

### Reference Files

For the full command reference with every permutation and flag:
- **`references/commands.md`** — Complete command listing organized by resource
- **`references/api-quirks.md`** — Planka API behaviors that affect CLI usage
