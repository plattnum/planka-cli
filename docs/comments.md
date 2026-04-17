# Comments

Comments are text notes on a card. They support literal text, stdin, and file input.

## Commands

### List comments on a card

```bash
plnk comment list --card <cardId>
plnk comments --card <cardId>               # alias
```

### Read comments

Comments have no standalone `get` command — Planka has no direct GET endpoint for them, and the old PATCH-with-empty-body workaround silently bumped `updatedAt` on every read. Fetch comments through their parent card instead:

```bash
plnk comment list --card <cardId>           # all comments on a card (with full text)
plnk card snapshot <cardId> --output json   # whole card (comments come via the list endpoint, not `included`)
```

### Create a comment

```bash
# Literal text
plnk comment create --card <cardId> --text "Starting work on this"

# From a file
plnk comment create --card 1234 --text @status-update.md

# From stdin
echo "Blocked on API changes" | plnk comment create --card 1234 --text -

# From clipboard (macOS)
pbpaste | plnk comment create --card 1234 --text -
```

### Update a comment

```bash
plnk comment update <commentId> --text "Updated status: unblocked"
plnk comment update 9012 --text @revised-notes.md
```

### Delete a comment

```bash
plnk comment delete <commentId>
plnk comment delete 9012 --yes
```

## Examples

```bash
# Add a work log
plnk comment create --card 1234 --text "Completed API refactor. Tests passing."

# Post a multi-line comment from heredoc
cat <<EOF | plnk comment create --card 1234 --text -
## Status Update
- API refactor complete
- Tests: 47 passing
- Ready for review
EOF

# List all comments as JSON
plnk comment list --card 1234 --output json
```
