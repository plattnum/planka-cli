# plnk Command Reference

Complete listing of every command, flag, and usage pattern.

## Auth

```bash
plnk auth login [--server <url>] [--email <email>] [--password <pass>]
plnk auth token set <token> [--server <url>]
plnk auth whoami
plnk auth status
plnk auth logout
```

- `login` — Interactive by default. Prompts for missing values. Fully non-interactive when all flags provided.
- `token set` — Write a pre-existing API key to config. If `--server` omitted, uses existing config server.
- `whoami` — Validates token against server. Exit 3 if invalid.
- `status` — Shows credential source (flags/env/config) and validity.
- `logout` — Deletes config file. Does not revoke token server-side.

## User

```bash
plnk user list
plnk user get <userId>
```

Read-only. No create/update/delete.

## Project

```bash
plnk project list
plnk project get <projectId>
plnk project create --name <name>
plnk project update <projectId> --name <name>
plnk project delete <projectId> [--yes]
```

- `update` requires at least one mutable field.
- `delete` prompts for confirmation unless `--yes`.

## Board

```bash
plnk board list --project <projectId>
plnk board get <boardId>
plnk board find --project <projectId> --name <name>
plnk board create --project <projectId> --name <name>
plnk board update <boardId> --name <name>
plnk board delete <boardId> [--yes]
```

Alias: `plnk boards --project <projectId>`

- `find` uses three-tier matching: exact case-sensitive > case-insensitive > substring.
- `list` and `find` return boards from the project snapshot.

## List

```bash
plnk list list --board <boardId>
plnk list get <listId>
plnk list find --board <boardId> --name <name>
plnk list create --board <boardId> --name <name>
plnk list update <listId> [--name <name>] [--position <float>]
plnk list move <listId> --to-position <float>
plnk list delete <listId> [--yes]
```

Alias: `plnk lists --board <boardId>`

- `list` returns only active lists (filters out archive lists with empty names).
- Position values are typically powers of 2 starting at 65536.

## Card

```bash
plnk card list --list <listId>
plnk card get <cardId>
plnk card find --list <listId> --title <title>
plnk card find --board <boardId> --title <title>
plnk card find --project <projectId> --title <title>
plnk card create --list <listId> --title <title> [--description <text>] [--position top|bottom|<int>]
plnk card update <cardId> [--title <title>] [--description <text>]
plnk card move <cardId> --to-list <listId> [--position top|bottom|<int>]
plnk card archive <cardId>
plnk card unarchive <cardId>
plnk card delete <cardId> [--yes]
```

Alias: `plnk cards --list <listId>`

- `find` requires exactly one scope flag (`--list`, `--board`, or `--project`).
- `find --board` fetches the board snapshot and searches all cards across lists.
- `find --project` fetches all boards in the project, then all cards in each board.
- `--description` accepts literal text, `-` for stdin, `@file.md` for file.
- `--position top` = position 0.0, `bottom` = max float, or provide a numeric value.
- `update` requires at least one mutable field.

### Card Labels

```bash
plnk card label list <cardId>
plnk card label add <cardId> <labelId>
plnk card label remove <cardId> <labelId>
```

- Labels must exist on the board first (see Label section).
- `list` returns card-label junction records from the card snapshot.

### Card Assignees

```bash
plnk card assignee list <cardId>
plnk card assignee add <cardId> <userId>
plnk card assignee remove <cardId> <userId>
```

- `list` returns card-membership records from the card snapshot.

## Task

```bash
plnk task list --card <cardId>
plnk task get <taskId>
plnk task create --card <cardId> --title <title>
plnk task update <taskId> [--title <title>]
plnk task complete <taskId>
plnk task reopen <taskId>
plnk task delete <taskId> [--yes]
```

Alias: `plnk tasks --card <cardId>`

- `list` fetches tasks from the card snapshot's included data.
- `create` automatically finds or creates a task list on the card.
- `get` uses PATCH with empty body (Planka has no direct GET for tasks). This bumps `updatedAt`.
- `complete` sets `isCompleted: true`. `reopen` sets `isCompleted: false`.

## Comment

```bash
plnk comment list --card <cardId>
plnk comment get <commentId>
plnk comment create --card <cardId> --text <text>
plnk comment update <commentId> --text <text>
plnk comment delete <commentId> [--yes]
```

Alias: `plnk comments --card <cardId>`

- `--text` accepts literal text, `-` for stdin, `@file.md` for file.
- `get` uses PATCH with empty body (same `updatedAt` caveat as tasks).
- `list` uses `GET /api/cards/{cardId}/comments`.

## Label

```bash
plnk label list --board <boardId>
plnk label get <labelId>
plnk label find --board <boardId> --name <name>
plnk label create --board <boardId> --name <name> --color <color>
plnk label update <labelId> [--name <name>] [--color <color>]
plnk label delete <labelId> [--yes]
```

Alias: `plnk labels --board <boardId>`

- Labels are board-scoped. To apply a label to a card, use `plnk card label add`.
- `list` fetches labels from the board snapshot's included data.
- `get` uses PATCH with empty body.

### Planka color tokens

`berry-red`, `pumpkin-orange`, `light-mud`, `sunset-orange`, `rain-blue`, `lagoon-blue`, `sky-blue`, `midnight-blue`, `concrete-gray`, `bright-moss`, `dark-granite`, `pink-tulip`

## Attachment

```bash
plnk attachment list --card <cardId>
plnk attachment upload --card <cardId> <file>
plnk attachment download <attachmentId> --card <cardId> [--out <path>]
plnk attachment delete <attachmentId> [--yes]
```

- `upload` sends a multipart form with the file, type, and name.
- `download` fetches the card snapshot to find the attachment's real filename and download URL. Without `--out`, saves to the current directory using the original filename.
- `list` fetches attachments from the card snapshot's included data.

## Membership

```bash
plnk membership list --project <projectId>
plnk membership list --board <boardId>
plnk membership add --project <projectId> --user <userId>
plnk membership add --board <boardId> --user <userId> [--role <role>]
plnk membership remove --project <projectId> --user <userId>
plnk membership remove --board <boardId> --user <userId>
```

- Exactly one of `--project` or `--board` must be provided.
- Project memberships use project-managers endpoint. Board memberships use board-memberships endpoint.
- `--role` is optional (e.g., `editor`, `viewer`).

## Plural Aliases

All aliases are hidden from `--help` and produce identical output to their canonical form.

| Alias | Canonical |
|-------|-----------|
| `plnk boards --project <id>` | `plnk board list --project <id>` |
| `plnk lists --board <id>` | `plnk list list --board <id>` |
| `plnk cards --list <id>` | `plnk card list --list <id>` |
| `plnk tasks --card <id>` | `plnk task list --card <id>` |
| `plnk comments --card <id>` | `plnk comment list --card <id>` |
| `plnk labels --board <id>` | `plnk label list --board <id>` |

## Global Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--server <url>` | Planka server URL | env `PLANKA_SERVER` or config |
| `--token <token>` | API token | env `PLANKA_TOKEN` or config |
| `--output table\|json\|markdown` | Output format | `table` |
| `-v` / `-vv` / `-vvv` | Verbosity (info/debug/trace) | warn only |
| `--quiet` | Suppress all output | off |
| `--no-color` | Disable colors | off |
| `--yes` | Skip confirmation prompts | off |
| `--full` | Show all fields | trimmed |

## Machine-Readable Help

Any command supports `--help --output json` for structured help:

```bash
plnk card create --help --output json
plnk board --help --output json
plnk --help --output json
```

Returns JSON with `resource`, `action`, `summary`, `args`, `options` (with type and required), and `examples`.
