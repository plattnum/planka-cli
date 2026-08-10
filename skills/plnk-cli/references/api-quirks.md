# Planka API Quirks

Behaviors of the Planka REST API that affect how the CLI works. These are not bugs — they're how Planka is built. The CLI handles them, but understanding them helps when debugging or scripting.

## No direct GET for tasks, comments, or labels

Planka has no `GET /api/tasks/{id}`, `GET /api/comments/{id}`, or `GET /api/labels/{id}` endpoints. An earlier version of the CLI worked around this by sending `PATCH` with an empty JSON body `{}` — which returned the item but silently bumped the `updatedAt` timestamp, breaking any downstream change-detection.

**Resolution:** The `plnk task get`, `plnk comment get`, and `plnk label get` commands were removed. These resources live inside parents (tasks and comments inside a card, labels inside a board) and Planka itself never exposes them by independent identity. Read them through their parent:

```bash
plnk task list --card <cardId>              # tasks on a card
plnk comment list --card <cardId>           # comments on a card
plnk label list --board <boardId>           # labels on a board
plnk card snapshot <cardId> --output json   # whole card (tasks under included.tasks)
plnk board snapshot <boardId> --output json # whole board (labels under included.labels)
```

The parent listing paths use proper GET endpoints and don't mutate `updatedAt`.

## Custom fields: adopted groups have no name and no fields of their own

The single most confusing part of the custom field model.

A card adopts a project-level template (a *base group*) by creating a card-level group that
carries `baseCustomFieldGroupId`. That adopted group has:

- **`name: null`** — its display name lives on the base group
- **no fields of its own** — `GET /api/custom-field-groups/{adoptedId}` returns
  `included.customFields: []`, because the fields belong to the base group

So `plnk field list --group <adoptedCardGroupId>` correctly returns **zero** fields. Ask the
base group instead:

```bash
plnk field-group list --card <cardId> --output json   # note name: null, baseCustomFieldGroupId set
plnk field list --base-group <baseGroupId>            # the fields actually live here
```

`plnk card field set/clear` hides all of this — `--group Documentation` resolves through the
base group — so prefer names there and only drop to IDs to break ambiguity.

## Custom fields: base groups are only partly routed

There is **no `GET /api/base-custom-field-groups/{id}`**. That path falls through to the Planka
SPA and returns HTML with HTTP `200` — worse than a 404, since a JSON client fails with a parse
error rather than a clean not-found. `PATCH` and `DELETE` on the same prefix *do* exist.

Base groups are readable only through `GET /api/projects` or `GET /api/projects/{id}`, under
`included.baseCustomFieldGroups`. The board snapshot has no `baseCustomFieldGroups` key at all,
and its `included.customFields` covers only board- and card-level group fields.

`plnk field-group get <id>` handles this by falling back to the base route, so scripts can pass
any group ID without tracking its kind.

## Custom fields: value rules

- `content` is capped at **512 characters**. `plnk` rejects over-length client-side with exit
  `2` and sends no request.
- **Empty strings are rejected by the server.** There is no "set to empty" — clearing is
  `plnk card field clear`, which maps to a DELETE. `plnk` exits `2` on `--value ""` and says so.
- Clearing is **idempotent**: an already-unset value exits `0`.
- **No endpoint filters cards by custom field value.** Do not look for one. Filter client-side:

```bash
plnk card snapshot <cardId> --output json \
  | jq '.included.customFieldValues[] | select(.content | test("pattern"))'
```

- The server may **rewrite a group's `position`** on create. Never assert the echoed value.

## Board snapshot pattern

Many resources are not directly listable. Instead, the CLI fetches a parent "snapshot" that includes nested data:

- `GET /api/boards/{id}` returns `included.lists`, `included.cards`, `included.labels`, `included.boardMemberships`
- `GET /api/cards/{id}` returns `included.tasks`, `included.taskLists`, `included.cardLabels`, `included.cardMemberships`, `included.attachments`, `included.customFieldGroups`, `included.customFields`, `included.customFieldValues`
- `GET /api/projects/{id}` returns `included.boards`, `included.projectManagers`, `included.baseCustomFieldGroups`, `included.customFields`

This means listing labels on a board actually fetches the entire board snapshot. Listing tasks on a card fetches the entire card snapshot. The CLI extracts what it needs.

## Creation requires type fields

When creating certain resources via the API, a `type` field must be included:

- List creation: `"type": "active"`
- Card creation: `"type": "project"`
- Board creation: `"type": "kanban"`

The CLI handles this automatically. When scripting directly against the API, these must be included.

## Position values

Planka uses floating-point positions for ordering. The convention is powers of 2 starting at 65536:

- First item: 65536
- Second item: 131072
- Third item: 196608

When moving cards or lists, provide a position value. The CLI's `--position top` maps to 0.0 and `--position bottom` maps to a very large float.

## Attachment download URLs

Attachment metadata in card snapshots includes `data.url` with the full download URL:

```
http://host:port/attachments/{id}/download/{filename}
```

Planka routes downloads by attachment ID only — the filename segment is decorative. However, the CLI uses the real URL from the card snapshot to be correct.

## Comments endpoint

Comments use `GET /api/cards/{cardId}/comments` for listing and `POST /api/cards/{cardId}/comments` for creation. Note: the endpoint is `/comments`, not `/comment-actions` (which is a different Planka endpoint for activity tracking).

## Auth header

Planka uses `X-API-Key` header, not `Authorization: Bearer`. The CLI handles this automatically.

## Card find across scopes

- `find --list` fetches `GET /api/lists/{id}/cards` (single API call)
- `find --board` fetches `GET /api/boards/{id}` and searches the board snapshot's included cards (single API call)
- `find --project` fetches the project snapshot for board IDs, then fetches each board snapshot (N+1 API calls where N = number of boards)

For performance, prefer the narrowest scope possible.

## Empty collections

Empty collections are valid responses, not errors:

```json
{"success": true, "data": [], "meta": {"count": 0}}
```

A `find` with no matches returns an empty collection with exit code 0.
