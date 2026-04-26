# Fast copy

`plnk-tui` can put the selected node's full ID hierarchy onto the system clipboard in one keystroke. The copied payload is structured so an AI agent (or you) can immediately operate on the node without re-prompting for IDs.

Two formats, mapped to two keys:

| Key | Format | Use when |
|-----|--------|----------|
| `y` | Compact JSON | You want raw structured data with no shell side effects. Hand it to an agent and let it decide what to fetch. |
| `Y` | Snapshot command + breadcrumb | You want a paste-ready command an agent (or human) can run immediately to load full state. |

The keys work from either pane and on any selected node — project, board, list, card, or label group. Label groups resolve to their underlying list.

## `y` — compact JSON

Single-line JSON containing one entry per hierarchy level, each with `id` and `name`. The keys are emitted in hierarchical order: `project`, `board`, `list`, `card`. Lower levels are omitted when not relevant to the selection.

Card selection:

```json
{"project":{"id":"175...","name":"planka-cli"},"board":{"id":"175...","name":"Work"},"list":{"id":"175...","name":"Backlog"},"card":{"id":"176...","name":"Fast COPY: copy ID hierarchy for AI context"}}
```

List selection (no `card` key):

```json
{"project":{"id":"175...","name":"planka-cli"},"board":{"id":"175...","name":"Work"},"list":{"id":"175...","name":"Backlog"}}
```

Names are JSON-escaped per RFC 8259, so a name containing `"` or `\n` is preserved safely. The clipboard payload contains only inert data — pasting it into a shell does nothing.

## `Y` — snapshot command + breadcrumb

Two lines: a `#` comment carrying a human-readable breadcrumb, followed by the most useful `plnk` command for that level. The command writes JSON to stdout, so it's directly pipeable.

| Selection | Emitted command |
|-----------|-----------------|
| Project   | `plnk project snapshot <id> --output json` |
| Board     | `plnk board snapshot <id> --output json` |
| List      | `plnk list get <id> --output json` |
| Card      | `plnk card snapshot <id> --output json` |

Card example:

```sh
# planka-cli > Work > Backlog > Fast COPY: copy ID hierarchy for AI context
plnk card snapshot 1761418906062291986 --output json
```

`card snapshot` is preferred over `card get` because it returns the card plus all included entities (tasks, comments, attachments, labels, memberships) in a single round trip — exactly what an agent needs for context.

## How it reaches the clipboard — OSC 52

The TUI writes the payload using the [OSC 52](https://www.xfree86.org/current/ctlseqs.html) terminal escape sequence: `ESC ] 52 ; c ; <base64> BEL`. The terminal itself sets the system clipboard. There is no native clipboard dependency, so:

- It works without X11, Wayland, AppKit, or Win32 clipboard APIs.
- It works **over SSH** as long as the local terminal honors OSC 52.
- It works inside tmux when `set -g set-clipboard on` is configured.

### Terminal compatibility

| Terminal | Status |
|----------|--------|
| iTerm2 | Works (default) |
| kitty  | Works (default) |
| alacritty | Works (default) |
| WezTerm | Works (default) |
| Windows Terminal | Works (default) |
| GNOME Terminal / VTE ≥ 0.50 | Works |
| tmux | Works with `set -g set-clipboard on` in `~/.tmux.conf` |
| Apple Terminal.app | **Not supported** — OSC 52 is silently dropped |
| Older xterms | Variable — check `xterm` allowed window ops |

If the paste comes up empty, your terminal does not honor OSC 52. There is currently no native fallback; future versions may add `arboard` behind a feature flag.

## Security

Project, board, list, and card names are user-editable on the Planka server. A maliciously crafted name like `"Nice card\nrm -rf ~"` could otherwise break out of the leading `#` comment line in the `Y` form and put attacker text on its own line, which would execute on shell paste.

The TUI strips control characters (C0, DEL, C1) from the breadcrumb before embedding it in the shell command form, replacing each with a single space. The `Y` payload is therefore always exactly two lines: one comment line and one `plnk` command line.

The `y` (JSON) form is unaffected — JSON's string escaping handles control characters by definition, and JSON is not directly evaluated by a shell.

## Workflow

The intended pattern: keep `plnk-tui` open in one window, your AI agent in another. When you want the agent to act on a node, select it in the TUI, press `y`, paste. The agent now has unambiguous IDs and can call `plnk` directly without asking you to disambiguate.
