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
      "name": "Platform",
      "createdAt": "2026-04-14T12:00:00Z",
      "updatedAt": null
    }
  ],
  "meta": {
    "count": 1
  }
}
```

## Full field output

By default, table and JSON output shows trimmed fields (id, name). Use `--full` to include all fields:

```bash
plnk project list --full
plnk project list --output json --full
```
