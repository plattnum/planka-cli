# Labels

Labels are board-scoped tags with a name and color. Create labels on a board, then apply them to cards within that board.

## Board label commands

### List labels on a board

```bash
plnk label list --board <boardId>
plnk labels --board <boardId>               # alias
```

### Read labels

Labels have no standalone `get` command — Planka has no direct GET endpoint for them, and the old PATCH-with-empty-body workaround silently bumped `updatedAt` on every read. Fetch labels through their parent board instead:

```bash
plnk label list --board <boardId>           # all labels on a board
plnk board snapshot <boardId> --output json # whole board incl. labels under `included.labels` and cardLabels under `included.cardLabels`
```

### Find labels by name

```bash
plnk label find --board <boardId> --name "urgent"
plnk label find --board 456 --name "bug"
```

### Create a label

```bash
plnk label create --board <boardId> --name "urgent" --color berry-red
plnk label create --board 456 --name "feature" --color rain-blue
plnk label create --board 456 --name "blocked" --color sunset-orange
```

Planka color tokens: `berry-red`, `pumpkin-orange`, `light-mud`, `sunset-orange`, `rain-blue`, `lagoon-blue`, `sky-blue`, `midnight-blue`, `concrete-gray`, `bright-moss`, `dark-granite`, `pink-tulip`.

### Update a label

```bash
plnk label update <labelId> --name "critical"
plnk label update <labelId> --color berry-red
plnk label update 111 --name "blocker" --color sunset-orange
```

### Delete a label

```bash
plnk label delete <labelId>
plnk label delete 111 --yes
```

## Card label commands

Apply or remove labels from a specific card.

### List labels on a card

```bash
plnk card label list <cardId>
```

### Add a label to a card

```bash
plnk card label add <cardId> <labelId>
plnk card label add 1234 111
```

### Remove a label from a card

```bash
plnk card label remove <cardId> <labelId>
plnk card label remove 1234 111
```

## Examples

```bash
# Set up labels on a board
plnk label create --board 456 --name "bug" --color berry-red
plnk label create --board 456 --name "feature" --color rain-blue
plnk label create --board 456 --name "blocked" --color sunset-orange

# Tag a card
plnk card label add 1234 111

# See what labels a card has
plnk card label list 1234 --output json

# Find all labels matching "bug" on a board
plnk label find --board 456 --name "bug"
```
