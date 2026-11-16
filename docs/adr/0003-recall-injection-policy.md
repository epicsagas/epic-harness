# ADR 0003: No per-prompt injection in 0.9.0; hook-driven recall stops bumping access counts

- Status: Proposed
- Date: 2026-11-16
- Area: C (issue #276; PR #392)
- Spec: `docs/specs/0.9.0-pipeline.md`, section C
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area C

## Context

All references are to main at 74424a2.

- Context injection happens once per session. `epic resume` prints active evolved skill bodies within `inject_per_skill_chars` (1,600) and `inject_total_chars` (10,000) (`src/config.rs:36-48`). Neither `.claude-plugin/hooks.json` (SessionStart, PreToolUse, PostToolUse, PreCompact, SessionEnd) nor `.codex-plugin/hooks.json` (SessionStart, PreToolUse, PostToolUse, SubagentStart, SubagentStop, PreCompact, Stop) registers a prompt-submit hook, and `src/main.rs:165-167` dispatches six lifecycle hooks only.
- `_dispatch` already tells the model to run `epic mem recall "<hint>"` at task start, on errors, and before architectural decisions (`skills/_dispatch/SKILL.md:133-143`). On-demand recall exists today.
- Every `smart_recall` call increments `access_count` and sets `accessed_at` for the returned nodes (`src/mem/store/recall.rs:138-145`, `:150-164`). `access_count` is 15% of the ranking score (`src/mem/store/recall.rs:118-124`). Recency uses `updated`, not `accessed_at` (`src/mem/store/recall.rs:110-113`). `accessed_at` drives importance decay (`src/mem/store/decay.rs:36-46`).
- Hook-driven reads bump these counters with no model intent. At every SessionStart, resume spawns `epic-harness mem context --project <slug>` (top 5, `src/hooks/resume.rs:554-579`, `src/mem/cli.rs:853-863`) and then calls `smart_recall(Some(slug), None, 10)` (`src/hooks/resume.rs:581-584`). `reflect --context` calls `smart_recall` with a fixed hint and limit 30 (`src/hooks/reflect.rs:564-580`). The same top nodes are touched every session, which raises their `access_count` and keeps them ranked first: a self-reinforcing loop.
- Decay runs at every SessionStart (`src/hooks/resume.rs:624-641`) and multiplies importance by 0.9 for nodes whose `accessed_at` is older than 30 days. Today the SessionStart touches keep the top project nodes from ever decaying.
- PR 392 adds a prompt-submit hook that injects up to 10,000 characters of skill bodies and memory previews on every prompt and calls `smart_recall` per prompt. The audit (`.lead/audit/all.json`, PR 392) flags the counter mutation, edits to `_dispatch`, weakened plugin-asset tests, an inline JavaScript launcher, and stacked unrelated commits, and classes it as a feature request needing a product decision.
- Memory writers: the model (MCP `mem_add`, CLI `epic mem add`), the local HTTP server, reflect's auto-ingest (`src/hooks/reflect.rs:1171-1172`), and legacy import. Anything injected automatically is text the model did not author in this turn.

## Decision

1. Do not add a per-prompt hook in 0.9.0 (memo option C1). Recall stays on demand through `_dispatch` and the MCP tools.
2. Introduce two recall access modes in `src/mem/store/recall.rs`:
   - `Explicit` (current behavior): `access_count + 1` and `accessed_at = now` for returned nodes.
   - `Passive`: `accessed_at = now` only, `access_count` unchanged.
3. Hook-driven reads use `Passive`: resume step 5a (subprocess gains `--passive`), resume step 5b, and `reflect --context` memory collection. Explicit model or user reads (MCP `mem_recall`, MCP `mem_context`, CLI `epic mem recall` and `epic mem context` without `--passive`) stay `Explicit`.
4. `epic mem recall` and `epic mem context` accept a boolean `--passive` flag, parsed before the value-taking flag parser.
5. Option C2 (a gated, hints-only prompt hook capped near 1 to 2 kB, untrusted-data framing, no counter writes) is out of 0.9.0. It needs its own ADR, and that ADR may only be opened after a measurement shows recall is being missed. Measurement definition: over at least 30 sessions from observe logs, the share of sessions with a task-start `epic mem recall` call. Below 50% would justify C2. The threshold is a proposal for the maintainer.

## Alternatives considered

- C3, PR 392 as proposed (up to 10,000 characters every prompt). Rejected: it repeats the full SessionStart injection budget on every turn, which the memo's context-engineering sources argue against (Area C items 1, 3, 4). It also amplifies memory-poisoning exposure (items 7, 8, 9) and mutates ranking on every prompt.
- C2 now. Rejected for 0.9.0: cost and hit rate are unmeasured, and the measurement is cheap to take first.
- Fully side-effect-free passive reads (no `accessed_at` update either). Not chosen as the default: removing the SessionStart touch would make the project's most important nodes start decaying by 10% per session start once they are 30 days untouched by explicit reads. That is a larger behavior change than the ranking fix needs. Offered as Decision needed C-1.
- Make every read passive, including explicit model recall. Rejected: AGENTS.md defines access frequency as a ranking signal (`AGENTS.md:276`, `:292`); removing all bumps turns that 15% weight into a constant.

## Consequences

- SessionStart and `reflect --context` stop inflating `access_count`. Access frequency now reflects explicit retrievals only, so ranking drifts away from "whatever SessionStart showed last time".
- Decay behavior is unchanged: passive reads still refresh `accessed_at`.
- No new hook, no new injection surface, no new host-portability work for Codex or grok.
- Version-skew hazard: resume spawns `epic-harness` from `PATH` (`src/hooks/resume.rs:565`). An older binary on `PATH` treats `--passive` as an unknown flag that takes a value and still bumps counters. Harmless, but the fix is only complete when the binary on `PATH` is 0.9.0.
- #392 is closed as superseded by this ADR. #276 stays open as a backlog feature discussion linked to the C2 measurement.

## Migration and compatibility

- DB schema: none. Existing `access_count` values are kept (no reset). Counts inflated before 0.9.0 stay inflated and wash out only through newer explicit reads. A one-time reset is possible but not proposed, because it would also erase genuine explicit-read history.
- CLI: new optional `--passive` flag on `epic mem recall` and `epic mem context`. Existing invocations behave as before.
- Library API: `smart_recall` and `smart_recall_pool` keep their signatures and stay `Explicit`; a new `smart_recall_passive` covers the hooks.
- Docs: AGENTS.md "Access tracking" line updated.

## Rollback

Revert the commit. No persisted format changes. The `--passive` flag disappears, and resume returns to bumping counters.
