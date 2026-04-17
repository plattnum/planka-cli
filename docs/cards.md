# Cards

Cards belong to a list. They are the primary work items in Planka. Cards have tasks (checklists), comments, labels, assignees, and attachments.

## Commands

### List cards in a list

```bash
plnk card list --list <listId>
plnk cards --list <listId>                  # alias
```

### Get a card by ID

```bash
plnk card get <cardId>
plnk card get 1234 --output json
plnk card get 1234 --full                   # include all fields
```

### Find cards by title

Must be scoped to a list, board, or project. Uses three-tier matching: exact > case-insensitive > substring.

```bash
# Search within a list (fastest, fewest API calls)
plnk card find --list <listId> --title "Fix auth"

# Search within a board (searches all lists)
plnk card find --board <boardId> --title "Fix auth"

# Search within a project (searches all boards and lists)
plnk card find --project <projectId> --title "auth"
```

Always returns a collection. Multiple results are expected, not an error.

### Get a card snapshot

```bash
plnk card snapshot <cardId> --output json
```

Returns the full `GET /api/cards/{id}` response verbatim, including `item` (the card — with fields the normal `get` discards like `commentsTotal`, `coverAttachmentId`, `listChangedAt`, `prevListId`, `stopwatch`, `type`) and `included` (tasks, taskLists, comments, attachments, cardMemberships, cardLabels, users, customFieldGroups, customFields, customFieldValues). Nothing is dropped. JSON only.

### Create a card

```bash
plnk card create --list <listId> --title "Fix auth bug"

# With description
plnk card create --list 789 --title "Fix auth" --description "OAuth flow broken on mobile"

# Description from file
plnk card create --list 789 --title "Fix auth" --description @spec.md

# Description from stdin
echo "Details here" | plnk card create --list 789 --title "Fix auth" --description -

# Control position
plnk card create --list 789 --title "Urgent fix" --position top
plnk card create --list 789 --title "Low priority" --position bottom
```

### Update a card

```bash
plnk card update <cardId> --title "Fix auth race condition"
plnk card update <cardId> --description "Updated details"
plnk card update <cardId> --description @updated-spec.md

# Pipe from clipboard (macOS)
pbpaste | plnk card update <cardId> --description -
```

At least one field must be provided.

### Move a card to a different list

```bash
plnk card move <cardId> --to-list <listId>
plnk card move 1234 --to-list 790 --position top
plnk card move 1234 --to-list 790 --position bottom
```

### Archive / unarchive a card

```bash
plnk card archive <cardId>
plnk card unarchive <cardId>
```

### Delete a card

```bash
plnk card delete <cardId>
plnk card delete 1234 --yes
```

### Card labels

Manage which labels are applied to a card. Labels must first be created on the board (see [Labels](labels.md)).

```bash
plnk card label list <cardId>
plnk card label add <cardId> <labelId>
plnk card label remove <cardId> <labelId>
```

### Card assignees

Manage which users are assigned to a card.

```bash
plnk card assignee list <cardId>
plnk card assignee add <cardId> <userId>
plnk card assignee remove <cardId> <userId>
```

## Examples

```bash
# Full card lifecycle
plnk card create --list 789 --title "Implement login" --description @login-spec.md
plnk card label add 1234 111                     # tag as "urgent"
plnk card assignee add 1234 88                   # assign to user
plnk card move 1234 --to-list 790                # move to "In Progress"
plnk task create --card 1234 --title "Write tests"
plnk task create --card 1234 --title "Update docs"
plnk comment create --card 1234 --text "Starting work"
plnk card move 1234 --to-list 791 --position top # move to "Done"

# Script: find all cards matching "auth" in a project, output JSON
plnk card find --project 123 --title "auth" --output json

# Script: create a card and capture its ID
ID=$(plnk card create --list 789 --title "New task" --output json | jq -r '.data.id')
echo "Created card $ID"
```
