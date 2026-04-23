# Boards

Boards belong to a project. Each board contains lists, which contain cards.

## Commands

### List boards in a project

```bash
plnk board list --project <projectId>
plnk boards --project <projectId>           # alias
```

### Get a board by ID

```bash
plnk board get <boardId>
plnk board get 456 --output json
```

### Find boards by name

Searches within a project. Uses three-tier matching: exact > case-insensitive > substring.

```bash
plnk board find --project <projectId> --name "Sprint"
plnk board find --project 123 --name "sprint"         # case-insensitive match
plnk board find --project 123 --name "pri"             # substring match
```

Always returns a collection, even for a single result.

### Get a board snapshot

```bash
plnk board snapshot <boardId> --output json
```

Returns the full `GET /api/boards/{id}` response verbatim, including `item` (the board) and `included` (lists, cards, labels, boardMemberships, cardMemberships, cardLabels, tasks, taskLists, attachments, users, projects, customFieldGroups, customFields, customFieldValues). Nothing is dropped. JSON only.

### Create a board

```bash
plnk board create --project <projectId> --name "Sprint 1"
```

### Update a board

```bash
plnk board update <boardId> --name "Sprint 2"
```

### Delete a board

```bash
plnk board delete <boardId>
plnk board delete 456 --yes
```

## Examples

```bash
# List all boards in a project, JSON output
plnk board list --project 123 --output json

# Find a board and get its ID for further commands
plnk board find --project 123 --name "Sprint" --output json

# Create a board and capture its ID
plnk board create --project 123 --name "Backlog" --output json
```
