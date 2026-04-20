# plnk-cli skill

Pi/Agent Skills support files for teaching agents to use the `plnk` CLI directly.

## Files

- `SKILL.md` — main skill entrypoint; operational rules and decision procedure
- `references/commands.md` — command lookup reference organized for agent use
- `references/api-quirks.md` — Planka API behaviors that affect CLI usage

## Design intent

This skill is intentionally **CLI-first**:

- `plnk` is the canonical interface to Planka
- the skill teaches agents how to use the CLI well
- it does **not** define a shadow API over the CLI
- it does **not** assume fixed kanban columns

## Installation options

### Local skill path

Load the skill directory directly:

```bash
pi --skill /Users/plattnum/repos/planka-cli/skills/plnk-cli
```

### Project or global settings

Add the repo skill directory or parent skills directory to Pi settings:

```json
{
  "skills": [
    "/Users/plattnum/repos/planka-cli/skills"
  ]
}
```

### Package-style install from local repo

Because this repo uses conventional directories (`skills/`), Pi can install the repo directly:

```bash
pi install /Users/plattnum/repos/planka-cli
```

### Package-style install from git

Once the repo is hosted remotely:

```bash
pi install git:github.com/plattnum/planka-cli
```

Or with a raw git URL:

```bash
pi install https://github.com/plattnum/planka-cli
```

## Notes

- Keep `SKILL.md` focused on invariants and operating procedure.
- Keep long-form command detail in `references/`.
- Put repo-specific workflow conventions in `AGENTS.md` / `CLAUDE.md`, not in the core skill.
