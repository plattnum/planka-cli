---
layout: post
title: "A companion, not a replacement: plnk-tui"
description: "plnk-tui is a small terminal UI for Planka — a tree view of project → board → list → card → task that lives next to the agent, not in place of the Planka web UI."
date: 2026-05-10
permalink: /blogs/a-companion-not-a-replacement.html
---

# A companion, not a replacement: plnk-tui

[plnk-tui](https://plattnum.github.io/planka-cli/)  is plnk's TUI companion — a live (Terminal UI) explorer for [Planka](https://planka.app/), with real-time websocket sync.

I didn't set out to build a [Planka](https://planka.app/) client. [Planka](https://planka.app/) already has a very polished, responsive web UI that I open and use every day to manage my work.

But as I've been working more in partnership with coding agents, I find myself in the terminal more day-to-day — and I've been adopting all sorts of TUIs and CLIs to avoid context-switching out to a GUI.

I had no idea any of this existed. TUIs — Terminal User Interfaces — feel like a whole subculture I'd somehow walked past for years. Active communities, mature frameworks in Python and Rust, a long tail of polished tools like [LazyGit](https://lazygit.dev/), [LazyDocker](https://lazydocker.com/), [k9s](https://k9scli.io/). I lost myself in them for days.

I was building [plnk](https://plattnum.github.io/planka-cli/) and using it on my own work. Eating my own dog food. For weeks the loop was the same. I'd go look at the [Planka](https://planka.app/) web UI, navigate down through project, board, and list, find the cards I wanted the agent to handle, and give it commands like:

> *On project ABC, find the Work Board, and in the Backlog list let's find the card with the title that starts with "assess the byz\*".*

> *On the Work Board, move "Refactor session token storage" from In Progress to Review.*

> *Find the cards labeled `AuthSystem` in the Backlog on the Work Board, scan them and prepare for questions.*

Then the agent would finish a card. I'd switch back to the browser, scroll the board, pick the next card, parse it myself,  switch back, type in the next prompt.  Then again. And again.

My work on [plnk](https://plattnum.github.io/planka-cli/) had already built an efficient command surface between my agent — running in the terminal — and my [Planka](https://planka.app/) instance: a thin, strict CLI that turned [Planka](https://planka.app/)'s REST API into a command language the agent could actually use. (More on the why in [the previous post](/blogs/the-long-way-back-to-planka.html).)

But while the agent had a clean working surface and I had the beautiful web UI, the agent and I were missing a common bridge — a shared dataset we could both speak in. I got a bit tired of writing so verbosely to the agent.

I want to be able to speak in Unique Identifiers / IDs, get to the point, cut to the chase.

So I built [`plnk-tui`](https://plattnum.github.io/planka-cli/) — a terminal UI that gives me the same hierarchy the CLI already speaks:

```
project → board → list → card → task
```

Not a replacement for the [Planka](https://planka.app/) web UI. A companion to it, and a bridge — somewhere the agent and I can speak a common language without me jumping out to the GUI every time I need to read or edit a card.

My goal was simple: navigate the [Planka](https://planka.app/) hierarchy with my keyboard and, with one keypress — `y` (YANK) — copy the current context as a JSON string the agent can drop straight into the conversation. One fell swoop, and the agent has exactly where I am. No more describing a card around the agent until it figures out which one I meant.

Here's what a yank looks like — the full hierarchy, project down to card:

```json
{"project":{"id":"1766510032108651830","name":"BookBridge"},"board":{"id":"1766510291450856764","name":"Work"},"list":{"id":"1766512261381227840","name":"In Progress"},"card":{"id":"1766513265430496590","name":"BB-000: PRD and Design Document"}}
```

Or, if I'm at the board level and want to talk about every card on it:

```json
{"project":{"id":"1766510032108651830","name":"BookBridge"},"board":{"id":"1766510291450856764","name":"Work"}}
```

I also provided a plnk command builder in case I just want to run the command myself:

```bash
plnk card snapshot 1766513265430496590 --output json
```

`y` gives the agent the IDs to use with plnk, `Y` gives me and the agent an immediate command to run.

Now it's just a few keypresses — navigate, yank, paste into the agent, and we're rolling. Less time searching. More time working.

This started out as a read-only hierarchy viewer of [Planka](https://planka.app/), but as time went on I added web-socket listening for live updates — that was an interesting challenge. Now I don't have to refresh the hierarchy during an agent session that's actively manipulating it.

Then I added some basic edit capability.

Maybe I'll add card creation and richer editing later. For now, it's already doing what I needed — a BRIDGE between my [Planka](https://planka.app/) instance and my agent, where we can speak the same hierarchy without me tab-flipping out to the GUI. Less searching. More working. That's enough for now.
