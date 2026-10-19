# Grammar reference

The shape of every `plnk` command and the rules that make it predictable for humans, scripts, and agents.

## Shape

```
plnk <resource> <action> [target] [flags]
```

Every command fits this form. No verb-first commands, no hidden actions — the grammar is the surface.

## Resources

`project`, `board`, `list`, `card`, `task`, `comment`, `label`, `field-group`, `field`, `attachment`, `membership`, `user`, `auth`

Each has its own per-resource docs in [`docs/cli/`](.).

## Hierarchy

```
project
  board
    list
      card
        task
        comment
        attachment
        custom field group ── custom field ── value
    label
    custom field group ── custom field
  membership
  base custom field group ── custom field
```

All scoped queries follow this hierarchy. You can't list cards without a list (or board / project for `find`). You can't list tasks without a card. Sole exception: `project find` is unscoped because projects are the root.

## Search matching

`find` commands use three-tier matching, stopping at the first tier with results:

1. Exact case-sensitive match
2. Exact case-insensitive match
3. Substring case-insensitive match

`find` always returns a collection — never an error for multiple results.

`get` is different: it takes an opaque ID and fails with exit code `4` if not found.

## Output formats

`--output {table,json,markdown}` on every command. Default is `table`.

**Table** — human-readable, trimmed to essential fields. Add `--full` for every field.

**JSON** — structured envelope for scripting:

```json
{
  "success": true,
  "data": [{"id": "123", "name": "Platform"}],
  "meta": {"count": 1}
}
```

The JSON is a strict projection of the internal serde model — identical keys, types, and nulls.

**Markdown** — for reports and notes.

## JSON errors

Errors produce a structured envelope too:

```json
{
  "success": false,
  "error": {
    "type": "ResourceNotFound",
    "message": "Resource not found: card 999"
  }
}
```

## Machine-readable help

```bash
plnk card create --help --output json
```

Returns `{resource, action, summary, args, options}` with each option's `{type, required, description}`. Agents can introspect the command surface before ever running it. See [`examples.md`](examples.md#agent-friendly-help) for a sample.

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `2` | Invalid arguments / validation error |
| `3` | Authentication failure |
| `4` | Resource not found |
| `5` | Remote API / server error |

## Text input for descriptions and comments

Flags like `--description`, `--text`, and similar accept three input forms:

| Syntax | Meaning |
|--------|---------|
| `"literal text"` | Inline text |
| `-` | Read from stdin |
| `@file.md` | Read from file |

Example:

```bash
pbpaste | plnk card update <cardId> --description -
plnk card update <cardId> --description @spec.md
```

## Timestamps

Flags that take a timestamp, such as `card find --due-after`, `--due-before`,
and `card update --due-date`, accept ISO 8601 and a set of relative forms.

### ISO 8601

| Form | Meaning |
|------|---------|
| `YYYY-MM-DD` | Midnight in your local timezone on that date |
| `YYYY-MM-DDThh:mm` | Date-time, UTC when no offset is given |
| `YYYY-MM-DDThh:mm:ss` | Date-time with seconds |
| `YYYY-MM-DDThh:mm:ss.fff` | Date-time with fractional seconds |
| `...Z` | UTC offset |
| `...±hh:mm` | Explicit numeric offset |

A date-time without an offset is treated as UTC; a bare date resolves to local
midnight, like `today`. A malformed timestamp is a validation error and exits
`2`.

### Relative dates

Relative forms resolve against the current time in your **local** timezone, which
`plnk` reads from the environment (`TZ` or `/etc/localtime`). When the offset
cannot be determined, `plnk` falls back to UTC.

| Form | Meaning |
|------|---------|
| `now` | The exact reference instant, time of day included |
| `today`, `tomorrow`, `yesterday` | That calendar day |
| `in 3 days`, `in 2 weeks`, `in 1 month` | Calendar arithmetic forward |
| `2 weeks from today` | Same as `in 2 weeks` |
| `3 days ago`, `2 months ago` | Calendar arithmetic into the past |
| `next week`, `next month`, `next year` | One calendar unit forward |
| `last week`, `last month`, `last year` | One calendar unit back |
| `this week`, `this month`, `this year` | Start of the current unit |
| `monday` .. `sunday` | Next occurrence of that weekday |

Rules:

- Every relative form **except `now`** resolves to the **start of the day**
  (00:00:00) in your local timezone. `now` resolves to the reference instant
  itself, keeping the current time of day, so `--due-before now` selects overdue
  cards and `--due-after now` selects cards due in the future. Because the
  bounds are strict, `--due-before tomorrow` still includes every card due
  today: each of those instants falls before tomorrow's local midnight. A card
  due exactly at tomorrow's midnight is excluded.
- Weekday names always mean the *next* occurrence. `monday` on a Monday means the
  Monday a week later, never today.
- Calendar quantities clamp to the target month's length: `in 1 month` from 31
  January is 28 or 29 February. `next month` means the *start* of the next
  month, so from 31 January it is 1 February.
- Multi-word forms need quoting: `--due-before '2 weeks from today'`.
- Unrecognised values exit `2`. The error names the flag and repeats both
  grammars.

Because relative dates depend on the current time, two runs across a DST change
can legitimately return different results for the same input.

### Closed cards

A due query — any `card list` or `card find` with `--due-after` or `--due-before`
— excludes cards with `isClosed: true`. A closed card is done, so it does not
belong in an answer about what is still due.

The exclusion is tied to a bound being set. Without one, a listing is unchanged
and keeps returning closed cards.

| Flag | Effect |
|------|--------|
| `--include-closed` | Keep closed cards in a due query |
| `--closed` | Keep only closed cards, with or without a due window |

`--closed` and `--include-closed` contradict each other. Passing both exits `2`
before any request is made.

## Global flags

Every subcommand accepts these (via clap's global flags):

| Flag | Description |
|------|-------------|
| `--server <url>` | [Planka](https://planka.app) server URL |
| `--token <token>` | API token |
| `--output table\|json\|markdown` | Output format (default: `table`) |
| `-v` / `-vv` / `-vvv` | Verbosity: info / debug / trace (logs to stderr) |
| `--quiet` | Suppress all output |
| `--no-color` | Disable colored output |
| `--yes` | Skip confirmation prompts |
| `--full` | Show all fields (default is trimmed) |
| `--http-max-in-flight <n>` | Max in-flight HTTP requests |
| `--http-rate-limit <rps>` | Sustained request rate |
| `--http-burst <n>` | Rate-limit burst size |
| `--retry-attempts <n>` | Retry attempts after the initial request |
| `--retry-base-delay-ms <ms>` | Base retry delay |
| `--retry-max-delay-ms <ms>` | Max retry delay |
| `--no-retry` | Disable automatic HTTP retries |

For the full transport policy semantics see [transport.md](transport.md).

## Plural aliases

Convenience shortcuts for listing resources. Hidden from `--help`, identical output to the canonical form:

```bash
plnk boards --project <id>         # → plnk board list --project <id>
plnk lists --board <id>            # → plnk list list --board <id>
plnk cards --list <id>             # → plnk card list --list <id>
plnk cards --board <id>            # → plnk card list --board <id>
plnk tasks --card <id>             # → plnk task list --card <id>
plnk comments --card <id>          # → plnk comment list --card <id>
plnk labels --board <id>           # → plnk label list --board <id>
plnk field-groups --card <id>      # → plnk field-group list --card <id>
plnk fields --base-group <id>      # → plnk field list --base-group <id>
```

## Stdout vs stderr

- **stdout** is data: table rows, JSON, markdown.
- **stderr** is logs, prompts, and errors.

Scripts piping `plnk ... --output json` into `jq` can rely on stdout being clean JSON.
