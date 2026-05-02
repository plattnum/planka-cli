# Authentication

`plnk` stores automation/AI-oriented CLI credentials separately from `plnk-tui`'s human login hints. Running `plnk init` configures the CLI only; `plnk-tui` has its own first-run prompt and config.

## Precedence

Credentials are resolved in this order — first match wins:

| Priority | Method | Server | Token |
|----------|--------|--------|-------|
| 1 | CLI flags | `--server <url>` | `--token <token>` |
| 2 | Environment | `PLANKA_SERVER` | `PLANKA_TOKEN` |
| 3 | Config file | `~/.config/plnk/config.toml` | `~/.config/plnk/config.toml` |

The config file location honors `XDG_CONFIG_HOME` on every OS and can be overridden with `PLANKA_CONFIG=<path>`. On Unix, the file is written with `0600` permissions.

Planka uses an `X-API-Key` header under the hood for `plnk` API-token calls — not `Bearer`, not `Authorization`.

## Interactive bootstrap

First-time setup:

```bash
plnk init
```

Walks through server URL, API token (masked), and optional HTTP tuning. Re-running is safe — existing values are shown as defaults.

## Interactive login (email + password)

Exchanges credentials for a token and stores it in the config:

```bash
plnk auth login --server https://planka.example.com
# Prompts for email and password
```

## Direct token (for CI or pre-existing API keys)

```bash
plnk auth token set <token> --server https://planka.example.com
```

## Environment variables (stateless, for CI)

```bash
export PLANKA_SERVER=https://planka.example.com
export PLANKA_TOKEN=your-api-key
plnk project list
```

## Other auth commands

```bash
plnk auth whoami                    # show current user
plnk auth status                    # show credential source + validity
plnk auth logout                    # delete stored credentials
```

`status` always exits `0` (informational). `whoami` exits `3` on auth failure, making it useful for scripts that need to verify credentials before proceeding.

## `plnk-tui` auth is separate

`plnk-tui` is for an interactive human session and does not implicitly read this CLI config. On first run it prompts for server, username, and password, then can save only the non-secret server + username to `~/.config/plnk-tui/config.toml`. It never saves the password.
