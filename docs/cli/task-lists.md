# Task lists

Task lists are the named checklists on a card. Every task belongs to exactly one task list, and a card may carry several — "Design", "QA", "Release checklist". The Planka web UI shows each as its own checklist with its own name and order.

```
card
  task list ── task
  task list ── task
```

## Commands

### List task lists on a card

```bash
plnk task-list list --card <cardId>
plnk task-lists --card <cardId>             # alias
```

Sorted by position.

### Find task lists by name

```bash
plnk task-list find --card <cardId> --name QA
```

Three-tier matching: exact, then case-insensitive, then substring. Returns a collection.

### Read a task list

```bash
plnk task-list get <taskListId>
```

### Create a task list

```bash
plnk task-list create --card <cardId> --name QA
plnk task-list create --card <cardId> --name "Release checklist" --position top
plnk task-list create --card <cardId> --name Internal --show-on-front false --hide-completed true
```

| Flag | Values | Default |
|---|---|---|
| `--position` | `top`, `bottom`, or a number | `65536` |
| `--show-on-front` | `true` / `false` | not sent; the server defaults to `true` |
| `--hide-completed` | `true` / `false` | not sent; the server defaults to `false` |

`--show-on-front` controls whether the checklist appears on the card face in the web UI. An empty `--name` is rejected before any request is sent.

### Update a task list

```bash
plnk task-list update <taskListId> --name "QA sign-off"
plnk task-list update <taskListId> --hide-completed true
```

At least one of `--name`, `--show-on-front`, `--hide-completed` is required.

### Move a task list

```bash
plnk task-list move <taskListId> --to-position top
plnk task-list move <taskListId> --to-position 131072
```

### Delete a task list

```bash
plnk task-list delete <taskListId>
plnk task-list delete <taskListId> --yes
```

Deleting a task list deletes every task in it. The confirmation prompt says so.

## Checklists and tasks

`plnk task create` needs to know which checklist a task goes into:

| Task lists on the card | `task create --card <id>` without `--task-list` |
|---|---|
| none | creates one named "Tasks" and uses it |
| one | uses it |
| two or more | exits `2` and names every candidate |

Pass `--task-list <id|name>` to choose explicitly. See [Tasks](tasks.md).

## Examples

```bash
# Two checklists on one card
plnk task-list create --card 1234 --name Design
plnk task-list create --card 1234 --name QA

# Tasks go where you say
plnk task create --card 1234 --title "Wireframes" --task-list Design
plnk task create --card 1234 --title "Regression pass" --task-list QA

# Read one checklist
plnk task list --card 1234 --task-list QA --output json

# Tidy up
plnk task-list update <qaId> --hide-completed true
plnk task-list move <designId> --to-position top
```
