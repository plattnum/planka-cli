# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
[Unreleased]: https://github.com/plattnum/planka-cli/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/plattnum/planka-cli/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/plattnum/planka-cli/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/plattnum/planka-cli/releases/tag/v0.1.1
