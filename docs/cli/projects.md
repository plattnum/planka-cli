# Projects

Projects are the top-level container in Planka. Everything else lives under a project.

## Commands

### List all projects

```bash
plnk project list
plnk project list --output json
```

### Get a project by ID

```bash
plnk project get <projectId>
plnk project get 123 --output json
```

### Find projects by name

```bash
plnk project find --name "Platform"
plnk project find --name platform --output json
```

Uses three-tier matching (exact case-sensitive → case-insensitive → substring); stops at the first tier with results. Unlike every other `find` command, `project find` takes no parent scope because projects are the root resource.

### Get a project snapshot

```bash
plnk project snapshot <projectId> --output json
```

Returns the full `GET /api/projects/{id}` response verbatim, including `item` (the project) and `included` (boards, boardMemberships, projectManagers, users, customFields, notificationServices, backgroundImages, baseCustomFieldGroups). Nothing is dropped — fields we don't model (custom fields, notification services, etc.) still pass through.

JSON only. `--output table` and `--output markdown` fail with exit code 2 because the nested heterogeneous shape has no natural tabular rendering.

### Create a project

```bash
plnk project create --name "Platform"
plnk project create --name "Platform" --output json
```

### Update a project

```bash
plnk project update <projectId> --name "Platform Core"
plnk project update 123 --name "Renamed Project"
```

At least one field must be provided.

### Delete a project

```bash
plnk project delete <projectId>
plnk project delete 123
plnk project delete 123 --yes    # skip confirmation
```

## JSON output

```bash
plnk project list --output json
```

```json
{
  "success": true,
  "data": [
    {
      "id": "123",
      "name": "Platform"
    }
  ],
  "meta": {
    "count": 1
  }
}
```

## Full field output

By default, JSON output is a strict projection of the full wire format to a curated set of fields (for projects: `id`, `name`). Field names and types match `--full` exactly — trimmed is a subset, never a translation. Use `--full` to include every field Planka returns:

```bash
plnk project list --full
plnk project list --output json --full
```
