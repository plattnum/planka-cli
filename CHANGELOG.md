# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-02

### Changed
- **Breaking** — `plnk-tui` auth is now fully separate from `plnk` CLI auth.
  - Env vars renamed: `PLANKA_SERVER` / `PLANKA_USERNAME` / `PLANKA_PASSWORD` → `PLNK_TUI_SERVER` / `PLNK_TUI_USERNAME` / `PLNK_TUI_PASSWORD`. The `PLANKA_SERVER` / `PLANKA_TOKEN` env vars used by `plnk` itself are unchanged.
  - The TUI no longer reads `plnk`'s `~/.config/plnk/config.toml`. It now uses its own `~/.config/plnk-tui/config.toml` (mode `0600` on Unix), which stores only non-secret server + username. Passwords are never persisted.
  - Rationale: `plnk` is automation/AI/API-token oriented; `plnk-tui` is for an interactive human session. Sharing one credential store for two very different access patterns was a footgun, and the prior overlap implied the TUI could be driven from a CLI API token (it can't — it needs a username + password to authenticate over the same endpoint as `plnk auth login`).
  - Migration: rename any `PLANKA_*` env vars in your `plnk-tui` invocations to `PLNK_TUI_*`, or just launch `plnk-tui` and accept the first-run save prompt.

### Added
- `plnk-tui`: first-run interactive prompts for server, username/email, and password when no flags, env vars, or config are present. After a successful login the TUI offers to save only the non-secret server + username to `~/.config/plnk-tui/config.toml`.

### Fixed
- `plnk-tui`: fast-copy (`y` / `Y`) on a label group node now includes the label's id and name in the JSON payload and the breadcrumb. The `Y` form for a label group inside a list now generates a scoped `plnk card find --list <list-id> --label <label-id> --output json` command instead of a plain list snapshot, so the pasted command actually filters to the labeled cards. Previously the label identity was silently dropped from both forms.

## [0.1.3] - 2026-04-26

### Added
- `plnk-tui`: `y` and `Y` keybinds copy the selected node's full ID hierarchy to the system clipboard via OSC 52. `y` copies compact JSON; `Y` copies a paste-ready `plnk <resource> snapshot --output json` command preceded by a breadcrumb comment. Works locally, inside tmux (with `set -g set-clipboard on`), and over SSH without a native clipboard dependency. Built for handing the selected node off to an AI agent in one keystroke. ([#2])

### Security
- `plnk-tui`: Strip control characters (C0, DEL, C1) from the breadcrumb embedded in the `Y` clipboard form to prevent newline injection from user-controlled Planka resource names breaking out of the leading `#` comment line on shell paste. ([#2])

## [0.1.2] - 2026-04-25

### Added
- `plnk-tui`: Client-side explorer tree filter (`/`) — case-insensitive substring match, plus `*` and `?` glob wildcards. ([#1])

## [0.1.1] - 2026-04-24

First release published via [cargo-dist](https://opensource.axo.dev/cargo-dist/). Prebuilt archives, shell installer, and SHA-256 checksums on GitHub Releases for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`, and `x86_64-pc-windows-msvc`.

### Added
- `plnk-tui`: Manual hierarchy refresh keybinding (`r` / `R`).
- `plnk-tui`: `L` toggles the live websocket subscription on the selected board.

[#1]: https://github.com/plattnum/planka-cli/pull/1
[#2]: https://github.com/plattnum/planka-cli/pull/2
[Unreleased]: https://github.com/plattnum/planka-cli/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/plattnum/planka-cli/compare/v0.1.3...v0.2.0
[0.1.3]: https://github.com/plattnum/planka-cli/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/plattnum/planka-cli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/plattnum/planka-cli/releases/tag/v0.1.1
