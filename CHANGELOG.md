# Changelog

All notable changes to epic-harness will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Cargo source builds depended on frontend tooling**: `build.rs` invoked
  `pnpm` and could mutate the source tree while building. Cargo now embeds the
  checked-in dashboard asset. `make dashboard-build` generates it, and CI
  verifies an exact byte match. Source installation requires Rust 1.94 or newer
  and does not require Node.js or `pnpm`.
- **Orbit state could collide or dismiss the wrong project**: SQLite identity
  is now `(project, id)` with a lossless schema-v6 migration. Dashboard
  dismissal uses one exact project-scoped file-and-SQLite backend. Completion
  requires a concrete pull-request URL and successful CI before Evolve.
- **Evolved-skill dashboard data came from an orphaned SQLite copy**: bounded
  project-scoped reads now use each project's `evolved/` directory, the same
  source that writes `SKILL.md` and `meta.json`.
- **SessionEnd reflection work could be lost or starved**: jobs now publish
  atomically after durable writes, use two bounded worker slots and a 64-entry
  scan, refresh ownership when claimed, and use atomic retry/completion
  transitions. Per-session persistence keys make recovery idempotent. Malformed
  or exhausted jobs move to `.failed`. Reflection fallback and Orbit reads now
  apply file, record, and byte limits before parsing.
- **Legacy project-state migration could cross file boundaries or remove its
  only source**: SessionStart now validates the complete project-local
  `.harness/` tree before copying. Symlinks, non-regular entries, and copy
  failures keep the source tree. Only a complete migration removes it.
- **Eval scaffolding could generate a recursive benchmark**: `epic eval --init`
  no longer treats a Makefile or justfile `eval` wrapper that calls
  `epic-harness eval` as a domain benchmark. Genuine domain `eval` targets
  remain auto-detected.
- **Plugin hooks could run a stale or missing runtime** (#113): the bootstrap
  parsed `epic-harness version` from stdout even though the real binary writes
  it to stderr, installed an unpinned `latest` release, accepted installer
  success without verifying the result, and then ran `resume` through a
  separate shell fragment. All shipping version owners now declare `0.8.3`.
  A canonical runtime revision distinguishes unreleased fixes that share that
  semantic version. The Node runner installs and verifies the exact version and
  revision before SessionStart continues, and fails visibly without emitting
  event-invalid stdout.
- **Codex hook manifests were not portable or schema-valid** (#113): plugin
  paths were unquoted, POSIX-only binary probes had no Windows override,
  matcher groups used unsupported `description` fields, `SubagentStop`
  succeeded with empty stdout instead of JSON, and `SessionEnd` inherited its
  one-second default. Hooks now use one quoted cross-platform Node runner,
  define `commandWindows`, validate and forward the runtime's subagent-stop JSON
  (using `{}` only when it is silent), and give SessionEnd Codex's three-second
  maximum. CI runs the bootstrap and manifest contracts on Linux, macOS, and
  Windows, and supports a manual dispatch for fork PR verification.
- **A hook process was treated as a session** (#113): `session_id()` was
  `YYYYMMDD_PID`, but hosts run each hook in its own process — one installation
  produced 37,611 "sessions" from 40,804 observations, which broke sequence
  detection, repeated-error analysis, the 50-event telemetry cap and lock
  cleanup. `HookInput` now keeps the host's `session_id`, `turn_id`,
  `tool_use_id`, `agent_id` and `agent_type`, and `session_id()` uses the host
  id when present. Ids are sanitized to `[A-Za-z0-9_-]` and capped, since they
  reach `session_{id}.jsonl` and `resume.{id}.lock`. SessionStart persists the
  host session's partition date; later hooks fail visibly if that record is
  missing or corrupt instead of switching identity.
- **Reading a file was scored as a failed tool call** (#113): success came from
  matching output text for `FAIL`, `TypeError`, `timeout` and similar, so
  reading a log or a diff that merely *mentions* an error counted as a failure —
  66% of all classified errors came from read-oriented commands. Structured
  status (`exit_code`, `is_error`, `success`, `status`) is now authoritative in
  both directions, and without it, calls whose output is fetched content
  (`Read`, `Grep`, `Glob`, read-only Bash) record `unknown` instead of a
  fabricated failure. Build and test failures are unaffected. `unknown`
  observations stay unscored and are excluded from success rates rather than
  counted against them.
- **Object-shaped Bash responses lost their output** (#113): only `output` was
  read from a `tool_response` object, never Claude Code's `stdout`, so those
  observations were scored against empty text.
- **Codex `apply_patch` was categorized as `other`** (#113): Codex's edit tool
  never produced edit statistics, so observing it could not feed the edit side
  of Ring 3.
- **The live-agent `running` transition never fired** (#113): it depends on an
  `Agent` `PreToolUse` event, but the Claude manifest invoked `observe` only
  after tool use — a general defect, not Codex-specific. Both manifests now
  register the subagent lifecycle, Codex through native
  `SubagentStart`/`SubagentStop`.
- **Codex edits escaped orchestration safety** (#113): the pause directive and
  concurrent-write conflict checks matched `Edit`/`Write` only, so
  `apply_patch` bypassed both. `guard` now recognizes it and checks every file
  in the patch envelope, not a single `file_path`.
- **Commands were persisted verbatim and unbounded** (#113): one stored command
  reached ~90 KB, and marker scans found authorization headers and key-shaped
  strings. `action` is now credential-masked and capped at 2 KB. File paths are
  deliberately preserved — file-level pattern detection keys on them. A latent
  panic on byte-slicing multi-byte UTF-8 in the same paths is fixed.
- **Nothing ever deleted observations** (#113): the deletion function had no
  caller outside its own test, so the database only grew (~22.8 MB in three
  days). New `db.retention_days` (default 90, `0` disables) prunes at session
  end and sweeps stale `resume.*.lock` and `telemetry_error_count_*.txt` files.
- **Orbit could report completion it had not earned** (#113): pipelines were
  observed marked `complete` with no PR, and one with `audit_fail_count` above
  its own `max_retries`. `reflect` now reports both, and the orbit skill records
  `pr_url` at ship time.
- **`epic team status` and `unlink` could not see Codex agents** (#113): both
  scanned `.claude/agents/` only, so the flat `~/.codex/agents/{team}-*.toml`
  files `sync` writes were invisible and never cleaned up. `status` lists them
  and `unlink --global` removes them; a project-scoped unlink now says they
  remain and how to remove them.
- **Ring 3 read zero observations on every host** (#113): `today()` returns
  `YYYYMMDD`, but the observation range query only ISO-expanded 10-character
  `YYYY-MM-DD` bounds. `timestamp` is compared lexicographically and `'-'`
  (0x2D) sorts below `'0'` (0x30), so `2026-07-27T..` compares *less than*
  `20260727` and `WHERE timestamp >= '20260727'` matched nothing — no metrics,
  no score history, no evolution records, however many observations were
  stored. A new `day_bounds()` normalizes both forms and is now the single
  source of truth for all three range queries (the project-scoped stats
  variant had the identical bug).
- **Reflection mixed other projects' observations** (#113): the range query had
  no project predicate even though writes are project-tagged, so a reflection
  could analyze mostly foreign data and save the result under the invoking
  project. It now takes an explicit scope, with a new `idx_obs_project_ts`
  index. Legacy rows written before observations carried a project (`NULL`)
  are excluded from scoped reads — they cannot be attributed retroactively.
- **Metrics and snapshots bypassed project isolation** (#113): reflect, resume
  and `reflect --context` used the unscoped metrics/snapshot readers — which
  their own docs describe as indeterminate once several projects have written
  rows — while saves were already scoped. Snapshots were also inserted with no
  project value, so resume could restore a different repository's state.
- **JSONL fallback observations were silently dropped** (#113): observations go
  to SQLite and reach JSONL only when a DB write fails, yet reflect read JSONL
  *only if* SQLite returned nothing — so on any day the DB also had rows the
  fallback records were ignored, and the error branch returned early despite a
  comment claiming it fell through. Both stores are now read, merged and
  deduped.
- **`epic reflect --context` reported `obs_stats.total: 0`** (#113) for projects
  with thousands of database observations, because it read only project
  `obs/*.jsonl`. It now reads SQLite first.

### Changed
- **Codex hook registration corrected** (#113): `PostToolUse` observed only
  `Bash`, hiding every edit, MCP and function-tool call from Ring 3 — it now
  matches `*`. `PreCompact` is registered so snapshot/resume works. `reflect`
  moved from `Stop` (turn-scoped, fires every turn) to `SessionEnd`; left on
  `Stop`, the date fix above would have made every turn re-analyze the whole
  day.
- **Polish works on Codex** (#113): Codex edits arrive as `apply_patch` with the
  patch body in `tool_input.command` and no `file_path`, so polish returned
  immediately and never ran. It now resolves targets from the
  `*** Update File:` / `*** Add File:` / `*** Move to:` headers.
- **Resume context reaches the Codex model** (#113): everything resume surfaces
  went to stderr, and the first stdout fix emitted tagged lines beginning with
  `[` that Codex parsed as malformed JSON. Resume now buffers all context and
  emits one valid SessionStart `additionalContext` object, including evolved
  skill Markdown. Other events retain their event-specific output contracts,
  and Claude Code behaviour is unchanged.
- **Dashboard opens on Codex session startup/resume** (#113): server reuse no
  longer suppresses the browser open. `clear` and `compact` starts stay quiet,
  and browser-spawn errors are reported instead of discarded.
- **Plugin bootstrap works under Node ESM and Codex** (#113): the SessionStart
  script now uses ESM imports, reads `PLUGIN_ROOT` plus `.codex-plugin/plugin.json`,
  stays silent on stdout, and no longer invokes the removed
  `epic-harness install claude` seeding path.
- **`epic team sync` emits native Codex agents** (#113): it wrote Claude-style
  Markdown into `~/.codex/agents/{team}/`, which Codex ignores. It now writes
  flat `~/.codex/agents/{team}-{agent}.toml` with the required `name`,
  `description` and `developer_instructions` keys. `model: sonnet` and Claude
  `tools:`/`skills:` are dropped rather than mapped to invented equivalents.

## [0.8.2] — 2026-07-09

### Changed
- **Skill synthesis is now host-agnostic**: `reflect` no longer spawns a
  synchronous `claude -p --model haiku` subprocess (which could hang or time
  out under slow/remote hosts). It now emits a pending-synthesis manifest
  (`$HARNESS_DIR/pending_synth.jsonl`) with masked failure evidence + the
  template body; any host agent synthesizes a better body out-of-band and
  submits it via the new `epic-harness evolve accept-synth --skill <name>
  [--file <path> | --stdin]` CLI, which re-validates and re-runs the Critic
  gate before applying it. Unconsumed manifests leave the template body in
  place — synthesis can only improve a skill, never block seeding.

### Removed
- `[evolution]` config fields `llm_synthesis_cmd`, `llm_synthesis_model`, and
  `llm_synthesis_timeout_secs` — no longer meaningful now that synthesis runs
  out-of-process via the manifest protocol above. Existing values for these
  keys in `config.toml` are silently ignored (no error). `llm_synthesis` and
  `llm_synthesis_max_per_session` are unchanged and still apply.

### Fixed
- **`reflect` SQLite `near "DO"` error**: the legacy `metrics_state` /
  `skill_attribution` primary-key rebuild (`(key)` → `(key, project)`) used a
  compound `INSERT...SELECT...ON CONFLICT DO UPDATE` that sqlx's `Any` driver
  rejected as a syntax error, leaving affected databases stuck on the old
  single-column PK and breaking every subsequent `ON CONFLICT (key, project)`
  write. Rebuilds now use `INSERT OR REPLACE INTO ... SELECT` instead (no
  `ON CONFLICT` needed for a copy into an empty table). Also fixed a
  Postgres-style `$1,$2,$3` placeholder bug in the legacy JSONL→SQLite
  `import_metrics` migration path.

## [0.8.1] — 2026-07-03

Ring 3 rework: the evolution loop now closes in code, measures honestly, and
uses the LLM for the one step that needs intelligence.

### Added
- **LLM skill synthesis** (`src/evolve/synthesis.rs`): seeded skill bodies are
  synthesized by a headless `claude -p` call from the session's real failure
  evidence (per-category error snippets, counts, detected patterns) instead of
  static templates. Falls back to the template body on any failure; synthesized
  content passes the same validate → Critic → gate path. Config:
  `[evolution] llm_synthesis` / `llm_synthesis_cmd` / `llm_synthesis_model` /
  `llm_synthesis_timeout_secs` / `llm_synthesis_max_per_session`. Recursion
  guard via `EPIC_SYNTH_CHILD=1` + `EPIC_HOOK_PROFILE=minimal`; disabled in
  debug builds unless `EPIC_SYNTH_FORCE=1`.
- **Error snippet evidence**: `SessionAnalysis.error_snippets` — one masked,
  truncated representative snippet per failure category.
- **Holdout A/B attribution**: deterministic date-keyed rotation
  (`hash(skill, date) % attribution_holdout_modulus`) withholds under-evaluation
  skills for ~1/3 of days; `avg_score_without` now averages genuine holdout
  sessions (`SkillAttribution.sessions_holdout`, new SQLite column with
  idempotent migration). Config: `attribution_eval_sessions`,
  `attribution_holdout_modulus`.
- **Deterministic evolved-skill injection**: `epic resume` prints active
  evolved skills' bodies to stdout so SessionStart injects them into context —
  no more reliance on `_dispatch` prompt obedience to scan `evolved/`.

### Changed
- Skill eviction now requires evidence from both arms (≥3 active AND ≥2
  holdout sessions) instead of acting on the confounded legacy delta.
- Attribution scores only skills the session actually saw: the pre-seed skill
  listing is used, so skills seeded at session end no longer get credited with
  that session's score.
- `_dispatch` no longer instructs the model to scan `$HARNESS_DIR/evolved/`
  (holdout-arm skills must stay out of context).

### Fixed
- Cargo.toml description and AGENTS.md structure line now match the real skill
  count (26 + `_dispatch`).
- Holdout partition now uses the session-start date on both `resume` and
  `reflect`, so a session spanning UTC midnight no longer credits an
  active-injected skill to the holdout baseline (the confound this rework
  removed).
- `mask_secrets` now masks absolute file paths (Unix/Windows/tilde-home) so
  error snippets can't leak repo paths into the synthesis prompt or generated
  skills.
- LLM synthesis default timeout/per-session tightened (30s×2 → 10s×1) to keep
  the reflect SessionEnd hook well under host hook budgets.

## [0.8.0] — 2026-06-26

The plugin-native release. epic-harness now distributes as a single plugin layout that Claude Code, agy (Antigravity CLI), and codex read directly from disk — the `install` subcommand and the embed+copy pipeline are gone.

### Changed — Plugin-native distribution (breaking)
- **Removed `epic-harness install`/`uninstall`** + `install.rs`/`install_wizard.rs` (~2,330 LOC). Skills, hooks, and the `harness-mem` MCP server now load from a root plugin layout (`plugin.json`, `skills/`, `hooks.json`, `.mcp.json`) read directly by each tool.
- **Skills moved to repo root** (`registry/skills/` → `skills/`). `registry/commands/` dropped (commands were already consolidated into skills).
- **Config seeding relocated**: `~/.harness/config.toml` + `HARNESS.md` are now self-seeded by the resume hook (`config::ensure_global_config`) on first session — idempotent (config.toml write-once, HARNESS.md synced only when stale).
- **Tool support narrowed to Claude Code + agy + codex.** The cursor/opencode/cline/aider integrations were removed.

### Added
- **agy (Antigravity CLI) support**: root `plugin.json` manifest + auto-scanned `skills/`/`hooks.json`/`.mcp.json`. `agy plugin validate .` passes (27 skills + hooks processed).
- **Telemetry documentation**: dedicated `## Telemetry` section in README + 9 i18n + quickstart — what is collected (command/duration/outcome/failure class/hook events + product/version/os/install_id), what is never collected (code/paths/secrets/PII), opt-out default-on, `epic-harness telemetry status|on|off`.
- Unit tests for `ensure_global_config` (config.toml write-once, HARNESS.md stale-only sync).

### Fixed
- Security: `time` 0.3.45 → 0.3.47, `undici` 7.25.0 → 7.28.0 (7 of 8 dependabot advisories). `glib` 0.20 remains blocked on tauri's `gtk-rs ^0.18` pin — upstream, tracked separately.
- The `check` phase is unified into `audit` everywhere (manifests, orbit, go/ship/_dispatch skills, integrations, docs).

## [0.7.0] — 2026-06-19

The HarnessX evolution-engine release. Adapts the AEGIS pipeline (arXiv:2606.14249v1) to epic-harness's single-agent, per-project evolution loop. The evolution engine went from "reactive single-agent + SKILL.md-only" to **strategic, typed, regression-protected, and self-verifying**. ~5,000 LOC across 5 PRs (#75–#80), +69 tests, all CI green. Driven by the gap-analysis at `docs/analysis/harnessx-vs-epic-harness-gap-analysis.md` (vault mirror `ref-010`).

### Added — Evolution engine
- **Digester** (`src/evolve/digester.rs`): compresses a session's observations into per-task `TaskDigest`s — binary outcome, ranked failure categories, implicated components, evidence excerpts, tool trajectory, cross-iteration persistence. Paper §4.3.
- **Planner** (`src/evolve/planner.rs`): builds an `AdaptationLandscape` (persistent failures, attempted edits, edit-type coverage, untried edit types, component heatmap) and flags under-exploration. Paper §4.3 — the primary defense against local-minimum bias.
- **Typed edits** (`src/evolve/edits.rs`): `HarnessEdit` enum (AddSkill/ModifySkill/AddGuardRule concrete; ModifyConfig/AddInstinct reserved) with a falsifiable `EditManifest` per edit (edit_type/target/intended_effect/predicted_impact). Paper §4.3 / Table 9. `edit_type` now persists in SQLite and round-trips (was hardcoded `AddSkill`).
- **Seesaw** (`src/evolve/seesaw.rs`): per-task regression gate that blocks seeding when a previously-solved task regresses beyond tolerance. Paper §4.1. (Deliberately per-task, not per-dimension — the paper §6.6 proves per-dimension gating misses sub-threshold coupling.)
- **Variant isolation** (`src/evolve/variants.rs`): `VariantPool` with fork-on-regression (spawns a sibling variant rather than overwriting) and warm/cold stack-based routing. `MAX_VARIANTS` separate from `MAX_EVOLVED_SKILLS`. Atomic persistence. Paper §4.5 — the real catastrophic-forgetting defense.
- **Reward-hacking detection** (`src/evolve/metrics.rs::detect_reward_hacking`): least-squares slope of output_quality vs execution_cost (efficiency proxy, higher=better) over a configurable window. Configurable thresholds in `EvolutionConfig`. Now computed + persisted (was a dead flag).
- **Critic layer** (`src/evolve/critic.rs` + `registry/skills/_critic/SKILL.md`): deterministic in-loop Critic (no external LLM, per project rule) that suppresses seeding under reward hacking and rejects manifests claiming score lifts that contradict evidence. The `_critic` skill is the out-of-band LLM counterpart for non-local effects.
- **Falsifiability ledger**: each shipped edit's `EditManifest` is persisted to `EvolutionRecord.manifests` + a `manifests.jsonl` sidecar. (The cross-round Critic read that verifies predictions held is honestly documented as a deferred follow-up — the write loop is wired.)
- **AddGuardRule concrete editor** (`src/hooks/guard.rs::append_guard_rule`): appends a blocked/warned rule to the project's `guard-rules.yaml` via atomic read-modify-write, round-trip safe through the incumbent parser. Makes the Planner's `add_guard_rule` untried-edit-type attemptable.
- **Regression harness** (`tests/evolve_regression_test.rs`): 6 hermetic scenarios (no live benchmark, no SQLite, no network, no HOME redirect) locking the seesaw/variant/planner/outcome-score contracts — the validation substrate the gap-analysis demanded before the engine's claims could be trusted.
- **Processor abstraction** (`src/hooks/processor.rs`): `HookPoint` enum (6 lifecycle points ↔ subcommands) + `Processor` trait + static dispatch table wrapping the existing `run()` hooks unchanged. Representational seam (paper §3.2) — a full in-process pipeline redesign was explicitly rejected as the most invasive change.

### Added — Harness as a first-class object
- **`HarnessSnapshot` + CLI** (`src/evolve/snapshot.rs`, `src/harness_cli.rs`): `epic harness snapshot` (JSON + deterministic content hash), `epic harness diff <a> <b>`. `restore` deferred (destructive). Was dead code; now constructed.

### Added — Dashboard surfacing (PR #80)
- **5 new dashboard handlers** (`src/serve.rs`): `get_seesaw_registry`, `get_variant_pool`, `get_harness_snapshot`, `get_adaptation_landscape`, `get_manifests` (tail-reads the falsifiability ledger, capped 50).
- **3 previously-broken dashboard panels fixed**: `get_session_snapshots`, `get_global_patterns`, `get_effect_pending` were returning `"null"` via the dispatch fallthrough; now implemented.
- **Evolution.svelte**: reward-hacking warning banner (when `reward_hacking_suspected`), Seesaw/Variants/Adaptation-landscape panels, `edit_type` column in evolution history. `Promise.allSettled` so a cold project degrades gracefully.
- `harness.ts`: client wrappers + types (`SolvedTaskRegistry`, `VariantPool`, `HarnessSnapshotData`, `AdaptationLandscape`, `EditManifestEntry`); `HarnessMetrics` gains `reward_hacking_suspected` + `epoch_class`.

### Added — Documentation
- `docs/references/operational-mirror.md`: maps the evolution engine onto RL vocabulary (state/action/reward + the three pathologies → concrete defenses) with a tuning guide. Paper §4.1–4.2, §7.3.
- `dimension:` frontmatter on 6 core static skills (tdd/secure=control_and_safety, verify/perf=evaluation_and_reward, debug=observability, simplify=context_assembly).

### Fixed
- **Seesaw `reg.update()` latent bug**: the solved-task registry update now only runs when the gate passed. Previously it ran unconditionally, which could raise a task's best-of on a regressing round's coincidental high scores and mask future genuine regressions.
- **Flaky `rebuild_produces_stable_hash` test**: switched from live `build_snapshot()` (which diverged when a parallel test wrote to `evolved_dir()`) to a hermetic fixture.
- **SQLite DDL migration**: `evolution_records.edit_type` added via idempotent `ensure_column` (pragma_table_info guard + `AssertSqlSafe`), so existing databases upgrade without manual surgery.
- **Reward-hacking + epoch_class SQLite round-trip**: were hardcoded `false`/`None` on load; now read from the `metrics_state` key/value table.

### Changed
- `seed_smart_skills()` split into `plan_skill_edits()` (pure planner, emits `Vec<HarnessEdit>`) + `apply_skill_edits()` (Critic-verifies then applies) — signature preserved, behavior unchanged for the common path.
- `EditManifest`, `Metrics` now derive `Serialize/Deserialize/Default`.
- `cargo clippy --all-targets -- -D warnings` remains clean across the stack; 643 lib + 6 regression + 7 CLI tests.

### Known limitations (documented in code)
- Falsifiability loop is write-only: manifests persist, but the cross-round Critic read is a deferred follow-up.
- Seesaw is deliberately coarse (per-task); sub-threshold coupling out of scope — variant isolation is the practical mitigation.
- `AddGuardRule` editor is concrete but Planner auto-emission is not wired (conservative).
- `HarnessSnapshot restore` deferred (destructive).
- Processor trait is a representational wrapper (main.rs dispatch unchanged).

### Added — Project-scoped dashboard (#92, #93)
- File-based evolution readers (`seesaw`/`variants`/`snapshot`/`manifests`) thread `project` via `*_for(project)` — every dashboard panel now reflects the selected project.
- `metrics_state` / `evolution_records` / `score_history` / `skill_attribution` writers bind the project slug; `metrics_state` and `skill_attribution` gained composite PKs `(key, project)` / `(skill_name, project)` via idempotent table-rebuild migrations.
- `score_history` DELETE is project-scoped (was whole-table — a latent cross-project data-loss bug).

### Fixed — Evolution engine accuracy (#95)
- `EditType::from_db_str` no longer distorts unknown DB values into `AddSkill` — added `EditType::Unknown` (excluded from coverage) and made `"add_skill"` explicit.
- `edit_type_roundtrips` test timestamp is index-based; `token_estimate` / `estimate_tokens` documented as intentional scaffold.

## [0.6.5] — 2026-06-15

### Fixed
- **Dashboard always shows the installed version**: the sidebar had a hardcoded `v0.4.1` and `__APP_VERSION__` was declared but never defined, so the dashboard version display was frozen regardless of the installed release. The running binary now stamps its own `CARGO_PKG_VERSION` into the served HTML via `<meta name="harness-version">` (`serve.rs`), and the sidebar reads it at runtime (build-time `__APP_VERSION__` from `app/package.json` is the `vite dev` fallback). The version is now correct for every install/update without depending on a frontend rebuild. `app/package.json` was added to the version-bump checklist.
- **Release build reproducibility**: `Cargo.lock` is now tracked (it was gitignored). The v0.6.4 release failed in CI because, without a lockfile, CI resolved `brotli-decompressor` 5.0.2 (published 2026-06-13), which broke `brotli` 8.0.3's `implement_allocator` macro. The committed lock pins `brotli-decompressor` to 5.0.1, matching the successful v0.6.3 build. Otherwise identical to 0.6.4.

## [0.6.4] — 2026-06-15

### Fixed
- **Observations lost project attribution**: `insert_observation_pool` dropped the `project` column, so ~30% of observations written since the per-project → global db consolidation had blank project. The project slug is now bound at the observe hook call site and threaded through the store layer.
- **`migrate` targeted the wrong database**: `open_migrate_conn` opened the per-project db instead of the global one, so `migrate` and `migrate --to-global` wrote to the wrong database. Now targets global `~/.harness/harness.db`. Removed the dead per-project `harness_db_path()`.
- **Sessions merge schema drift**: `merge_attached_db_async` referenced stale session columns (`snapshot_json`/`millis`); aligned with the current `pending_tasks`/`context_usage`/`created_at_millis` columns.
- **DDL divergence from production**: `DDL_SQLITE` was missing the `project` column on 7 tables (`sessions`, `evolution_records`, `metrics_state`, `score_history`, `orch_runs`, `orch_control`, `promotion_counters`); aligned so fresh dbs match the production global db schema.

## [0.6.3] — 2026-06-12

### Added
- **`epic slug` CLI subcommand**: prints the worktree-safe project slug; used in Cursor hooks.

### Fixed
- **Project slug in linked worktrees**: `project_slug()` now uses `git-common-dir` to resolve a stable slug in linked worktrees instead of CWD dirname.
- **Orchestration test flakiness**: added the `serial` attribute to env-mutating tests to prevent race conditions.
- **Episteme ingest**: disabled Episteme integration in reflect session metrics and removed the broken dependency.
- **src-tauri build**: prefixed unused project params to satisfy the compiler after signature changes.

### Changed
- sqlx requirement updated from 0.8 to 0.9.

## [0.6.2] — 2026-06-10

### Fixed
- **FTS5 search broken after rusqlite→sqlx migration**: `search` and `recall` commands returned empty results because FTS virtual table columns were inaccessible via `sqlx::AnyPool`
  - Two-step rowid query pattern: `SELECT rowid FROM nodes_fts WHERE MATCH ?` → `SELECT ... FROM nodes WHERE rowid IN (...)`
  - Schema DDL now includes `CREATE TRIGGER IF NOT EXISTS` for `nodes_ai/au/ad` to keep FTS index in sync automatically
  - `migrate_fts_schema()`: auto-detects old 3-column FTS tables and migrates to 6-column (id, title, body, tags, projects) with content=nodes
  - Removed manual `DELETE/INSERT INTO nodes_fts` from upsert/delete — triggers handle sync
- **src-tauri dashboard build broken after sqlx migration**: function signatures changed during migration but `src-tauri` (separate crate) wasn't updated
  - Added pool-only alias functions: `load_metrics_all_pool`, `list_recent_snapshots_all_pool`, `query_recent_records_all_pool`, `query_obs_stats_all_pool`, `list_all_pipelines_pool_limited`
  - Fixed call sites in `src-tauri/src/commands/harness.rs` to use new signatures
- **Duplicate exports in dashboard harness.ts**: removed duplicate `getOrchestratorRun` and `getOrchestratorAgentStatus` function blocks
- **`store::pool` visibility**: changed `pub(crate)` → `pub` for cross-crate access from `src-tauri`

## [0.6.1] — 2026-06-10

### Fixed
- **`cargo fmt` failure**: reformatted `detect_stack()` C# detection block and Java/Kotlin `pom.xml`/`build.gradle` condition in `src/eval/config.rs` to satisfy rustfmt

## [0.6.0] — 2026-06-09

### Added
- **Security pipeline (Ring 2)**: three new skills ported from [defending-code](https://github.com/anthropics/defending-code-reference-harness)
  - **threat-model**: trust boundary analysis, threat actor enumeration, threat scenario generation → `THREAT_MODEL.md`
  - **vuln-scan**: 4-dimension systematic scanner (injection, auth, data exposure, dependencies) → `VULN-FINDINGS.json`
  - **triage**: adversarial validation with severity adjustment, chaining analysis, root-cause grouping → `TRIAGE.json`
  - Pipeline flow: `/threat-model` → `/vuln-scan` → `/triage`
- **Prompt auto-tuning for evolved skills**: underperforming skills receive targeted tuning guidance based on A/B score gaps
  - Tuning sections appended after `<!-- auto-tuned -->` delimiter — original content never modified
  - Auto-rollback after 3 consecutive declining sessions (`TUNING_DECLINE_LIMIT`)
  - History tracked in `SkillMeta.prompt_tuning_history` (capped at 10 entries)
  - New functions: `auto_tune_skills()`, `append_tuning_section()`, `strip_tuning_sections()`, `build_tuning_section()`
  - 10 new unit tests covering serialization, scoring, decline counting
- **Audit `--strict` mode**: trust boundary isolation for reviewer/auditor independence
  - Artifact-only delivery: audit modes receive only diff + spec, no builder context
  - Cross-check independence: code/security/test modes run blind until synthesis
  - Blind scoring: prevents anchoring bias between modes
  - No self-review: builder session excluded from audit agent selection
  - Activation: `--strict` flag or `mode: strict` in `.harness/engagement.md`
- **Engagement context**: optional `.harness/engagement.md` for security assessment scoping
  - Defines: Authorization, Scope (in/out), Constraints, Environment, Exclusions
  - `secure` skill checks for engagement context and loads scope if present
  - Reference template: `docs/references/engagement.md`
- **Tiered verification ladder** in `/ship`: T0 (build) → T1 (tests+lint+fmt) → T2 (AC verification) → T3 (security)
  - T1/T2 auto-retry ≤3 times
  - T3 conditional on engagement.md or security-scope diff
- **Semantic deduplication** in `/audit`: cross-mode finding dedup between parallel checks and synthesis
  - NEW/DUP_BETTER/DUP_SKIP classification
  - Severity reassessment across modes (highest severity wins)
- **SkillOpt-inspired evolution optimization**: three deep learning-inspired techniques adapted from [SkillOpt](https://arxiv.org/abs/2605.23904) for natural language skill evolution
  - **Negative Feedback Buffer**: persists rejected skill proposals with TTL-based expiry to prevent re-generating known-bad skills
  - **Minibatch Reflection**: decomposes observations into fixed-size batches for structural pattern extraction, catching micro-patterns hidden by session averages
  - **Slow/Meta Update**: epoch classification (Improving/Regressing/PersistentFailure/StableSuccess) with slow parameter tracking per evolved skill
- Ineffective skill auto-eviction: skills that demonstrably lower session scores are automatically removed after 3+ sessions of negative attribution
- New config options: `rejected_buffer_ttl` (default: 10) and `minibatch_size` (default: 8) in `[evolution]` section
- New types: `RejectedEntry`, `MinibatchInsight`, `EpochClass` in `src/shared/evolution.rs`
- New functions: `analyze_minibatches()`, `classify_epoch()`, `update_meta_field()`, rejected buffer CRUD
- **Eval skill + CLI**: project quality & regression evaluation with 4 dimensions (correctness, performance, quality, regression)
  - `epic eval --init` scaffolds eval.yaml with auto-detected stack (Rust/Node/Python/Go/Java)
  - `epic eval --json` outputs structured results for CI pipelines
  - `epic eval --baseline-update` saves current run as baseline for regression comparison
  - LLM-as-judge integration in SKILL.md for quality dimension (deferred to LLM session)
  - Orbit integration: Step 5.5 Eval phase inserted automatically when eval.yaml exists
  - New modules: `src/eval/{mod,config,runner,baseline,report}.rs`

- **`eval` benchmark auto-detect and in-repo baselines** (#73)
  - `Benchmark` struct gains `command` and `result_type` fields; benchmarks are now executed
  - `eval --init` scans for `benchmarks/eval_runner.py`, `Makefile eval`, `justfile eval` and pre-populates `benchmarks:` list
  - Baselines default to in-repo `benchmarks/baselines/latest.json`; results save to `benchmarks/results/`
  - `result_type: composite` parses `composite`/`score` float from JSON stdout (e.g. Episteme eval_runner.py output)
  - Warns at runtime when benchmark infrastructure exists but `benchmarks:` is empty
  - `correctness` and `quality` dimensions no longer inject stack-based defaults (`cargo test`, `cargo clippy`) — eval delegates build/test/lint to `verify`

- **`merge-project` subcommand**: consolidate duplicate project slugs into one
  - Three-layer merge: global `harness.db` (UPDATE project column), per-project `harness.db` (ATTACH + INSERT OR IGNORE), file-based data (`obs/`, `sessions/`, `evolved/`, `evolution.jsonl`, `orbit/`)
  - `--dry-run` previews row/file counts without writing
  - `--delete-source` removes the source directory after a successful merge
  - Handles composite-PK tables (`metrics_state`, `skill_attribution`, `promotion_counters`) with conflict-aware merge strategies

### Changed
- **Skill descriptions normalized**: removed `Trigger:` prefix from all 14 skill descriptions, applied consistent `[What it does]. [When to use]` pattern
- Skill count: 23 → 26 (9 pipeline + 17 quality gates)

## [0.5.0]

### Added
- **SQLite operational store**: all project operational data (observations, sessions, metrics, evolution, orbit pipelines, evolved skills) now stored in `harness.db` alongside the existing `memory.db` knowledge graph
- `epic-harness migrate` subcommand: import legacy JSONL/JSON data into SQLite (`--dry-run` to preview, `--reset` to retry interrupted migration)
- `store::observations::query_latest_observations_conn()`: query N most recent observation records

### Changed
- Dashboard commands (`get_harness_metrics`, `get_orbit_pipelines`, `get_evolved_skills`, `get_obs_summary`) now read from SQLite instead of file I/O
- `observe` hook writes to SQLite first, falls back to JSONL on write failure
- `reflect` hook reads/writes metrics and evolution data from SQLite
- `snapshot` hook syncs sessions and orbit pipelines to SQLite
- Web dashboard HTML response now includes `Cache-Control: no-cache` header to prevent stale UI

### Migration Guide

Existing users with JSONL/JSON data should run once after upgrading:

```bash
epic-harness migrate --dry-run   # preview what would be imported
epic-harness migrate             # perform the import
```

Original files are **not deleted** after import. New users are automatically on SQLite — no action needed.

## [0.4.9] — 2026-05-29

### Added
- Dashboard: auto-build Tauri app in CI, agent dismiss action, orbit pipeline dismiss
- README: static badge links fixed

## [0.4.8] — 2026-05-28

### Added
- Dashboard: orbit full-auto mode, pipeline dismiss, agents done-separation, Ring 1 skill rename

## [0.4.7] — 2026-05-27

### Fixed
- Dashboard: dynamic version injection, evolution history newest-first sort, council auto-proceed, security hardening

## [0.4.6] — 2026-05-26

### Added
- Install: brew + cargo-binstall cascade fallback when binary not found

### Changed
- Rename `check` skill to `audit` for consistency with pipeline naming
- Remove Antigravity integration section from docs

## [0.4.5] — 2026-05-26

### Added
- Antigravity (Gemini CLI) plugin package with install docs and i18n translations
- Cursor integration: sessionStart + preCompact hooks, `.cursor/skills/` generation
- Plugin manifests: declare hooks and mcpServers in Claude/Codex plugin.json

### Changed
- Cursor: migrate commands to skills format
- Antigravity: switch from install.rs to plugin-only distribution
- i18n: sync installer URLs, remove stale Codex install command
- README: rewrite top description for multi-tool positioning
- Remove deprecated Codex and Antigravity file-based plugin files

### Fixed
- Dashboard: i18n double-call in Agents page

## [0.4.4] — 2026-05-25

### Changed
- Unified all commands into skills, removing the separate command layer

## [0.4.3] — 2026-05-25

### Fixed
- Hooks: use event-specific stdout for codex/antigravity protocol compatibility

### Changed
- README updated for v0.4.2 release
- Codex removed from install wizard

## [0.4.2] — 2026-05-24

### Added
- Antigravity (Gemini CLI fork) integration: extension manifest, hooks, skills, commands
- `_dispatch` skill: core router that auto-invokes matching skills on context signals
- `agent-introspection` skill: failure recovery on 3+ consecutive errors
- `reflect` skill: 5-dimension evidence-based self-assessment
- `orchestrate` skill: multi-agent orchestration status and control
- `discover` skill: problem discovery via 5 Whys, JTBD, Socratic method
- `context` skill: session restoration from snapshots

### Changed
- Consolidated `commands/`, `skills/`, `agents/` into unified `registry/` structure (#26)
- Replaced gemini-cli integration with antigravity plugin
- Fixed 16 accuracy issues across skills, commands, presets, and integrations
- Optimized 12 skill/command description fields (~236 tokens saved)
- Fixed `_dispatch` and `reflect` referencing non-existent `mem_query` → `mem_list`
- Fixed `orchestrate` inbox path accuracy
- Fixed `orbit` evolve trigger logic for CI failures
- Fixed `evolve` cross-project path and `resume.ts` → `resume.rs`
- All preset skills now include required `## Red Flags` section
- Added 4 structural sections (Process, Anti-Rationalization, Evidence Required, Red Flags) to `_dispatch`

### Fixed
- Orchestrate test race condition with `serial_test::serial`
- ARM64 Linux CI build failure: lowered `rust-version` to 1.87.0
- ARM64 CI runner updated to `ubuntu-24.04-arm` via cargo-dist custom runners

## [0.3.3] — 2026-05-09

### Added
- Agent judgment known issues section in all READMEs
- Version bump checklist in AGENTS.md (Cargo.toml, package.json, plugin.json, git tag)

### Changed
- Reorganized project structure: `commands/`, `skills/`, `agents/`, `presets/` → `registry/`; docs → `docs/`
- Renamed syntagma references to episteme across docs and build
- `/orbit` isolates pipeline in git worktree to prevent session conflicts
- AGENTS.md synced with latest project content (replaced outdated template)
- CLAUDE.md consolidated to reference AGENTS.md
- Marketplace.json descriptions updated (6 → 8 commands)
- 9 i18n READMEs added (de, es, fr, hi, ja, ko, pt-BR, zh-CN, zh-TW)

## [0.2.1] — 2026-04-25 [YANKED]

> **YANKED**: This release wrote Claude hook commands containing `${CLAUDE_PLUGIN_ROOT}` into global `~/.claude/settings.json`.  
> That variable is only available in plugin-scoped hooks, causing runtime hook errors in global settings.

### Fixed
- **`epic install claude` now installs hooks**: Claude integration now syncs hook definitions into `~/.claude/settings.json` (from embedded `hooks/hooks.json`) instead of reporting zero installable files, so PreToolUse/PostToolUse/Session hooks are actually applied after install.

### Changed
- Install help/list output now includes `claude` in both install and uninstall integration lists.
- Added regression tests for Claude install behavior:
  - `settings.json` hook merge semantics are validated.
  - Claude canonical file generation remains explicitly empty (`generate_canonical_files("claude")`).

## [0.2.0] — 2026-04-19

### Added
- **Unified Memory system** (`harness mem`): cross-agent knowledge graph stored in `~/.harness/memory.db` (SQLite + FTS5), shared by all supported coding agents
  - 15 CLI subcommands: `add`, `edit`, `delete`, `query`, `search`, `related`, `link`, `graph`, `export`, `serve`, `validate`, `migrate`, `context`, `mcp`, `mcp-install`
  - **SQLite + FTS5 storage**: single `~/.harness/memory.db` — fast full-text search, ACID transactions, WAL concurrency; no rg/grep subprocess
  - **Auto-migration**: legacy `nodes/*.md` + `edges.jsonl` automatically imported into SQLite on first run
  - **`mem export`**: dumps all nodes to `~/.harness/exports/<id>.md` for Git-diffable plain-text backup; supports `--out <dir>` and `--dry-run`
  - Knowledge graph: typed nodes (concept/pattern/project/decision/error) + directed edges (uses/extends/conflicts/replaces/related/caused_by)
  - Web UI: `harness mem serve` → `http://localhost:7700` — D3.js force-directed graph, realtime search, CRUD, EN/KO language toggle, dark theme
  - **MCP server** (`epic-harness mem mcp`): native Rust stdio JSON-RPC 2.0 server, 5 tools — no Node.js required; register via `harness mem mcp-install [--force]`
  - Auto-recording: PostToolUse hook detects decisions/patterns → auto-stores (fire-and-forget, secret-masked)
  - Session context injection: relevant project memories injected at session start via `resume` hook
  - Migration: `harness mem migrate --all [--dry-run]` converts existing per-project memories to unified store
  - Security: 127.0.0.1 binding, UUID v4 strict path validation, secret masking, sensitive file path filtering
  - > ⚠️ **This feature is still in development.** Some capabilities described above may not yet be functional.
- **opencode integration**: JS plugin (`plugins/epic-harness.js`) for session/tool lifecycle hooks, 6 commands, 4 agents → `~/.config/opencode/`
- **cline integration**: 5 executable hook scripts (PreToolUse/PostToolUse/TaskStart/TaskResume/TaskCancel) → `~/Documents/Cline/Rules/Hooks/`
- **aider integration**: `.aider.conf.yml` + `.aider/CONVENTIONS.md` — no hook system, conventions auto-loaded via `read:` config
- **Interactive install menu**: `epic-harness install` (no args) shows numbered checklist; select by number (e.g. `1,3`) or `a` for all
- **Progress bar**: TTY shows animated `[====>   ] N/M filename`; non-TTY shows one-line summary per tool
- **`epic team` / `epic org`**: org-level agent team management — persistent team definitions that accumulate knowledge across projects
  - 9 CLI subcommands: `list`, `show`, `status`, `sync`, `link`, `unlink`, `delete`, `history`, `help`
  - Interactive team designer with auto stack detection (Rust/Node/Python/Go/Java)
  - Agent CRUD with mission, playbook (append-only), and history backup
  - Cross-tool sync: copies agents to `.claude/agents/{team}/` per project; `--global` for `~/.claude/agents/`
  - Project linking: `epic team link` binds team to current project; `status` shows linked teams
  - Security: path traversal prevention (`[a-zA-Z0-9_-]` allowlist), YAML injection defense (`yaml_quote`), Unicode prompt injection stripping (Plane-14, C0/C1 controls), HTML comment sanitization, ANSI escape filtering
  - Org browsing: `epic org` to browse team libraries across organizations

### Changed
- **`epic install claude` plugin cache sync**: now overwrites `~/.claude/plugins/cache/epicsagas/epic/*/` with the commands/skills/agents embedded in the binary — local changes take effect immediately without waiting for an npm publish
- **Codex skills path**: now installs to `~/.agents/skills/` (official Codex discovery path; was `~/.codex/skills/` which is not scanned)
- **Codex commands**: renamed `commands/` → `prompts/` → `~/.codex/prompts/`; invoke as `/prompts:check` etc.
- **Codex hooks**: require `features.codex_hooks = true` in `~/.codex/config.toml` (off by default); install now writes this config and warns if an existing config lacks the flag
- **Codex PostToolUse**: removed non-functional Edit/Write matchers (Codex only intercepts Bash); removed unsupported `async` flag
- Install output: replaced per-file `println!` with progress bar; errors still surfaced via `eprintln!`

### Removed
- `integrations/antigravity/` — Antigravity is an IDE-level setting; file-based install is not the right mechanism
- `install.sh` — fully superseded by `epic-harness install` Rust subcommand

## [0.1.3] — 2026-04-09

### Fixed

- Shell injection vulnerability in hook command dispatch
- Guard rule matching consistency across blocked/warned rule evaluation
- Reflect analysis correctness (session scoring, trend calculation edge cases)

## [0.1.2] — 2026-04-09

### Fixed

- Plugin install commands corrected from `/plugin` to `claude plugins` CLI syntax in all docs

## [0.1.1] — 2026-04-09

### Added

- Multi-language README for top 10 Claude Code countries (10 locales)
- npm publish step in release CI workflow
- Linux arm64 binary target in release builds
- `cargo install` and `cargo binstall` install methods documented
- Homebrew tap (`epicsagas/tap`) integration in CI and install docs

### Fixed

- Hook dispatch now checks `PATH` before falling back to Node.js scripts
- Homebrew tap path shortened to `epicsagas/tap` across all i18n READMEs
- Broken CI badge removed from all READMEs
