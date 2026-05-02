---
layout: post
title: "The Long Way Back to Planka: Building plnk"
description: "Why I came back to Planka and built plnk — a thin, strict, hierarchy-aware CLI for AI agents to work with Planka kanban boards."
date: 2026-05-02
permalink: /blogs/the-long-way-back-to-planka.html
---

# The Long Way Back to Planka: Building plnk

> I came back to Planka.

I have historically used [Planka](https://planka.app/) — an open-source, self-hostable Trello-style kanban tool — as my project/task management system. I like it because it is simple, visual, and already has the hierarchy I want and understand intuitively:

```
project → board → list → card → task/comment
```

However, as I started working with agents day-to-day, I went looking for a new system or SaaS to coordinate that work. I wanted Planka to stay the shared source of truth, but I didn't have much luck with the existing set of MCP servers.

## Trying everything else first

I tried a bunch of AI-inspired task systems: SaaS + MCP, MCP servers that manage markdown files in git repos, PRD capture tools, Claude workspace, "vibe kanban" boards, sub-agent managers, agent harnesses, and various AI + task orchestration setups.

There are a lot of these tools right now — people are still figuring out what real human-and-agent collaboration is supposed to look like.

But for my actual workflow, they felt slow, buggy, hard for me to use because the UI was bent around agent integration, or just pointed at a problem I do not really have.

Currently, I do not need a harness to spin up fifty-plus agents and send them off to vibe-code an entire product.

That is not how I work with agents today. Most of the time I am working hand-in-hand with one to three agents on specific mechanical tasks: inspect this code, implement this change, write the test, update the docs, move the card, record the decision.

I want the agent close to the work — not disappearing into an orchestration layer where my job becomes reviewing dashboards, debugging orchestrators, and then orchestrating the orchestrators.

> So I came back to Planka.

At first I tried the obvious routes. I looked at the different Planka MCP servers. I tried shell-script-style CLI approaches. I looked at Python libraries.

The MCP servers had the most polish, but they kept stacking up costs: tool descriptions filling my context, slow round-trips, bugs in specific tool calls, and chatty multi-call patterns for simple actions. Every abstraction between the agent and Planka was paying its dues — in tokens, latency, or correctness.

Eventually I landed on the simplest thing: let the agent use the REST API directly.

I pointed the agent at the Swagger docs and had it call Planka with curl.

And honestly, that worked better than I expected.

It was efficient in the sense that there was no extra abstraction layer. The agent could hit the real API, get real data, and mutate the real system.

But it was also painfully verbose. Even simple actions required constructing curl commands, headers, JSON payloads, and follow-up queries. To find the card I wanted to discuss, the agent often had to drill through projects, boards, lists, and cards, or fetch a broad snapshot and then sift through it.

That meant more context. More noise. More chances for the agent to get slightly lost.

And there was a worse problem underneath. Every call needed an `X-API-Key` header, which meant the agent was writing my key into its own context on every request. I was hosting locally and rotating the key after each session, but those are mitigations, not fixes. A working surface that asks the agent to handle credentials directly is not one I would trust outside a sandbox.

The REST API was the right foundation, but it was not the right working surface.

That is when [plnk](https://github.com/plattnum/planka-cli) clicked for me.

I did not need a new task manager. I did not need an MCP server. I needed a thin, strict, boring command-line layer over Planka's API — something hierarchical, predictable, and friendly to agents.

Putting the key in config did more than stop it leaking. Planka already ties every API key to a user with their own role and project access. The problem was not capability, it was practicality: with the key in agent context, leaning on that boundary meant accepting it could leak. With the key in config, the existing Planka mechanism becomes something I can actually rely on — a real agent user, scoped access, audit trail, all using machinery Planka already has.

Instead of making the agent build this:

```bash
curl -X POST "$PLANKA_SERVER/api/cards/..." \
  -H "X-API-Key: ..." \
  -H "Content-Type: application/json" \
  --data '{"...": "..."}'
```

I wanted it to write this:

```bash
plnk comment create --card <card-id> --text "Implemented the fix"
```

Instead of asking the agent to rediscover the world every time, I wanted scoped commands:

```bash
plnk card find --list <list-id> --title "auth" --output json
plnk board snapshot <board-id> --output json
plnk card move <card-id> --to-list <list-id>
```

That is the heart of plnk: Planka stays the system of record, but the agent gets a compact interface that is much harder to misuse.

It is not trying to be magical. That is the point. It is just a deterministic CLI with a hierarchy, typed errors, structured output, and machine-readable help. Enough structure to keep the agent on rails, without turning the whole workflow into another platform.

## Why a CLI instead of MCP?

Something useful happens once the agent has the CLI: it starts improvising. Depending on the question, it pipes plnk output through jq to trim noise, loops over cards, grep-filters lists, writes a short script when the question is more than one command away.

That is the power angle. MCP tools are often single-purpose calls; a CLI becomes a language. The agent can combine plnk, jq, loops, files, pipes, and shell scripts to do exactly the right amount of work — including work I never wrote a command for.

The agent can also learn what it needs on demand:

```bash
plnk card --help --output json
plnk card move --help --output json
```

That is the core idea behind plnk: make Planka scriptable, hierarchical, and agent-friendly without replacing Planka.

## The shape of plnk

plnk follows a strict grammar:

```
plnk <resource> <action> [target] [flags]
```

Examples:

```bash
plnk project list
plnk board list --project <project-id>
plnk list list --board <board-id>
plnk card find --list <list-id> --title "TUI"
plnk card snapshot <card-id> --output json
```

The hierarchy matters. A card belongs to a list, a list belongs to a board, a board belongs to a project. Searches are scoped. There are no surprising global queries where an agent accidentally grabs the wrong card from the wrong board.

That design is intentional. AI agents are very capable, but they do better when the interface keeps them on rails.

A few things I care about:

- JSON output for agents and scripts
- ASCII Table output for humans
- Markdown output for docs and agent-to-human replies
- Typed exit codes
- Machine-readable help
- stdout for data, stderr for diagnostics
- IDs treated as opaque strings
- No guessing when a search is ambiguous

The result is a CLI that works for shell scripts and, most importantly, AI agents.

## How I actually use it with agents

My agent harness loads a small skill file for plnk — a prompt-level reference for the CLI's grammar — that lives in the plnk repo. The agent does not have to rediscover the tool every session.

My workflow has two phases: getting work into Planka, then working through it.

### Loading the work in

This part is also a conversation. We either draft a PRD together or I hand the agent one I already have, break it into tasks, then I tell the agent how I want it dropped into Planka. Something like:

> "Load this PRD as the first card in Auth Refactor, Work Board, Backlog. Then create one card per task, label them all `auth-refactor-rollout`, and give them a shared title prefix `ARR-###` so they sort together."

Small PRDs go straight into the card description; large ones get attached as a document. Either way the PRD is the anchor card, and the task cards group under a shared label.

### Working through it

Once the work is in Planka, the loop is just a conversation. I describe what I want using names that already exist in my Planka: project, board, list, label, card title. Something like:

> "In Auth Refactor, Work Board, look in Backlog and pick the next card labeled `auth-refactor-rollout`."

Or I open a card in the Planka web UI and just tell the agent its title and where it lives — `Auth Refactor / Work Board / Backlog / "multi-tenant auth"`. The agent resolves the rest through plnk.

*(I'm also working on plnk-tui — a terminal UI that makes this even tighter: `y` to yank the ID hierarchy of a card, label, list, board, or project straight into the conversation. More on that in the next post.)*

The important part is that Planka remains the artifact. By the time a card reaches Done, it carries the useful history: what changed, why it changed, what was decided, what follow-up work exists. I do not want that buried in an agent transcript I will never reread. I want it in the project system, attached to the card it belongs to.

So here I am, back where I started — [Planka](https://planka.app/), with one fewer abstraction and a small CLI to keep the agents close to the work. I hope it is useful to someone else with the same instinct.

Tooling moves fast right now, and there is a real chance [plnk](https://github.com/plattnum/planka-cli) never finds an audience before something else replaces it. That is the deal with building anything in this space — you are mostly writing for the moment. So if plnk helps anyone, even briefly, that is enough. And if not, it has kept my work and my agents' work organized and collaborative across many projects, and made it easier to shift between them as I go.

---

*Next: plnk-tui — where the conversation gets even tighter.*
