# Grammar reference

The shape of every `plnk` command and the rules that make it predictable for humans, scripts, and agents.

## Shape

```
plnk <resource> <action> [target] [flags]
```

Every command fits this form. No verb-first commands, no hidden actions — the grammar is the surface.

## Resources

`project`, `board`, `list`, `card`, `task`, `comment`, `label`, `attachment`, `membership`, `user`, `auth`

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
    label
  membership
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
```

## Stdout vs stderr

- **stdout** is data: table rows, JSON, markdown.
- **stderr** is logs, prompts, and errors.

Scripts piping `plnk ... --output json` into `jq` can rely on stdout being clean JSON.
