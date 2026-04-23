# Memberships

Memberships control who has access to projects and boards. Project memberships and board memberships are managed through the same command with `--project` or `--board` flags.

## Commands

### List members

```bash
# Project members
plnk membership list --project <projectId>

# Board members
plnk membership list --board <boardId>
```

Exactly one of `--project` or `--board` must be provided.

### Add a member

```bash
# Add to project
plnk membership add --project <projectId> --user <userId>

# Add to board with a role
plnk membership add --board <boardId> --user <userId> --role editor
```

### Remove a member

```bash
# Remove from project
plnk membership remove --project <projectId> --user <userId>

# Remove from board
plnk membership remove --board <boardId> --user <userId>
```

## Examples

```bash
# See who's on a project
plnk membership list --project 123 --output json

# Add a user to a board
plnk membership add --board 456 --user 88 --role editor

# Remove a user from a project
plnk membership remove --project 123 --user 88
```
