# Live target model

`plnk-tui` subscribes to **exactly one board at a time** over the websocket. That board is called the **live target**. All live updates — card moves, comment additions, label changes, user actions — arrive for that board only. Every other board in the tree is static; you see whatever the last hydration loaded.

This is deliberate. A single live subscription keeps the protocol simple, the state machine local, and the memory footprint bounded.

## States

The session line and live-sync panel show one of these:

| State | Meaning |
|-------|---------|
| **no live target** | Idle. No websocket is running. Projects view is the startup default. |
| **loading** | A snapshot fetch is in flight (typically for an unloaded board being hydrated). |
| **connecting raw websocket** | Engine.IO handshake in progress for the target board. |
| **live websocket connected** | Subscribed and receiving events. |
| **error: …** | The socket died. A short reason follows. Press `L` on any board to re-establish on that board. |

## Toggling a board live

Select any board in the tree and press `L`.

- If that board is **not** already the live target, the TUI shuts down any previous websocket, updates `subscribed_board_id`, increments `active_socket_session_id`, and spawns a fresh listener for the new target.
- If that board **is** already the live target, pressing `L` again shuts the websocket down, clears `subscribed_board_id`, and returns the session to `no live target` / idle.

Status flows `Idle | Live → Connecting → Live` when subscribing, and `Live | Connecting | Error → Idle` when toggling the current board off. The selection in the tree can move freely afterward without affecting the subscription.

## Starting with a preselected board

Pass `--board <id>` (or set `PLNK_TUI_BOARD`) at startup. The listener spawns immediately and status begins at `loading` → `connecting` → `live`, bypassing the idle-on-startup path.

```bash
plnk-tui --board 1755733092435231757
```

This mode is the historical default, preserved for scripts and power users who already know which board they care about.

## Snapshots vs live events

The live subscription streams *deltas*. To reconstruct a full board from scratch, the TUI also requests a REST snapshot (`GET /api/boards/<id>`) and caches the result. Subsequent delta events are applied against that cache.

Consequence: if you switch live targets between two boards you've already visited, the second board reappears already populated — only the delta stream has to catch up. Unvisited boards go through a full snapshot fetch first.

## Debug log

Press `D` to toggle the websocket debug overlay. It shows recent engine-level events (`engineOpen`, `socketConnect`, `boardLive`, errors) with their raw summaries. Useful when a board "should be live but isn't" to see whether the connect/subscribe handshake stalled, or whether events are flowing but the UI isn't applying them.

## Troubleshooting

- **"socket connect failed"** — typically wrong server URL, no network, or the server is rejecting the Engine.IO upgrade. Verify `plnk auth status` works first.
- **Events flowing but tree doesn't update** — toggle `D`, look for unrecognized event names. The applier is intentionally narrow; new Planka event types may need code changes.
- **Need to stop live sync for the current board** — select that same board and press `L` again. This shuts the websocket down and returns the TUI to idle.
