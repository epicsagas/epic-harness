# Live Agents

The dashboard shows two distinct sources of recorded activity:

- **Recently active coding sessions** have recorded tool activity within the
  last five minutes. They do not require `/orbit` or a spawned subagent. The
  timestamp is the latest observation, not the session ID or a process scan.
  Sessions expire from this section after five minutes without observations.
- **Tracked subagents** come from explicit `SubagentStart`/`SubagentStop` events
  or Claude's `Agent` tool lifecycle. Cards show the reported status, task,
  progress, heartbeat and dependencies. Missing heartbeats are flagged after
  30 minutes; this does not invent a completion event.

The project selector applies to both sources. All-projects reads every contained
project harness directory and the shared SQLite store, independent of the
directory where the dashboard server was started. Identical IDs in different
projects stay distinct. File and SQLite runs are normalized to the same UI
shape, including persisted per-agent status updates. Directory links that
escape the project store are skipped. Reading the panel never changes a run.

The panel polls every five seconds, avoids overlapping requests, discards
responses from a previously selected project, and shows load failures. It never
fills an unavailable live view with demo agents. Session history dates and
ordering come from actual observation timestamps, including UUID session IDs.

Hosts must deliver Epic observation/lifecycle hooks to appear here. The panel
cannot discover an uninstrumented coding process or prove process liveness
from a recent tool call. Antigravity currently contributes invocation activity
only when its supported hooks produce observations; it does not claim an
unsupported subagent lifecycle contract.

After installing a binary with dashboard changes, restart the dashboard server
and reload the browser: an already running `epic-harness serve` process keeps
its embedded dashboard and Rust code in memory.
