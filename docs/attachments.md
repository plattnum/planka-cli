# Attachments

Attachments are files uploaded to a card. Upload local files, download with the original filename, or delete.

## Commands

### List attachments on a card

```bash
plnk attachment list --card <cardId>
plnk attachment list --card 1234 --output json
```

### Upload a file to a card

```bash
plnk attachment upload --card <cardId> <file>
plnk attachment upload --card 1234 ./spec.png
plnk attachment upload --card 1234 /path/to/report.pdf
```

### Download an attachment

Downloads the file using its original filename from Planka. Requires `--card` to look up the attachment metadata.

```bash
# Download to current directory with original filename
plnk attachment download <attachmentId> --card <cardId>

# Download to a specific path
plnk attachment download <attachmentId> --card <cardId> --out ./downloads/spec.png
```

Without `--out`, the file saves to the current directory using whatever name it was uploaded with.

### Delete an attachment

```bash
plnk attachment delete <attachmentId>
plnk attachment delete 555 --yes
```

## Examples

```bash
# Upload a spec document
plnk attachment upload --card 1234 ./design-spec.pdf

# See what's attached
plnk attachment list --card 1234 --output json

# Download it (saves as "design-spec.pdf" in current dir)
plnk attachment download 555 --card 1234

# Download to a specific location
plnk attachment download 555 --card 1234 --out ~/Downloads/design-spec.pdf

# Clean up
plnk attachment delete 555 --yes
```
