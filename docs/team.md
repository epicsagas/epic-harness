# `epic team` — Org-Level Agent Teams

> Implementation: `src/hooks/team/`
> Spec: `docs/research/team-spec.md`

---

## Overview

`epic team` manages **org-level agent teams** — persistent team definitions that accumulate
knowledge across projects and are never silently overwritten.

Core model:
```
Org  ──owns──▶  Team  ──has──▶  Agent(s)  (copied to .claude/agents/ per project)
                  │
                  ├──has──▶  Playbook     (global, append-only)
                  └──has──▶  Mission      (one-line purpose)
```

Teams live in `~/.harness/orgs/{org}/teams/{team}/` — independent of any project.
Agents are **copied** into `.claude/agents/{team}/` at sync time, where Claude Code
auto-discovers them.

---

## Storage Layout

```
~/.harness/orgs/
└── {org}/                            # default: "epic"  (HARNESS_ORG env to override)
    └── teams/
        └── {team}/
            ├── config.json           # name, org, type, projects[], created, updated
            ├── mission.md            # one-line domain ownership statement
            ├── playbook.md           # accumulated knowledge (append-only, never truncated)
            ├── agents/
            │   ├── domain-expert.md  # canonical role definition
            │   └── reviewer.md
            └── .history/             # backups before each agent replacement
                └── domain-expert-2026-04-16.md
```

### config.json

```json
{
  "name": "backend",
  "org": "epic",
  "type": "stream",
  "projects": ["epic-harness", "my-api"],
  "created": "2026-04-16T00:00:00Z",
  "updated": "2026-04-16T00:00:00Z"
}
```

`projects[]` is for traceability — stale entries cause no harm.

---

## Usage

### Interactive design (primary flow)

```
epic team
```

Launches a 4-phase interactive flow:

```
Phase 1 — Resolve context
  - Org from HARNESS_ORG env | default "epic"
  - Scan project: detect stack (Rust/Node/Python/Go/Java), read README excerpt
  - List existing teams in org

Phase 2 — Design
  - Prompt: team name (default: sanitized project name)
  - Prompt: team type (stream/platform/enabling/subsystem)
  - Prompt: mission (one-line domain ownership)
  - Show proposed agent composition from type template

Phase 3 — Write / merge
  - New team: create all files
  - Existing team: merge per strategy (no silent overwrites)

Phase 4 — Sync
  - Copy agents to ./.claude/agents/{team}/
  - Inject ## Team Context into each copy
```

### Subcommands

```bash
epic team list                       # list teams in current org
epic team list --org netflix         # list teams in named org
epic team show backend               # config + agents + mission
epic team show backend --playbook    # also print full playbook
epic team sync backend               # re-copy agents to .claude/agents/
epic team link backend               # attach existing team (sync + add to config.projects)
epic team unlink backend             # remove .claude/agents/backend/ (keeps global store)
epic team delete backend             # remove from current project (.claude/agents/backend/)
epic team delete backend --global    # permanently delete from org store + local copy
epic team history backend reviewer   # list .history/ backups for an agent
```

### Env / flags

| Variable / Flag | Description |
|---|---|
| `HARNESS_ORG` | Default org (overrides "epic"). Set per-shell or in `.envrc`. |
| `--org <name>` | Override org for any subcommand |
| `--playbook` | `show` only: print full accumulated playbook |

---

## Team Types

Type drives default agent composition proposals. User can override at design time.

| Type | Keyword | Default agents |
|---|---|---|
| Stream-aligned | `stream` | `domain-expert`, `reviewer`, `tester` |
| Platform | `platform` | `api-designer`, `infra-specialist`, `dx-agent` |
| Enabling | `enabling` | `specialist` |
| Complicated Subsystem | `subsystem` | `domain-specialist`, `integration-tester` |

---

## Merge Strategy

Re-running `epic team` on an existing team never silently overwrites.

| Object | Action |
|---|---|
| Agent — new name | **Add** automatically |
| Agent — content unchanged | **Skip** (no-op) |
| Agent — content changed | **Prompt** (default: keep existing). Replaces → backs up to `.history/` |
| `playbook.md` | **Always append** `---` separator + new section. Never truncated. |
| `mission.md` — unchanged | **Skip** |
| `mission.md` — changed | **Prompt** (default: keep existing) |
| `config.json → projects[]` | Append project name if absent. Never removes. |

All prompts default to **skip** (safe). Destructive ops require explicit `y`.

---

## Project Integration

At sync time, agents are **copied** to `.claude/agents/{team}/` with a `## Team Context`
section injected. This gives each agent orientation without loading the full playbook
into the context window.

```markdown
## Team Context
**Team**: backend (Stream-aligned)
**Mission**: Own the API layer end-to-end across all backend services
**Full playbook**: `epic team show backend --playbook`
```

The global store holds canonical definitions (no Team Context).
The project copy holds canonical definition + injected context.

Re-running `epic team sync backend` refreshes the project copy (e.g. after mission update).

`.claude/agents/` is **not** gitignored by default — teams may want to version-control
their project-local copies. Add to `.gitignore` explicitly if undesired.

---

## Multi-Org Example

```bash
# Default org accumulates across all personal/work projects
epic team                          # creates in "epic" org

# Model a Netflix-style topology in a separate org
HARNESS_ORG=netflix epic team      # creates in "netflix" org

# List orgs
ls ~/.harness/orgs/
# epic/  netflix/  startup-x/
```

Same team name in same org = intentional cross-project sharing. `epic/teams/backend`
accumulates knowledge from every project that creates or links it.

---

## Agent Integration

Each coding agent integration contains a thin wrapper that delegates to `epic team`:

```
integrations/
├── opencode/commands/team.md    → "run epic team"
├── cursor/commands/team.md      → "run epic team"
└── codex/prompts/team.md        → "run epic team"
```

No team logic lives in the integration layer. The CLI is the source of truth.

---

## Implementation

```
src/hooks/team/
├── mod.rs      entry point — pub fn run(args) -> i32
├── store.rs    storage layer — TeamConfig, path helpers, CRUD, content builders
└── cli.rs      dispatch + 7 subcommands, interactive flow, scan_project, sync_to_project
```

### Key types (`store.rs`)

```rust
pub struct TeamConfig {
    pub name: String,
    pub org: String,
    pub team_type: String,     // "stream" | "platform" | "enabling" | "subsystem"
    pub projects: Vec<String>,
    pub created: String,
    pub updated: String,
}
```

### Key functions (`store.rs`)

| Function | Purpose |
|---|---|
| `orgs_base_dir()` | `~/.harness/orgs/` |
| `team_store_dir(org, team)` | `~/.harness/orgs/{org}/teams/{team}/` |
| `save_agent(org, team, name, content, backup)` | Write agent; if `backup=true` copies old to `.history/` |
| `inject_team_context(content, team, type, mission)` | Inject/replace `## Team Context` section |
| `build_playbook_section(...)` | Generate typed playbook section with coordination notes |
| `default_agents_for_type(type)` | Return `(role, description)` pairs for the type template |

### Key functions (`cli.rs`)

| Function | Purpose |
|---|---|
| `cmd_default()` | Interactive 4-phase design flow |
| `sync_to_project(org, team)` | Copy + inject agents into `.claude/agents/{team}/` |
| `cmd_list` | List teams with type + project count |
| `cmd_show` | Show config, mission, agents (+ playbook with `--playbook`) |
| `cmd_sync` | Re-sync from global store to project |
| `cmd_link` | Sync + register project in config |
| `cmd_unlink` | Remove `.claude/agents/{team}/` from project |
| `cmd_delete` | No flag: remove `.claude/agents/{team}/` from current project. `--global`: permanently delete from org store (prompts confirmation) |
| `cmd_history` | List `.history/` backups for an agent |
