# ADR 0008: Opt-in task-scoped prompt hook (option C2) supersedes ADR 0003 decision 1 for opt-in use

- Status: Accepted (maintainer approved decisions D1 to D5 for issue #276)
- Date: 2026-11-25
- Area: C (issue #276; supersedes decision 1 of ADR 0003 for opt-in use, and decision 5 as its precondition)
- Spec: [issue #276](https://github.com/epicsagas/epic-harness/issues/276), option C2 of ADR 0003 made concrete
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area C

## Context

- ADR 0003 decision 1 rejected any per-prompt hook for 0.9.0 and decision 5 deferred option C2 (a gated, hints-only prompt hook capped near 1 to 2 kB, untrusted-data framing, no counter writes) until a measurement showed recall was being missed (share of sessions with a task-start recall below 50%).
- Nothing in Rust writes or reads dispatch logs today; `_dispatch` asks the model to append them itself, so that measurement cannot be taken from existing data.
- Context injection happens once per session. `epic resume` prints a hint-less top-7 recall, so the memory relevant to the current task can be missing (#276).
- Claude Code supports a `UserPromptSubmit` hook: stdout JSON `hookSpecificOutput.additionalContext` is injected next to the prompt, exit 2 or `decision: "block"` rejects it, and a timed-out hook is cancelled while the prompt proceeds (https://code.claude.com/docs/en/hooks, fetched 2026-11-25).

## Decision

1. Ship the C2 MVP for Claude Code with `[prompt] enabled = false` by default (D1). The hook is registered in `.claude-plugin/hooks.json`; when disabled it returns before any file, git, or database access. Users opt in with `[prompt] enabled = true` in `~/.harness/config.toml`. The hook's own dispatch lines, plus the `_dispatch` lines the model writes, provide the measurement ADR 0003 asked for. Flip the default only if task-start recall is below the ADR 0003 threshold.
2. Recall uses a new `ReadOnly` access mode (D2): no write of `access_count` or `accessed_at`, so ranking and decay do not depend on how many prompts were typed. The recall path opens `memory.db` with `mode=ro` through `memory_pool_if_exists`, never caches the pool, and never runs schema init or migrations, so a timeout can never abandon a migration.
3. Codex is not registered in this change (D3). Its `UserPromptSubmit` schema matches, but registration waits for a live nonce probe and for the Windows quoting work (#265). `codex_has_no_prompt_submit` pins this.
4. Memory items are the title plus a masked first body line of at most 120 characters (D4). Stored notes are untrusted (MCP `mem_add` and auto-ingest write them): they pass through `mask_secrets`, are flattened to one line, are framed as data, and are length-capped.
5. `registry/scripts/hooks/run.mjs` prints nothing for the `prompt` subcommand when `epic` is not found (D5), because stdout of this event becomes model context on every prompt.
6. Budgets and failure behavior: default 1,500 characters (clamped to 200..=4000, far below Claude's 10,000 character `additionalContext` cap), 3 memories (0..=5), and an in-process cap of 250 ms (50..=2000) covering connect plus query, with a manifest timeout of 5 s. `run` returns 0 on every path, the hook never sets `onFailure`, and the main binary prints nothing for `prompt` on the stdin passthrough branch, so the raw prompt is never echoed.
7. Privacy: the prompt lives in memory only. The dispatch line (`harness_dir()/dispatch/dispatch_<date>.jsonl`) carries static rule ids, counts, memory ids, and timings, never prompt text, hint tokens, memory titles or bodies, cwd, or transcript path. Skills come from a static regex table that names existing static skills; the hook only reports matches and `_dispatch` stays authoritative.

## Alternatives considered

- Wait for the measurement before shipping anything (ADR 0003 decision 5 as written). Rejected: no data source exists until some hook writes dispatch lines, and a default-off hook costs one launcher plus binary start per prompt (about 30 to 80 ms; see the measurement in the pull request).
- `Passive` recall per prompt. Rejected: it would refresh `accessed_at` on every matched node and stop them from ever decaying, a ranking side effect driven by typing volume.
- Register Codex now. Rejected until a live probe confirms Codex injects the context into the model turn.
- PR 392 as proposed (10,000 characters per prompt, counter mutation, `_dispatch` edits). Rejected in ADR 0003 and unchanged.

## Consequences

- Users who never set `[prompt] enabled = true` get no output and no behavior change; the cost is one process start per prompt.
- Enabled users get at most 1,500 characters of keyword- and memory-matched context per prompt. Overlap with the SessionStart recall is possible; per-session dedup is later scope.
- `_dispatch` gains two sentences under "Memory-Augmented Dispatch" and no other edits.
- Evolved skills are never injected per prompt, so holdout attribution is unaffected.
- Injected text is saved in the Claude transcript and replayed on `--resume`, not recomputed.

## Rollback

Set `[prompt] enabled = false` (the default), or revert the commit. No persisted format changes; dispatch files are append-only JSONL under the project data directory.
