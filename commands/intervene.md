---
description: "Intervene in active agent orchestration. Pause, cancel, or redirect running agents mid-execution."
---

# /intervene — Agent Intervention

You are the **Intervention Controller** — write control directives that the orchestrate hook reads on the next agent tool call.

**CRITICAL**: Run `HARNESS_DIR=$(epic-harness path)` first. NEVER use `.harness/` in the project directory.

## Process

### Step 1: Resolve harness directory
Run `HARNESS_DIR=$(epic-harness path)`

### Step 2: Check active orchestration

1. Read `$HARNESS_DIR/orchestrator/run.json`
2. If no active run (status != "running"), output: "No active orchestration to intervene in."
3. Show current agent list with statuses

### Step 3: Parse user command

The user invocation format:
- `/intervene pause {agent_id}` — pause a specific agent (blocks next tool call)
- `/intervene pause all` — pause all agents
- `/intervene cancel {agent_id}` — cancel a specific agent
- `/intervene cancel all` — cancel entire orchestration
- `/intervene redirect {agent_id} {new_instruction}` — change what an agent is doing
- `/intervene resume {agent_id}` — resume a paused agent
- `/intervene reassign {from_agent_id} {to_agent_id}` — reassign a blocked agent's task to another

If the user provides no arguments, ask which action and target they want.

Validate the agent_id against the agent list in run.json. If the agent_id is not found, output: "Agent '{agent_id}' not found in active orchestration. Available agents: {list}" and stop.

### Step 4: Write control directive

Read current generation from `$HARNESS_DIR/orchestrator/control.json` (or 0 if file doesn't exist). Increment by 1.

Write to `$HARNESS_DIR/orchestrator/control.json`:
```json
{
  "action": "pause|cancel|redirect|resume",
  "target": "agent_id or 'all'",
  "message": "optional user message",
  "generation": <current_generation + 1>
}
```

For `redirect`, the message field contains the new instruction.
For `pause`/`cancel`, the message field is optional — use an empty string if none provided.
For `resume`, set action to "resume" and target to the agent_id.
For `reassign`, set action to "reassign", target to `from_agent_id`, message to `to_agent_id`.

**Additional actions by type:**

- **cancel all**: Also update `$HARNESS_DIR/orchestrator/run.json` status to "aborted"
- **redirect**: Also append the new instruction to the target agent's `$HARNESS_DIR/orchestrator/agents/{agent_id}/inbox.jsonl` as a new line: `{"type": "redirect", "instruction": "{new_instruction}", "at": "{ISO-8601}"}`
- **cancel {agent_id}**: Also update the agent's status in run.json to "cancelled"
- **reassign**: Also call `reassign_agent(from_id, to_id)` logic — the hook handles state update on the next agent tool call. The `from_agent` status is set to Failed and the task is delivered to `to_agent`'s inbox as a Handoff message.

### Step 5: Confirm

Output confirmation:
```
Intervention recorded: {action} {target}
The {target} agent will respond on its next tool call.
Use /status to monitor.
```

For reassign: "Reassignment recorded: task from {from} → {to}. The {to} agent will receive the task in its inbox on next tool call."

## Notes

- This command only writes the control file. The orchestrate hook reads and acts on it.
- Multiple interventions can be queued (generation number increases each time).
- The orchestrate hook checks `control.json` before every agent tool call, so interventions take effect within one tool call cycle.
- "cancel all" is destructive — confirm with the user before executing.
- "redirect" requires a new instruction — if none is provided, ask the user.

## Red Flags

- Intervening without an active orchestration (check run.json first)
- Cancelling all agents without user confirmation
- Redirecting an agent without a concrete new instruction
- Writing to control.json when run.json status is "aborted" or "complete"
- Reassigning to an agent that doesn't exist in run.json
- Reassigning when from_agent is not in BLOCKED or FAILED state
