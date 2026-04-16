# Users

Users are read-only in the CLI. List all users or get a specific user by ID.

## Commands

### List all users

```bash
plnk user list
plnk user list --output json
```

### Get a user by ID

```bash
plnk user get <userId>
plnk user get 88 --output json
plnk user get 88 --full
```

## Examples

```bash
# Find a user's ID for assignee/membership commands
plnk user list --output json

# Get full user details
plnk user get 88 --output json --full
```
