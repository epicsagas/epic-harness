---
description: "Real-time orchestration dashboard. Shows active agent count, per-agent progress stage, dependency graph, elapsed time, and recent event summary."
---

# /status — Orchestration Dashboard

You are displaying the **Status Dashboard** — a read-only view of the current orchestration run.

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

## Process

### Step 1: Resolve harness directory

```bash
HARNESS_DIR=$(epic-harness path)
```

If `epic-harness` is not available, fall back to `HARNESS_DIR="${HOME}/.harness/projects/$(basename $(git rev-parse --show-toplevel 2>/dev/null || echo default))"`.

### Step 2: Check orchestration state

1. Check if `$HARNESS_DIR/orchestrator/run.json` exists:
   ```bash
   cat "$HARNESS_DIR/orchestrator/run.json"
   ```
2. If the file does not exist or the directory is missing, output:
   > No active orchestration. Run `/go` or `/orbit` to start.
3. If the file exists, read `run.json` and inspect the `status` field:
   - If `status` is not `"running"`, output:
     > Last orchestration ended with status: **{status}**. No active run.
   - If `status` is `"running"`, proceed to Step 3.

### Step 3: Read all agent states

Extract the agent list from `run.json.agents` (or `run.json.agent_ids` depending on schema).

For each agent `{id}`:

1. Read agent status:
   ```bash
   cat "$HARNESS_DIR/orchestrator/agents/{id}/status.json"
   ```
   If the file is missing, mark the agent status as `unknown`.

2. Read the last 5 events:
   ```bash
   tail -5 "$HARNESS_DIR/orchestrator/agents/{id}/stream.jsonl"
   ```
   If the file is missing or empty, show "no events yet".

3. Calculate elapsed time:
   - If `started_at` is set and `completed_at` is not: elapsed = now - `started_at`
   - If both are set: elapsed = `completed_at` - `started_at`
   - Format as `Xm Ys` or `Xh Ym` for readability.

4. Read the dependency graph from `run.json.dependencies` (or `run.json.depends_on` per agent).

### Step 4: Render dashboard

Output a formatted dashboard (keep total under 30 lines):

```
## Orchestration Dashboard

- **Run ID**: {run.id}
- **Status**: {run.status}
- **Elapsed**: {total_elapsed since run.started_at}
- **Agents**: {count where status=running} / {total} active

| Agent | Role | Status | Elapsed | Last Event |
|-------|------|--------|---------|------------|
| {id} | {role} | {status} | {elapsed} | {summary of last event} |

### Dependency Graph
{render with ASCII arrows, e.g.:}
  builder ──→ reviewer ──→ integrator
  builder ──→ tester

### Recent Events (last 5 across all agents)
- [{timestamp}] {agent_id}: {event_summary}

### User Interventions
{read $HARNESS_DIR/orchestrator/control.json; show directives if any, else "none"}
```

**Dependency graph rendering rules:**
- Read edges from `run.json.dependencies` (format: `{"agent_a": ["agent_b", "agent_c"]}` meaning agent_a must finish before agent_b and agent_c start).
- Render using `──→` arrows: `agent_a ──→ agent_b`
- If multiple agents depend on one, group on separate lines:
  ```
  builder ──→ reviewer
  builder ──→ tester
  ```
- If no dependencies exist, output: "No inter-agent dependencies."

**Event summary formatting:**
- Each event is one line from `stream.jsonl` with fields: `timestamp`, `type`, `message`.
- Summarize `message` to at most 60 characters.
- Sort all events by timestamp descending, take top 5.

### Step 5: Optional — file change summary

```bash
git diff --stat
```

If changes exist, append:

```
### File Changes
- {count} files modified ({insertions} insertions, {deletions} deletions)
- Key changes: {list up to 5 file paths}
```

## Notes

- This command is **read-only** — it never modifies orchestration state.
- Use `cat` and `jq` to read JSON files efficiently when available.
- If `jq` is not available, read files directly and parse the relevant fields.
- Handle missing or malformed files gracefully — show "N/A" or "not available" rather than erroring.
- Keep output concise — max 30 lines for the main dashboard view.

## Red Flags

- Attempting to modify any file in `$HARNESS_DIR/orchestrator/`
- Showing raw JSON to the user instead of a formatted dashboard
- Failing silently when the orchestrator directory is missing (always explain the state)
- Listing more than 5 recent events (clutters the view)
