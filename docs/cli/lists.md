# Lists

Lists belong to a board. They are the columns in a kanban view. Cards live inside lists.

## Commands

### List lists in a board

```bash
plnk list list --board <boardId>
plnk lists --board <boardId>                # alias
```

### Get a list by ID

```bash
plnk list get <listId>
```

### Find lists by name

Searches within a board.

```bash
plnk list find --board <boardId> --name "Backlog"
plnk list find --board 456 --name "doing"
```

### Create a list

```bash
plnk list create --board <boardId> --name "In Progress"
```

### Update a list

```bash
plnk list update <listId> --name "Done"
plnk list update <listId> --position 131072
```

### Move a list

Reorder a list within its board.

```bash
plnk list move <listId> --to-position 65536
```

### Delete a list

```bash
plnk list delete <listId>
plnk list delete 789 --yes
```

## Examples

```bash
# Show all lists on a board
plnk lists --board 456

# Create three lists for a kanban workflow
plnk list create --board 456 --name "To Do"
plnk list create --board 456 --name "In Progress"
plnk list create --board 456 --name "Done"

# Find the "In Progress" list
plnk list find --board 456 --name "progress" --output json
```
