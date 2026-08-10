# Custom Fields

Custom fields are the only place in Planka where a card can carry **structured, named metadata** rather than free prose. A spec URL, an external ticket id, an owner ruling date — anything that is a named attribute — has nowhere else to live except buried in a description.

Three resources are involved:

- **`plnk field-group`** — the groups that hold fields
- **`plnk field`** — the named slots inside a group
- **`plnk card field`** — the values a card stores for those slots

## The model

```
project ── base custom field group ── custom field      (the reusable template)
                                          │
board  ─────── custom field group ── custom field       (board-local definition)
                                          │
card   ─────── custom field group ── custom field       (card-local definition)
   └────────── custom field value   (card × group × field → string)
```

A group on a **project** is a *base group*: a reusable template. A card adopts one by creating a card-level group that carries `baseCustomFieldGroupId`. A group created without that id is a one-off, local to whatever it hangs from.

Two consequences of adopting a template are worth knowing before you script against this:

- An adopted card group has **`name: null`**. Its display name lives on the base group.
- An adopted card group carries **no fields of its own**. The fields belong to the base group.

`plnk` hides both when you use names — `--group Documentation` resolves through the base group — but they show up plainly in JSON output.

## What custom fields are and are not

They make a value **visible and structured**. They do not make it **queryable**:

- `content` is capped at **512 characters**, so a card governed by a dozen documents cannot list them all in one field.
- **There is no server-side filter on custom field values.** No Planka endpoint filters cards by field value. Filtering means pulling a snapshot and filtering client-side, exactly as if the value lived in the description.

Use them for the one or two canonical pointers, not as a general-purpose index.

## Field groups

### List groups

```bash
plnk field-group list --project <projectId>   # base groups (the templates)
plnk field-group list --board <boardId>       # groups on a board
plnk field-group list --card <cardId>         # groups on a card
plnk field-groups --card <cardId>             # alias
```

`--project` returns base groups; `--board` and `--card` return ordinary groups. These are different types with different columns.

### Find groups by name

```bash
plnk field-group find --project <projectId> --name "Documentation"
plnk field-group find --card <cardId> --name "Docs"
```

### Get a group

```bash
plnk field-group get <groupId>
```

Accepts either kind of ID. Base groups and ordinary groups live behind different routes, so `get` tries the ordinary route first and falls back to the base route.

### Create a group

```bash
# A reusable template on a project
plnk field-group create --project <projectId> --name "Documentation"

# A board-local group
plnk field-group create --board <boardId> --name "Properties"

# Adopt a template onto a card
plnk field-group create --card <cardId> --base <baseGroupId>

# A one-off group on a card
plnk field-group create --card <cardId> --name "Ad-hoc"
```

On `--card`, exactly one of `--base` or `--name` is required. Passing neither exits `2`.

Attaching a base group to every card on a board is a scripted loop, not a CLI primitive.

### Update and delete

```bash
plnk field-group update <groupId> --name "Docs"
plnk field-group delete <groupId> --yes
```

## Fields

### List and find fields

```bash
plnk field list --base-group <baseGroupId>
plnk field list --group <groupId>
plnk fields --base-group <baseGroupId>          # alias

plnk field find --base-group <baseGroupId> --name "Spec"
```

Asking an **adopted** card group for its fields returns nothing — the fields belong to its base group. Ask the base group instead.

### Create a field

```bash
plnk field create --base-group <baseGroupId> --name "Specification" --show-on-front
plnk field create --group <groupId> --name "Implementation Plan"
```

`--show-on-front` controls whether the value appears on the front of the card in the Planka web UI. It is off by default, matching Planka. It is the difference between a field a human sees at a glance and one only a script reads.

### Update and delete

```bash
plnk field update <fieldId> --name "Spec"
plnk field update <fieldId> --show-on-front false
plnk field delete <fieldId> --yes
```

On `update`, `--show-on-front` takes an explicit `true` or `false`, because "leave unchanged" and "set to false" have to stay distinguishable.

## Card field values

```bash
plnk card field list <cardId>
plnk card field set <cardId> --group <id|name> --field <id|name> --value "specs/design.html"
plnk card field clear <cardId> --group <id|name> --field <id|name>
```

`--group` and `--field` accept an **ID or a name**, following the precedent set by `card label add`. Names resolve within the card's own attached groups, reaching through to the base group where needed. Use an ID to avoid ambiguity.

Resolution follows the house three-tier match — exact case-sensitive, then case-insensitive, then substring — stopping at the first tier with results:

- no match → `ResourceNotFound`, exit `4`
- more than one match in the winning tier → validation error naming every candidate, exit `2`

### Value rules

| Situation | Result |
|---|---|
| value longer than 512 characters | exit `2`, no request sent |
| empty value | exit `2`, message points at `card field clear` |
| clearing an already-unset value | exit `0` — clearing is idempotent |

There is no "set to empty": Planka rejects an empty string outright, so clearing is a distinct operation rather than a value you can assign.

## Worked example

Point a card at the documents that govern it:

```bash
# 1. Define the template once, on the project
plnk field-group create --project 123 --name "Documentation"
# → returns the base group id, e.g. 777

# 2. Add fields to the template
plnk field create --base-group 777 --name "Specification" --show-on-front
plnk field create --base-group 777 --name "Implementation Plan" --show-on-front

# 3. Adopt the template onto a card
plnk field-group create --card 1234 --base 777

# 4. Fill in the values — by name, not id
plnk card field set 1234 --group "Documentation" --field "Specification" \
                         --value "specs/2026-08-06-design.html"
plnk card field set 1234 --group "Documentation" --field "Implementation Plan" \
                         --value "specs/2026-08-06-plan.md"

# 5. Read them back
plnk card field list 1234 --output json

# 6. Clear one
plnk card field clear 1234 --group "Documentation" --field "Specification"
```

## JSON output

Output is a strict serde projection — wire spelling and nulls intact. An adopted group is recognisable by its null `name` and non-null `baseCustomFieldGroupId`:

```bash
plnk field-group list --card 1234 --output json
```

```json
{
  "success": true,
  "data": [
    {
      "id": "1838340027382236525",
      "name": null,
      "baseCustomFieldGroupId": "1838336514124154209",
      "cardId": "1838330705323492688",
      "boardId": null,
      "position": 65536.0
    }
  ],
  "meta": { "count": 1 }
}
```

Add `--full` for every field, including `createdAt` and `updatedAt`.
