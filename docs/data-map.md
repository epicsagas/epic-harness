# Data Map — Global vs Project Storage

Complete inventory of every data artifact written to disk.

## Directory Tree

```
~/.harness/
├── harness.db              ← Global operational DB (17 tables, project column)
├── memory.db               ← Global knowledge graph (4 tables, project-agnostic)
├── config.toml             ← Global configuration
├── graph.json              ← Memory graph visualization cache
│
├── session-state/                  ← Host-session identity (working-directory independent)
│   └── session_start.{id}.json     ← Fixed session partition date
│
├── global/
│   ├── patterns.jsonl              ← Cross-project learning patterns (opt-in)
│   └── .cross-project-enabled      ← Empty file (opt-in marker)
│
├── orgs/{org}/teams/{team}/        ← Team system (fully global)
│   ├── config.json
│   ├── mission.md
│   ├── playbook.md
│   ├── agents/{name}.md
│   └── .history/{name}-{ts}.md
│
├── exports/                        ← Memory node exports (global)
│
└── projects/{slug}/                ← Per-project directory
    ├── obs/
    │   └── session_{sid}.jsonl          ← Observations (JSONL fallback)
    ├── sessions/
    │   └── snapshot_{millis}.json       ← Session snapshots (JSON fallback)
    ├── evolved/
    │   ├── {name}/SKILL.md              ← Evolved skill markdown
    │   ├── {name}/meta.json             ← Skill metadata
    │   ├── promotion_counters.json      ← Promotion counters (JSON fallback)
    │   └── manifest.json               ← Workspace manifest
    ├── evolved_backup/                  ← Stagnation rollback backup
    ├── orbit/
    │   └── PIPELINE-*.json              ← Orbit pipeline state
    ├── reflect-queue/
    │   ├── job_*.{pending,claimed,completed,failed}
    │   └── worker-*.slot                ← Bounded SessionEnd workers
    ├── orchestrator/
    │   ├── run.json                     ← Orchestrator run state
    │   ├── control.json                 ← Control directive
    │   └── agents/{id}/
    │       ├── status.json              ← Agent status
    │       ├── stream.jsonl             ← Agent events
    │       └── inbox.jsonl              ← Agent messages
    ├── memory/
    │   ├── nodes/*.md                   ← Legacy memory nodes
    │   ├── edges.jsonl                  ← Legacy edges
    │   └── .migrated                    ← Migration marker
    ├── metrics.json                     ← Legacy metrics
    ├── evolution.jsonl                  ← Legacy evolution records
    ├── pending_synth.jsonl              ← Bounded synthesis backlog
    └── guard-rules.yaml                 ← Custom guard rules
```

## Global Databases

### harness.db — Operational Data

Single file at `~/.harness/harness.db`. Shared across all projects, scoped by `project` column.

| Table | Purpose | Project Column |
|-------|---------|:--------------:|
| `_harness_meta` | Schema version, migration state | ✗ |
| `observations` | Tool call observations (success/failure/score) | ✓ |
| `sessions` | Session snapshots | ✓ |
| `evolution_records` | Per-session evolution analysis | ✓ |
| `metrics_state` | Key-value metrics (total_sessions, avg_success_rate, …) | ✓ |
| `score_history` | Session score entries (capped 50/project) | ✓ |
| `skill_attribution` | Per-skill A/B scoring | ✓ (PK) |
| `promotion_counters` | Skill promotion gating counts | ✓ (PK) |
| `workspace_manifest` | Workspace skill manifest | ✓ |
| `evolved_skills` | Evolved skill metadata | ✓ |
| `orch_runs` | Orchestrator runs | ✓ |
| `orch_agents` | Agents within runs | ✗ (FK→runs) |
| `orch_agent_events` | Agent lifecycle events | ✗ |
| `orch_agent_inbox` | Inter-agent messages | ✗ |
| `orch_control` | Control directives (pause/cancel/redirect) | ✓ |
| `orbit_pipelines` | Orbit pipeline definitions | ✓ |
| `global_patterns` | Cross-project patterns | ✓ |

### memory.db — Knowledge Graph

Single file at `~/.harness/memory.db`. Project-agnostic (projects stored as tags on nodes).

| Table | Purpose |
|-------|---------|
| `nodes` | Knowledge nodes (type, title, tags, importance, access_count) |
| `nodes_fts` | FTS5 full-text search index |
| `edges` | Relationships between nodes |
| `_meta` | Schema version |

## Global Files

| Path | Format | Writer Hook | Purpose |
|------|--------|-------------|---------|
| `session-state/session_start.{id}.json` | JSON | resume | Stable host-session partition date across project-directory changes |

## Per-Project Files

All paths are under `~/.harness/projects/{slug}/`. The slug uses the canonical
project root name plus a stable hash, so repositories with the same directory
name remain separate.

### Active Data (written by hooks)

| Path | Format | Writer Hook | Purpose |
|------|--------|-------------|---------|
| `obs/session_{sid}.jsonl` | JSONL | observe | Tool observations (fallback) |
| `sessions/snapshot_{ms}.json` | JSON | snapshot | Session state (fallback) |
| `evolved/{name}/SKILL.md` | Markdown | reflect | Evolved skill body |
| `evolved/{name}/meta.json` | JSON | reflect | Skill metadata |
| `orbit/PIPELINE-*.json` | JSON | snapshot | Orbit pipeline state |
| `orchestrator/run.json` | JSON | orchestrate | Orchestrator run |
| `orchestrator/agents/{id}/*.jsonl` | JSONL | orchestrate | Agent events/inbox |
| `reflect-queue/job_*` | JSON | reflect | Durable SessionEnd work |
| `pending_synth.jsonl` | JSONL | reflect | Host skill-synthesis backlog |
| `guard-rules.yaml` | YAML | User | Custom guard rules |

### Legacy Data (read by migrate, no longer written)

| Path | Format | Imported To |
|------|--------|-------------|
| `metrics.json` | JSON | `metrics_state` + `score_history` |
| `evolution.jsonl` | JSONL | `evolution_records` |
| `evolved/promotion_counters.json` | JSON | `promotion_counters` |
| `evolved/manifest.json` | JSON | `workspace_manifest` |
| `memory/nodes/*.md` | Markdown | `nodes` (memory.db) |
| `memory/edges.jsonl` | JSONL | `edges` (memory.db) |
| `session_start.{id}.json` | JSON | Global `session-state/` record on the next same-project SessionStart |

## Dual-Write Pattern

Hooks write to SQLite first; on DB failure, fall back to files.

| Data | SQLite (primary) | File (fallback) |
|------|-----------------|-----------------|
| Observations | `observations` table | `obs/session_{sid}.jsonl` |
| Snapshots | `sessions` table | `sessions/snapshot_{ms}.json` |
| Evolution | `evolution_records` table | `evolution.jsonl` |
| Metrics | `metrics_state` + `score_history` | `metrics.json` |
| Orchestrator | `orch_*` tables | `orchestrator/` JSON/JSONL |
| Orbit | `orbit_pipelines` table | `orbit/PIPELINE-*.json` |

## Migration Commands

| Command | Source | Destination |
|---------|--------|-------------|
| `migrate` | Per-project JSONL/JSON files | `~/.harness/harness.db` |
| `migrate --to-global` | `~/.harness/projects/*/harness.db` | `~/.harness/harness.db` |

SessionStart also migrates a legacy project-local `.harness/` directory. It
validates the full source tree before copying. A symlink, non-regular entry, or
copy failure keeps the source tree. The source is removed only after a complete
copy.

## External Configuration (not in ~/.harness)

| Path | Purpose |
|------|---------|
| `~/.config/epic-harness/telemetry-consent` | Telemetry on/off |
| `~/.config/epic-harness/install-id` | Anonymous install UUID |
| `~/.claude/settings.json` | Claude Code hooks config |
| `~/.claude.json` | MCP server registration |
| `.harness/guard-rules.yaml` (in project tree) | Project-local guard rules |
