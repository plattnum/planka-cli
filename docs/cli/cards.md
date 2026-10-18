# Cards

Cards belong to a list. They are the primary work items in Planka. Cards have tasks (checklists), comments, labels, assignees, and attachments.

## Commands

### List cards in a list or across a board

```bash
plnk card list --list <listId>
plnk card list --board <boardId>
plnk cards --list <listId>                  # alias
plnk cards --board <boardId>                # alias
```

Filter by one or more board-scoped labels with repeated `--label` flags. Repeated labels use AND semantics.

```bash
plnk card list --list <listId> --label <labelId>
plnk card list --board <boardId> --label "Urgent"
plnk card list --board <boardId> --label "Urgent" --label "Backend"
```

Filter by due date with `--due-after` / `--due-before`. The bounds are strict and
behave exactly as they do on `card find`; see [Find cards by title, label, and/or
due date](#find-cards-by-title-label-andor-due-date).

```bash
plnk card list --board <boardId> --due-before 2026-04-30
plnk card list --board <boardId> --due-after 2026-04-01 --due-before 2026-04-30
plnk card list --board <boardId> --due-before now --include-closed
```

List closed cards on their own with `--closed`:

```bash
plnk card list --board <boardId> --closed
```

### Get a card by ID

```bash
plnk card get <cardId>
plnk card get 1234 --output json
plnk card get 1234 --full                   # include all fields
```

### Get many cards by exact ID

```bash
plnk card get-many --id <cardId> --id <cardId>
plnk card get-many --id 1234 --id 5678 --output json
plnk card get-many --id 1234 --id 5678 --concurrency 1
plnk card get-many --id 1234 --id 9999 --allow-missing --output json
```

Notes:
- Exact ID fetch only — no title matching or search fallback.
- Output order matches the input `--id` order.
- Default concurrency is `4`; valid range is `1..=16`.
- `--allow-missing` downgrades per-card `404` misses into metadata, but auth/server/transport failures still fail the command.
- JSON `meta` includes `requestedCount`, `foundCount`, `missingCount`, `missingIds`, `concurrency`, and `allowMissing`.

### Find cards by title, label, and/or due date

Must be scoped to a list, board, or project. Title matching uses three-tier matching: exact > case-insensitive > substring.

```bash
# Search within a list (fastest, fewest API calls)
plnk card find --list <listId> --title "Fix auth"
plnk card find --list <listId> --label "Urgent"

# Search within a board (searches all lists)
plnk card find --board <boardId> --title "Fix auth"
plnk card find --board <boardId> --label "Urgent"
plnk card find --board <boardId> --label "Urgent" --label "Backend" --title "Fix auth"

# Search within a project (searches all boards and lists)
plnk card find --project <projectId> --title "auth"

# Filter by due date
plnk card find --board <boardId> --due-before 2026-04-30
plnk card find --board <boardId> --due-after 2026-04-01 --due-before 2026-04-30
plnk card find --board <boardId> --label "Urgent" --due-before 2026-04-30T12:00:00Z

# Closed cards are excluded from a due query; opt back in
plnk card find --board <boardId> --due-before now --include-closed

# Closed cards on their own
plnk card find --board <boardId> --closed
```

Notes:
- At least one of `--title`, `--label`, `--due-after`, `--due-before`, or `--closed` is required.
- `--label` is only supported with `--list` or `--board` scopes.
- Label names are resolved within the owning board using exact > case-insensitive > substring matching.
- If a label query matches more than one board label, the CLI tells you it matched multiple labels and prints the candidates.
- If no board label matches, the CLI tells you no label matched in the current board scope.
- If a name is ambiguous, use the label ID instead (`plnk label list --board <boardId>`).

Due-date bounds accept ISO 8601 timestamps: `YYYY-MM-DD` (midnight in your
local timezone) or `YYYY-MM-DDThh:mm[:ss[.fff]]` with an optional `Z` or `±hh:mm`
offset. A date-time without an offset is treated as UTC.

They also accept relative dates, resolved against the current time in your local
timezone: `now`, `today`, `tomorrow`, `yesterday`, `next week`, `next month`,
`2 weeks from today`, `in 3 days`, `3 days ago`, `this month`, and bare weekday
names. See [Timestamps](grammar.md#relative-dates) for the full grammar.

```bash
plnk card list --board <boardId> --due-before tomorrow
plnk card list --board <boardId> --due-after today --due-before 'next month'
plnk card find --board <boardId> --due-after '2 weeks ago' --due-before today
```

The bounds are strict: `--due-after` keeps cards due **strictly after** its
expression and `--due-before` keeps cards due **strictly before** its expression.
A card due exactly on a bound is excluded. Cards without a due date are excluded
whenever a due bound is set. A card whose due date cannot be parsed as ISO 8601
is excluded and reported on stderr.

A due query — any `card list` or `card find` with `--due-after` or `--due-before`
— also excludes closed cards. A closed card is done, so leaving it in the window
buries the cards that still need attention. Pass `--include-closed` to keep
them:

```bash
plnk card list --board <boardId> --due-before now --include-closed
```

The exclusion applies only when a due bound is set. A plain
`plnk card list --board <boardId>` keeps returning closed cards, exactly as
before. Use `--closed` to see closed cards on their own, with or without a due
window:

```bash
plnk card list --board <boardId> --closed
plnk card find --board <boardId> --closed --due-before now
```

`--closed` and `--include-closed` contradict each other. Passing both is a
validation error and exits `2`.

Relative dates resolve to the **start of the day** in local time. Because the
bounds are strict, `--due-before tomorrow` still includes every card due today:
those instants fall before tomorrow's local midnight. `now` is the exception: it
keeps the current time of day, so `--due-before now` selects overdue cards and
`--due-after now` selects cards due in the future. To select cards due *today*,
combine the two bounds:

```bash
plnk card list --board <boardId> --due-after today --due-before tomorrow
plnk card list --board <boardId> --due-before now          # overdue
plnk card list --board <boardId> --due-after now           # still to come
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
plnk card update <cardId> --due-date 2026-04-30
plnk card update <cardId> --due-date 2026-04-30T12:00:00Z
plnk card update <cardId> --due-date 'next week'
plnk card update <cardId> --due-date 'in 2 weeks'
plnk card update <cardId> --clear-due-date

# Pipe from clipboard (macOS)
pbpaste | plnk card update <cardId> --description -
```

At least one field must be provided.

`--due-date` accepts the same ISO 8601 timestamps and relative dates as
`card find`, and is normalised to UTC before it is sent. A relative date resolves
to the start of the day in local time and is stored as that absolute instant.
Omitting the flag leaves the stored due date alone.

Pass `--clear-due-date` to remove the due date. Planka receives an explicit
`null`. `--due-date` and `--clear-due-date` are mutually exclusive.

### Move a card to a different list

```bash
plnk card move <cardId> --to-list <listId>
plnk card move 1234 --to-list 790 --position top
plnk card move 1234 --to-list 790 --position bottom
```

To move across boards, add `--to-board`:

```bash
plnk card move 1234 --to-board 456 --to-list 790
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

### Custom field values

```bash
plnk card field list <cardId>
plnk card field set <cardId> --group <id|name> --field <id|name> --value "specs/design.html"
plnk card field clear <cardId> --group <id|name> --field <id|name>
```

`--group` and `--field` accept an ID or a name, resolving through the card's attached groups
(and through the base group when the card's group was adopted from a template). Values are
capped at 512 characters and cannot be empty; `clear` is idempotent.

See [Custom fields](custom-fields.md) for the full model, including how to define a reusable
field template on a project and adopt it onto a card.

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
