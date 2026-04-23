---
name: plnk-cli
description: Use this skill when the user wants to inspect or manage Planka projects, boards, lists, cards, tasks, comments, labels, attachments, memberships, users, or authentication with the plnk CLI.
---

# plnk-cli

Use `plnk` as the canonical interface to Planka. Do not invent a parallel task API or assume a fixed kanban workflow. Operate through the CLI, prefer machine-readable output, and resolve ambiguity before mutating state.

## When to use this skill

Use this skill when the user wants to:

- inspect Planka projects, boards, lists, cards, tasks, comments, labels, attachments, memberships, or users
- create, update, move, archive, unarchive, or delete Planka resources
- find cards, lists, boards, or labels by name/title
- add comments, tasks, assignees, labels, or attachments to cards
- understand or debug `plnk` command behavior

## Core Rules

- Use `plnk` as the source of truth for Planka operations.
- Prefer `--output json` for agent work unless the user explicitly asked for table or markdown output.
- IDs are opaque strings. Pass them through exactly as returned.
- Use `get` for exact ID lookup.
- Use `find` for name/title lookup.
- `find` may return multiple results. Do not guess when matches are ambiguous.
- Prefer the narrowest scope possible: `--list` over `--board`, `--board` over `--project`.
- Read before write when current state is unclear.
- After discovery, perform mutations using IDs, not names.
- Ask before destructive or bulk operations unless the user was already explicit.
- Do not maintain a parallel TODO system unless the user explicitly asks for one.

## Resource Hierarchy

All operations are hierarchical.

```text
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

Implications:

- `project find` is the only unscoped `find`.
- `board find` requires `--project`.
- `list find` requires `--board`.
- `card find` requires exactly one of `--list`, `--board`, or `--project`.
- tasks and comments live under cards
- labels live under boards

## Default Operating Procedure

When the user asks to inspect or modify Planka state:

1. **Identify the narrowest possible scope.**
   - Prefer known IDs if already available.
   - Otherwise resolve names to IDs using `find` or `list`.

2. **Prefer JSON output.**
   - Use `--output json` for anything the agent needs to read, compare, or pipe.

3. **Resolve names before mutation.**
   - Find the project, board, list, card, label, or user first.
   - If multiple candidates match, summarize them and ask the user which one they mean.

4. **Read current state before writing when context is incomplete.**
   - Inspect the current board/list/card/task state before moving or editing if the target is not fully specified.

5. **Mutate by ID.**
   - Once a target has been identified, use the returned IDs for `update`, `move`, `archive`, `delete`, `label add/remove`, `assignee add/remove`, and similar operations.

6. **Confirm destructive or broad actions.**
   - Ask before `delete`, `archive`, bulk edits, or wide-scope moves unless the user was explicit.

7. **Report back with resolved names and IDs when useful.**
   - Especially after create/move/find operations or when ambiguity was resolved.

## Lists and Workflow

Lists are board-defined and may vary across users, teams, and boards. Do **not** assume canonical columns.

Never assume that a board has any specific list names such as:

- Backlog
- In Progress
- Review
- Done
- Blocked
- WontDo

When the user expresses workflow intent such as “move this to in progress”, “send this to review”, “put it in backlog”, or “mark this blocked”:

1. inspect the board's actual lists first
2. map the user's intent to an existing list
3. ask for confirmation if multiple lists plausibly match

## Auth and Output

Credential precedence:

1. CLI flags: `--server`, `--token`
2. Environment: `PLANKA_SERVER`, `PLANKA_TOKEN`
3. Config: `~/.config/planka/config.toml`

Useful auth checks:

```bash
plnk auth status
plnk auth whoami
```

Prefer JSON output for agent work:

```bash
plnk board list --project <projectId> --output json
plnk card find --board <boardId> --title "auth" --output json
```

JSON envelopes are shaped like:

```json
{"success": true, "data": [...], "meta": {"count": 3}}
```

and errors like:

```json
{"success": false, "error": {"type": "ResourceNotFound", "message": "..."}}
```

## Important Constraints

- `get` requires an ID. Never use `get` with a name.
- `find` returns collections, including zero or many matches.
- There is no standalone `get` for `task`, `comment`, or `label`.
  - use `task list --card <cardId>`
  - use `comment list --card <cardId>`
  - use `label list --board <boardId>`
  - or use `card snapshot` / `board snapshot`
- Snapshot commands are best when nested state is needed in one call:
  - `plnk project snapshot <projectId> --output json`
  - `plnk board snapshot <boardId> --output json`
  - `plnk card snapshot <cardId> --output json`
- Prefer narrow scopes for both correctness and performance.
- `stdout` is for data. `stderr` is for logs and diagnostics.

## Command Patterns

Browse the hierarchy:

```bash
plnk project list --output json
plnk board list --project <projectId> --output json
plnk list list --board <boardId> --output json
plnk card list --list <listId> --output json
```

Find resources by name/title:

```bash
plnk project find --name "Platform" --output json
plnk board find --project <projectId> --name "Sprint" --output json
plnk list find --board <boardId> --name "Backlog" --output json
plnk card find --board <boardId> --title "auth" --output json
```

Mutate a card:

```bash
plnk card create --list <listId> --title "Fix auth" --output json
plnk card update <cardId> --description @spec.md --output json
plnk card move <cardId> --to-list <listId> --position top --output json
```

Work with tasks and comments:

```bash
plnk task list --card <cardId> --output json
plnk task create --card <cardId> --title "Write tests" --output json
plnk comment create --card <cardId> --text "Starting work" --output json
```

## When Unsure

If command syntax, flags, or resource behavior are uncertain, inspect machine-readable help instead of guessing:

```bash
plnk --help --output json
plnk card --help --output json
plnk card create --help --output json
```

Then read the relevant references.

## References

Primary references:

- `references/commands.md`
- `references/api-quirks.md`

Resource docs:

- `../../docs/cli/projects.md`
- `../../docs/cli/boards.md`
- `../../docs/cli/lists.md`
- `../../docs/cli/cards.md`
- `../../docs/cli/tasks.md`
- `../../docs/cli/comments.md`
- `../../docs/cli/labels.md`
- `../../docs/cli/attachments.md`
- `../../docs/cli/memberships.md`
- `../../docs/cli/users.md`
- `../../docs/cli/transport.md`
