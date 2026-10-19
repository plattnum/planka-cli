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
plnk list create --board <boardId> --name "Done" --type closed
plnk list create --board <boardId> --name "Blocked" --color berry-red --position top
```

- `--type` is `active` (the default) or `closed`.
- `--position` takes `top`, `bottom`, or a number. The default is `65536`.
- `--color` takes one of the named colors below. Planka ignores a color sent on
  create, so `plnk` creates the list and then sets the color with a second
  request. If that second request fails, the list still exists: the error names
  its ID and exits with the code of the underlying failure.

### Update a list

```bash
plnk list update <listId> --name "Done"
plnk list update <listId> --position 131072
plnk list update <listId> --type closed
plnk list update <listId> --color lagoon-blue
plnk list update <listId> --clear-color
```

`--color` and `--clear-color` are mutually exclusive.

## List types

| Type | Meaning |
|------|---------|
| `active` | A normal working list. |
| `closed` | A "done" list. Moving a card into it marks the card closed (`isClosed: true`); moving it back to an `active` list reopens it. |

Every board also has `archive` and `trash` system lists. They cannot be created
or set from `plnk`, and `list list` does not return them.

`list list` shows a `Type` column, so the closed list is identifiable without
JSON.

## List colors

Planka accepts exactly ten list colors. Anything else exits `2` before any
request is sent.

`berry-red`, `pumpkin-orange`, `lagoon-blue`, `pink-tulip`, `light-mud`,
`orange-peel`, `bright-moss`, `antique-blue`, `dark-granite`, `turquoise-sea`

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

# Create three lists for a kanban workflow, with a closed "Done" list
plnk list create --board 456 --name "To Do"
plnk list create --board 456 --name "In Progress"
plnk list create --board 456 --name "Done" --type closed

# Find the "In Progress" list
plnk list find --board 456 --name "progress" --output json
```
