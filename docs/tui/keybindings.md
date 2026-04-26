# `plnk-tui` keybindings

A flat reference. The footer at the bottom of the TUI shows a short subset of the most relevant keys for the current mode; this page is the canonical list.

## Global

| Key | Action |
|-----|--------|
| `Ctrl-c` | Quit immediately. Works in every mode, including while saving. |
| `Tab` | Cycle focus between the explorer pane and the details pane. |
| `y` | Copy the selected node's hierarchy as compact JSON to the system clipboard via OSC 52. See [fast-copy.md](fast-copy.md). |
| `Y` | Copy the selected node's hierarchy as a paste-ready `plnk` snapshot command (with breadcrumb comment) to the system clipboard. |
| `D` | Toggle the websocket debug log overlay. |

## Explorer pane

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection up/down through visible tree rows. |
| `→` / `Enter` | Expand the selected node. For unloaded boards, this triggers a lazy snapshot load. |
| `←` | Collapse the selected node, or move to its parent if already collapsed. |
| `/` | Enter explorer filter mode. Type a case-insensitive substring, or use `*` / `?` as glob wildcards. `Enter` keeps the filter active; `Esc` clears/closes. |
| `v` | Toggle the explorer view between **hierarchy** (project → board → list → card) and **labels** (board → label groups). |
| `r` / `R` | Refresh the hierarchy below the selected node. On a project, this refetches the project tree and any already-loaded boards under it. On a board/list/card, this refetches that board snapshot; card refresh also reloads comments. |
| `L` | Toggle the selected board as the live target. Press once to subscribe, press again on the same board to unsubscribe and return to idle. See [live-target.md](live-target.md). |

## Details pane

Focus with `Tab` first.

| Key | Action |
|-----|--------|
| `↑` / `↓` | Scroll the detail pane one line. |
| `PageUp` / `PageDown` | Scroll by ten lines. |
| `Ctrl-u` / `Ctrl-d` | Alternative half-page scroll. |
| `g` / `G` | Jump to the top / bottom of the details. |
| `e` | Enter **title-edit mode** on the selected card. |
| `E` | Open the card's description in `$EDITOR`. The TUI suspends, waits for the editor to exit, then saves the new description. |

## Title-edit mode

Entered with `e` from the details pane while a card is selected.

| Key | Action |
|-----|--------|
| Any printable char | Insert at cursor. |
| `←` / `→` | Move cursor within the title. |
| `Home` / `End` | Jump to start / end of title. |
| `Backspace` | Delete char before cursor. |
| `Delete` | Delete char at cursor. |
| `Enter` | Save the new title to the server. |
| `Esc` | Cancel, restore the previous title. |

## Saving mode

When a card save is in flight the TUI disables most keys until the server responds. Only `Ctrl-c` (force quit) stays live.

## Empty state

When no live target has been promoted yet, the live-sync panel shows `live target: none — select a board and press L to promote it`. No websocket activity happens until you press `L`.
