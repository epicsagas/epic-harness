---
name: orchestrate
description: "Auto-triggered during active orchestration. Provides coordination protocol: inbox reading, dependency checking, message formatting, and handoff rules for multi-agent execution."
---

# Skill: Orchestrate

## Process

### Step 1: Check orchestration state
1. Run `HARNESS_DIR=$(epic-harness path)`
2. Read `$HARNESS_DIR/orchestrator/run.json`
3. If no active run, this skill does not apply — return

### Step 2: Read agent inbox
1. Read `$HARNESS_DIR/orchestrator/agents/{my_id}/inbox.jsonl`
2. Process any pending messages from other agents or user interventions
3. Messages from other agents: integrate into current task context
4. Messages from user (via /intervene redirect): adjust task accordingly

### Step 3: Check dependencies
1. Read your agent's inbox (`inbox.jsonl`) for unread messages
2. If you receive a `handoff` message from "orchestrator": your dependencies are resolved — proceed with your task
3. If inbox is empty and your task has not started yet: check `run.json` dependency graph as a fallback
4. If any dependency is not DONE, wait for the Handoff inbox notification from the orchestrator (push-based — no polling)

### Step 4: Execute with progress reporting
While working on the assigned task:
1. The Rust `orchestrate` hook automatically appends events to `stream.jsonl` on each tool call
2. Status updates happen automatically via the hook
3. No manual progress reporting needed — the hook handles it

### Step 5: Complete and hand off
When task is complete:
1. Report status via structured output format (DONE/DONE_WITH_CONCERNS/BLOCKED/NEEDS_CONTEXT)
2. The `orchestrate` hook will evaluate dependencies and notify downstream agents
3. If results should be shared with specific agents, include in the output summary

## Message Format Convention

When agents need to communicate (via SendMessage or future inbox/outbox):

```json
{
  "from": "agent_id",
  "to": "agent_id",
  "type": "handoff|question|blocked|result",
  "body": "message content",
  "timestamp": "ISO-8601"
}
```

## Agent-to-Agent Messaging

When an agent needs to communicate with another agent:

### Via send_message (Rust hook — automatic)
The orchestrate hook's `send_message` function writes directly to the target agent's inbox.
Message types:
- `handoff` — task completed, passing results to next agent
- `question` — blocking question, sender waits for reply
- `blocked` — sender is stuck, requesting help
- `result` — sharing output data for downstream consumption

### Via structured output (LLM → hook)
Include in your completion output:
```json
{
  "send_to": "agent-id",
  "msg_type": "handoff|question|blocked|result",
  "body": "message content"
}
```
The `run_post` hook detects this pattern and calls `send_message` automatically.

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "I'll just work independently" | Orchestration exists to prevent conflicts and duplication | Check dependencies first, report status |
| "Progress reporting slows me down" | The hook does it automatically on every tool call | No manual action needed |
| "I'll read all agents' state" | Only read your own inbox and dependencies | Respect isolation boundaries |

## Evidence Required

- [ ] Inbox checked before starting work
- [ ] Dependencies verified (all upstream agents DONE)
- [ ] Structured output format used for completion report

## Red Flags

- Starting work before dependencies are met
- Ignoring inbox messages from other agents
- Not reporting BLOCKED state when stuck
