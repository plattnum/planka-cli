# Tasks

Tasks are checklist items on a card. They have a title and a completed/incomplete state, and every task belongs to exactly one [task list](task-lists.md) — the named checklist that groups it.

## Commands

### List tasks on a card

```bash
plnk task list --card <cardId>
plnk task list --card <cardId> --task-list QA      # one checklist only
plnk tasks --card <cardId>                          # alias
```

Without `--task-list`, every task on the card is returned across all of its checklists. The table view shows a `Task List` column so you can tell them apart; JSON carries `taskListId` on every task.

With `--task-list <id|name>`, only that checklist's tasks are returned, read directly from the task list endpoint.

### Read a task

Tasks have no standalone `get` command — Planka has no direct GET endpoint for them, and the old PATCH-with-empty-body workaround silently bumped `updatedAt` on every read. Fetch tasks through their parent card or checklist instead:

```bash
plnk task list --card <cardId>                       # all tasks on a card
plnk task list --card <cardId> --task-list <id>      # one checklist
plnk card snapshot <cardId> --output json            # whole card incl. tasks under `included.tasks`
```

### Create a task

```bash
plnk task create --card <cardId> --title "Write unit tests"
plnk task create --card <cardId> --title "Write unit tests" --task-list QA
```

`--task-list` accepts a task list ID or name, resolved within the card's own checklists. Without it:

| Task lists on the card | Behaviour |
|---|---|
| none | a checklist named "Tasks" is created and used |
| one | it is used |
| two or more | exit `2`, listing every candidate — pass `--task-list` |

The command never guesses between checklists. A wrong guess would land the task in the wrong list with no error, which is worse than stopping.

### Update a task

```bash
plnk task update <taskId> --title "Write integration tests"
```

### Complete a task

```bash
plnk task complete <taskId>
```

### Reopen a completed task

```bash
plnk task reopen <taskId>
```

### Delete a task

```bash
plnk task delete <taskId>
plnk task delete 5678 --yes
```

## Name resolution for `--task-list`

Three-tier matching, stopping at the first tier with results: exact, then case-insensitive, then substring.

| Result | Exit |
|---|---|
| an ID belonging to the card | used as-is, no matching |
| exactly one match | used |
| no match | `4`, with a pointer to `plnk task-list list --card <id>` |
| several matches | `2`, naming each candidate as `name (id)` — pass an ID |

## Examples

```bash
# A single checklist — nothing to choose
plnk task create --card 1234 --title "Design API"
plnk task create --card 1234 --title "Write tests"
plnk task create --card 1234 --title "Deploy to staging"

# Two checklists — say which
plnk task-list create --card 1234 --name QA
plnk task create --card 1234 --title "Regression pass" --task-list QA

# Check off completed items
plnk task complete 5678
plnk task complete 5679

# See what's left in one checklist
plnk task list --card 1234 --task-list QA --output json

# Reopen a task
plnk task reopen 5678
```
