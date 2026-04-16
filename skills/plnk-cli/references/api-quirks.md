# Planka API Quirks

Behaviors of the Planka REST API that affect how the CLI works. These are not bugs — they're how Planka is built. The CLI handles them, but understanding them helps when debugging or scripting.

## No direct GET for tasks, comments, or labels

Planka has no `GET /api/tasks/{id}`, `GET /api/comments/{id}`, or `GET /api/labels/{id}` endpoints. The CLI works around this by sending `PATCH` with an empty JSON body `{}`, which returns the item but silently bumps the `updatedAt` timestamp.

**Impact:** Any workflow that watches `updatedAt` for change detection will see phantom updates when someone runs `plnk task get`, `plnk comment get`, or `plnk label get`.

**Recommendation:** For read-only use, prefer `task list --card`, `comment list --card`, or `label list --board` which use proper GET endpoints and don't mutate timestamps.

## Board snapshot pattern

Many resources are not directly listable. Instead, the CLI fetches a parent "snapshot" that includes nested data:

- `GET /api/boards/{id}` returns `included.lists`, `included.cards`, `included.labels`, `included.boardMemberships`
- `GET /api/cards/{id}` returns `included.tasks`, `included.taskLists`, `included.cardLabels`, `included.cardMemberships`, `included.attachments`
- `GET /api/projects/{id}` returns `included.boards`, `included.projectManagers`

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
