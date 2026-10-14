# Names instead of IDs

By default `plnk` works with Planka's opaque IDs. The `--names` mode lets you
type names where the CLI expects IDs, and shows names in the output.

```bash
plnk --names board list --project "Platform"
```

The mode is on when either of these is true:

- the global `--names` flag is given, or
- `names = true` is set in `~/.config/plnk/config.toml`.

`--no-names` forces the mode off and overrides a config that sets
`names = true`. `--names` and `--no-names` cannot be combined.

## Input resolution

In names mode, foreign-key flags accept an ID or a name:

```bash
plnk --names list list --board "Sprint 42"
plnk --names card list --board "Sprint 42" --list "In Progress"
```

Resolution is ID-first, then the same three-tier match used by `find`:
exact case-sensitive, exact case-insensitive, then substring. A single match is
used. More than one match is an error (exit `2`) that lists the candidates, so
you can be more specific. No match is an error (exit `4`).

Card IDs are not resolved by name — `--card` and positional card IDs stay
ID-only.

## Output

Table and markdown output show the resolved name instead of the ID. JSON output
adds a sibling `*Name` field next to each foreign key, for example:

```json
{ "id": "1", "projectId": "42", "projectName": "Platform" }
```

The mode is extended resource by resource.

## Projects

`--project` accepts a project name wherever it is expected. JSON output adds
`projectName` next to `projectId`, and table/markdown output shows the name.

## Boards

`--board` and `--to-board` accept a board name. Board search is scoped to the
resolved `--project` when one is given, otherwise it searches every project.
JSON gains `boardName`; table/markdown show it.

## Lists

`--list` and `--to-list` accept a list name. List search is scoped to the
resolved `--board`, then to the boards of a resolved `--project`, otherwise it
searches every board. JSON gains `listName`; table/markdown show it.

## Users

`--user` and the positional user argument on `card assignee` accept a username.
Users are matched on their unique `username`. JSON gains `userName` (the
username); table/markdown show it.

## Labels

`--label` and the positional label argument on `card label add|remove` accept a
label name. Labels are scoped to a board when one is known, otherwise the
search spans every board. JSON gains `labelName`; table/markdown show it.
Unnamed labels have no name to show and keep their ID.

## Custom field groups

`--group`, `--base`, and `--base-group` accept a group name, as do the
positional IDs on `field-group get|update|delete`. Base groups are matched
against the projects list; ordinary groups against every board snapshot. An
adopted group takes its display name from its base group. JSON gains
`customFieldGroupName` and `baseCustomFieldGroupName`; table/markdown show
them.

## Custom fields

`--field` on `card field` already accepts a name and keeps doing so. Custom
field references in JSON gain `customFieldName`, and table/markdown show it.
Fields are only reachable through their group, so the resolver enumerates
groups to build the field cache.
