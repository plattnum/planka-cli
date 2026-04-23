# Tasks

Tasks are checklist items on a card. They have a title and a completed/incomplete state.

## Commands

### List tasks on a card

```bash
plnk task list --card <cardId>
plnk tasks --card <cardId>                  # alias
```

### Read a task

Tasks have no standalone `get` command — Planka has no direct GET endpoint for them, and the old PATCH-with-empty-body workaround silently bumped `updatedAt` on every read. Fetch tasks through their parent card instead:

```bash
plnk task list --card <cardId>              # all tasks on a card
plnk card snapshot <cardId> --output json   # whole card incl. tasks under `included.tasks`
```

### Create a task

```bash
plnk task create --card <cardId> --title "Write unit tests"
plnk task create --card 1234 --title "Update documentation"
```

If the card has no task list, one is created automatically.

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

## Examples

```bash
# Create a checklist on a card
plnk task create --card 1234 --title "Design API"
plnk task create --card 1234 --title "Write tests"
plnk task create --card 1234 --title "Deploy to staging"

# Check off completed items
plnk task complete 5678
plnk task complete 5679

# See what's left
plnk task list --card 1234 --output json

# Reopen a task
plnk task reopen 5678
```
