# ADR 0006: Memory recall enforces exact project scope inside the query

- Status: Proposed
- Date: 2026-11-16
- Area: F (issues #228, #229, #230, related #266; PRs #331, #332, #333, related #366)
- Spec: `docs/specs/0.9.0-pipeline.md`, section F
- Literature: `docs/references/research-0.9.0-pipeline.md`, Area F

## Context

All references are to main at 74424a2.

- `nodes.projects` and `nodes.tags` are comma-joined strings written with `join(",")` and no spaces (`src/mem/store/types.rs:127-135`; CLI input is trimmed by `csv_to_vec`, `src/mem/cli.rs:329-334`). Reading back uses `split_csv`, which also trims (`src/mem/store/util.rs:24-29`).
- Recall with a hint takes the top `limit * 3` FTS rowids across all projects (`src/mem/store/recall.rs:39-53`) and only then filters with `projects LIKE '%proj%'` (`:59-70`). Other projects can fill the candidate budget and hide every match of the requested project (#230).
- `LIKE '%x%'` matches substrings and treats `%` and `_` as wildcards (#228). The pattern appears at `src/mem/store/recall.rs:61-63`, `:83-87`, `src/mem/store/search.rs:77-99`, `src/mem/store/decay.rs:39`, `:74-75` (tags `pinned` and `stale`), and `src/evolve/ingest.rs:89-93` (previous-session lookup for the session chain, not listed in the audit).
- MCP `mem_recall` appends up to five 1-hop neighbors from `graph_neighbors_pool` with no scope filter (`src/mem/mcp.rs:335-387`), leaking other projects' nodes (#229). `graph_neighbors_pool` returns neighbors in discovery order with a connection count as weight (`src/mem/graph.rs:251-292`); main takes the first five in that order and emits them in `read_nodes_pool` order, which is unordered `IN` (`src/mem/store/node.rs:110-131`). Main does not sort neighbors by weight. Exceeding `limit` with neighbors is documented (`src/mem/mcp.rs:106`).
- No candidate query excludes `stale` nodes although AGENTS.md says they are excluded from recall (#266, PR 366, planned for 0.8.15).
- The FTS table is external-content over `nodes` with columns `id, title, body, tags, projects` (`src/mem/store/schema.rs:27-30`). The code comment says the two-step query avoids FTS JOINs through sqlx `AnyPool` (`src/mem/store/recall.rs:39-40`). Bundled SQLite comes from libsqlite3-sys 0.30.1 (`Cargo.lock:2199-2200`).
- `nodes` has a TEXT primary key, so FTS keys on the implicit rowid (`src/mem/store/schema.rs:12-13`, `:27-30`).
- Audit (`.lead/audit/all.json`): PR 331's `instr(',' || col || ',', ',' || ? || ',') > 0` is correct exact membership; PR 333's pushdown is correct but needs a test because of the AnyPool comment; PR 332's scope filter is correct, its redefinition of `limit` and 200 cap is a product decision; the three PRs and PR 366 conflict textually on the same lines.

## Decision

Adopt memo option F2 with F1's membership predicate:

1. One SQL membership fragment, `instr(',' || <col> || ',', ',' || ? || ',') > 0`, replaces every `LIKE '%x%'` on `projects` and `tags` listed above.
2. Push the project predicate into the FTS candidate query as `rowid IN (SELECT rowid FROM nodes WHERE <membership>)`, keeping the two-step shape (step 1 still selects only FTS rowids, so no FTS column is read through `AnyPool`). By SQL semantics, `WHERE` filtering happens before `ORDER BY rank LIMIT`, so a small project is never starved. A starvation test proves it, and an `EXPLAIN QUERY PLAN` test pins the plan shape (FTS virtual-table scan, non-correlated subquery).
3. Exclude nodes tagged `stale` from all recall candidate queries and from neighbors (folds #266; Decision needed F-1).
4. MCP neighbors: when `project` is given, keep only neighbors whose `projects` contain it exactly, and never return `stale` neighbors. Selection keeps discovery order, scans at most 256 neighbor ids in chunks of 32, stops at 5, and emits in selection order. `limit` semantics are unchanged.
5. An empty project string is treated as no project (unscoped), matching what `LIKE '%%'` did.
6. Record F3 (a `node_projects(node_id, project)` table with an index, optionally mirrored as an FTS column filter) as the longer-term shape. Not built in 0.9.0.

## Alternatives considered

- F1 alone (exact membership plus adaptive over-fetch). Rejected: still ranks globally first, so a large foreign corpus can starve a small project unless the loop is unbounded.
- `JOIN nodes_fts f JOIN nodes n` (PR 333 shape). Not chosen: functionally equivalent, but it reads FTS through a join, which the existing code comment flags as fragile under `AnyPool`; the `rowid IN (subquery)` form keeps step 1 identical in shape to today.
- F3 now. Rejected for 0.9.0: needs a runtime DB migration (human review) and breaks the "projects is a CSV" assumption in every reader.
- PR 332's new `limit` semantics (neighbors count toward `limit`, cap 200). Rejected: product decision, not a bug fix (audit).
- Strip spaces in SQL (`replace(col, ' ', '')`) to mirror `split_csv` trimming. Rejected: changes tag semantics for tags that contain spaces. The write invariant (no spaces after commas) is pinned by a differential test instead.

## Consequences

- `project = "alpha"` no longer returns nodes of `alphabet`; `%` and `_` in filters are literal.
- Project-scoped recall returns in-scope matches even when other projects dominate the FTS ranking.
- MCP recall with `project` stops leaking neighbors from other projects. Unscoped recall keeps cross-project neighbors.
- Stale nodes disappear from recall and neighbors, matching AGENTS.md. Users who relied on finding old nodes through recall must use `epic mem list` or search.
- Decay: `pinned` and `stale` checks become exact. A tag such as `staleness` no longer blocks stale tagging, and `unpinned` no longer protects from decay.
- Session-chain linking in reflect ingest attaches to the correct project's previous session.
- Cost: the scope subquery scans `nodes` once per recall (`instr` cannot use an index). Acceptable at current memory sizes; F3 is the upgrade path.

## Migration and compatibility

- DB schema: none. No data rewrite.
- API: function signatures unchanged except the recall access mode added by ADR 0003. MCP tool schemas unchanged.
- Semver: patch-level fixes in substance. The release plan puts them in 0.9.0; moving them to 0.8.15 is Decision needed F-2.

## Rollback

Revert the commit. No persisted data changes.
