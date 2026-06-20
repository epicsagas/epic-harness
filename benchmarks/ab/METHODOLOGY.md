# epic-harness Bare-vs-Epic A/B Methodology — SWE-bench Verified Main Run

**Status:** pre-registration spec for issue #94's main run. Committed **before** any main-run
data is collected. Produced by a multi-agent design+critique+synthesis pass; load-bearing
source references were verified against the tree (see "Verified source references" below).

**Scope:** plugin-only A/B (epic loaded vs not) on real SWE-bench Verified instances, model
+router held fixed (GLM-5.2[1m] via `claudy zai`). Goal: measure epic's **value**
(correctness, robustness, security, process), not only its cost. The benchmark must be able
to conclude either direction.

Items marked **[BLOCKER]** must land before any main-run number is trusted.

---

## 1. Why "epic = pure overhead" was wrong

The smoke headline is a **measurement artifact of task selection**, not a finding about the
plugin. Three structural reasons:

1. **Binary pass@1 saturates on trivial tasks.** Both arms solved `is_palindrome` and
   `moving_average` (single-function, 1-file, ≤1 FAIL_TO_PASS). When both arms pass, the only
   measurable axis is cost — so "9–20× cost = pure overhead" is the *only* conclusion the
   instrument could ever produce on those tasks. It is tautological, not empirical.
2. **Trivial tasks structurally cannot engage epic's skills.** spec/tdd/secure/debug/orbit/
   council/verify target *multi-file, multi-step, genuinely-hard* work. A one-liner fix has
   no requirements to spec, no regression surface to verify, no threat to model. The smoke
   ran tasks where the value mechanism is offline, then measured value as zero — a ceiling
   effect on the dependent variable.
3. **Cost was framed absolutely, not value-normalized.** "9–20× cost" divides total tokens by
   *attempts*, not by *resolved instances*. On a tie (both resolve), per-attempt cost penalizes
   the arm that spent more, but that is the wrong denominator. The fair question is
   *tokens-per-instance-resolved-that-bare-failed*, i.e. the marginal cost of marginal value.
   On trivial tasks that denominator is empty (bare resolves everything), so the metric is
   undefined and defaults to "pure overhead."

The smoke was a valid **feasibility** test (toggle works, GLM drives headless, metrics
captured — its actual verdict "FEASIBILITY: GO"). It was never a valid **value** test. The
main run below is the value test.

---

## 2. Task selection

### 2.1 Difficulty stratification — committed proxy, not a fictional column

SWE-bench Verified has **no native `difficulty` field**. We reconstruct bands **offline from
the dataset's own `patch` field** and commit the mapping file before any run.

**Band assignment (deterministic, committed to `benchmarks/ab/difficulty_map.json`):**
- Extract from each instance's gold `patch`: `files_touched` (distinct paths in diff headers),
  `net_loc` (sum of +/- lines), `f2p_count` (len of `FAIL_TO_PASS`).
- 4 quantile bands by composite score `s = z(net_loc) + z(files_touched) + z(f2p_count)`:
  - **B1 (easy):** bottom quartile · **B2 (medium):** 25–50th · **B3 (hard):** 50–75th ·
    **B4 (hardest):** top quartile
- Band-assignment code lives in `benchmarks/ab/assign_bands.py`; `difficulty_map.json` is the
  single source of truth for every stratified metric.

**Triviality flag (skill-engagement capacity, not raw LOC):**
`trivial = (B1) AND (files_touched ≤ 1) AND (f2p_count ≤ 1) AND (problem_statement contains
a localized failing-test reproduction)`. Flagged, not silently dropped. **NETS coverage =
non-trivial / total**; pre-flight gate: trivial fraction must be **< 15%** of the drawn sample,
else redraw or oversample B3/B4.

### 2.2 Host architecture — C-extension repo handling  [BLOCKER]

arm64 cannot grade scikit-learn/scipy-family instances (no `gfortran` → Fortran
`CCompiler` NameError; salvage confirmed not worth it — see Appendix of `SMOKE-REPORT.md`).
Two accepted paths:
- **(Preferred)** Run the swebench grading on a **native x86_64 host** so the full Verified set
  is gradeable.
- **(Fallback)** Pre-declare the gradeable-on-target-host subset in
  `benchmarks/ab/gradeable_subset.json` *before* any run; report robustness/security/correctness
  metrics only on that subset; **never** let an instance silently fail-to-grade and count as a
  bare/epic loss. Record `host_arch` in every artifact row.

### 2.3 Security-relevant subset — deterministic, both-arms, from gold patch

Computed **once** before the run, applied identically to both arms:
- `security_relevant = (gold patch path matches /(auth|login|session|password|token|permission
  |middleware|views|forms|query|sql|serializ|crypto|hash|secret)/i) OR (Semgrep p/owasp-top-ten
  + Bandit on the gold patch's added lines yields ≥1 finding)`.
- Report the actual N. Do **not** assume 60–120. If N < 40 for the binary clean-rate metric or
  < 10 for CRITICAL-tier, mark the result **descriptive-only / underpowered**.

### 2.4 Sample size, seeds, budget — honest power

**Reality:** exact McNemar needs d ≥ ~55 discordants for a +5pp gap at power 0.8; realistic
+5pp at base resolve 0.3 yields ~75 discordants on n=171/band → **minimum detectable delta
(MDD) ≈ +18pp per band, ≈ +9pp pooled across 500**. Therefore:

- **Smallest effect of interest (SEOI) = +9pp aggregate**, not +5pp. Pre-register this. Bands
  where observed MDD > +9pp are reported **inconclusive-by-design**, never as null.
- **Paired design:** both arms see every instance, independent fresh workdirs (cwd-isolation
  guard), `n_B` per band.
- **Seeds:** ≥3 seeds per (arm, instance) on a 20-instance pilot to estimate GLM run-to-run
  variance, then ≥1 seed on the full 500. If pilot seed-variance flips net-new delta sign, run
  ≥3 seeds on the full hard band (B3+B4) only. McNemar/Wilcoxon on n=1/seed data is
  **directional only**; require a confirmatory seed if the headline is borderline.
- **Per-band n:** derive from `difficulty_map.json` quantiles. Expect ~125/band × 4 = 500.
- **Total budget:** 500 × 2 arms × ~$0.6/epic + ~$0.06/bare ≈ **$660 main run**; +20-instance
  pilot ≈ **$30**. Plus grading host.

### 2.5 Seed/workdir isolation  [BLOCKER]

Each `(arm, instance, seed)` cell gets a **fresh independent `mktemp` workdir** (the smoke's
cwd-contamination bug). Paired ≠ shared workdir.

---

## 3. Metrics table

All metrics computed from **two committed artifacts**: (a) the swebench per-instance grading
JSON (`tests_status`: per-test PASSED/FAILED for both FAIL_TO_PASS and PASS_TO_PASS), and (b)
the per-cell `stream-json` transcript NDJSON. Weights are for the composite score (§3.6); they
sum to 1.0 across the 5 dimensions.

### 3.1 Correctness & Completion (weight 0.30)

| # | Metric | Measures | Computation | Units | W |
|---|--------|----------|-------------|-------|---|
| C1 | **Net-new resolution delta** | epic resolves instances bare fails (load-bearing value signal) | paired 2×2 per instance: `N_epic_only − N_bare_only`, per band + pooled | int | 0.10 |
| C2 | **Resolution-rate delta, per band** | resolve rate (all F2P pass AND all P2P pass) delta, stratified | `delta_B = epic_rate_B − bare_rate_B`; McNemar exact on discordants per band | pp | 0.06 |
| C3 | **Partial-credit f2p_coverage** | how close unresolved runs got | `#F2P_passing / #F2P_total`; **conditional mean over instances neither arm resolved**; paired Wilcoxon | [0,1] | 0.04 |
| C4 | **PASS_TO_PASS regression integrity** | solve-without-breaking | `P2P_broken_rate = #(P2P has ≥1 failing) / #(non-empty patch)`; report jointly with f2p_coverage (2D scatter) | [0,1] | 0.06 |
| C5 | **No-patch / refusal count** | conservative-skipping vs clean-solving | `#(empty or near-empty model_patch)` | count | 0.04 |

*Original "first-try success" metric dropped — on a single-shot `-p` runner it collapses to
resolution rate.*

### 3.2 Robustness (weight 0.20)

All metrics computed **identically for both arms by replaying the transcript** through one
shared `replay_transcript(path) -> Vec<ObsRecord>` using `src/shared/classify.rs`
(`classify_failure`/`classify_tool`) + `src/shared/helpers.rs` (`normalize_error`/`hash_string`).
**Never** use epic's live obs JSONL as the scored instrument (its truncation/masking differ
from a clean replay; report only as a sanity check).

| # | Metric | Measures | Computation | Units | W |
|---|--------|----------|-------------|-------|---|
| R1 | **Recovery rate (genuine errors)** | convert mid-run error → resolved | `E_i` = instances with ≥1 **neutral** error (Bash non-zero exit on a non-test command, or build/runtime error) — NOT epic's `failure_category` (which includes TDD "Red"). `RecoveryRate = #resolved-in-E_i / #E_i`; Wilson CI; 2-prop z-test | [0,1] | 0.06 |
| R2 | **Stuck-loop incidence + loop-recovery** | dead-end escape | Replay `detect_patterns` (`analysis.rs:225`) over both arms' transcripts with `CONFIG.pattern` thresholds pinned in the report. Loop-positive = ≥1 of {repeated_same_error, fix_then_break, long_debug_loop, thrashing}. Report incidence AND, among loop-positive, resolve rate | [0,1] | 0.05 |
| R3 | **Budget-normalized exit on UNRESOLVED** | graceful failure vs burn-to-budget | `frac_max_turns_consumed = num_turns / MAX_TURNS` + `explicit_stop` | [0,1] | 0.04 |
| R4 | **Regression discipline (3-bucket)** | collateral breakage vs never-fixed | UNRESOLVED partitioned into FTP_only / PTP_only / BOTH_broke; headline = `PTP_involvement`; clean verify-gate signal = among final patches passing all F2P, fraction that broke P2P | [0,1] | 0.05 |

### 3.3 Security (weight 0.20)

**Patch source = the graded artifact only** (`model_patch` from `predictions.json`, applied to
a clean base-commit checkout inside the grading container) — **never the workdir** (which
contains THREAT_MODEL.md/VULN-FINDINGS.json that would manufacture false "epic introduces
vulns"). Semgrep/Bandit/pip-audit run on that applied tree, restricted to model_patch hunk
lines. Security-artifact files (`*.md` reports) excluded from scan.

| # | Metric | Measures | Computation | Units | W |
|---|--------|----------|-------------|-------|---|
| S1 | **No-introduced-vuln rate (security subset, resolved)** | clean-rate on security-relevant tasks | on RESOLVED∩security-relevant, `clean_i = 1 if new_findings==0`; paired Wilcoxon on within-instance weight diff over **both-arms-resolved intersection**; 2-prop z-test; Wilson CI. N<40 → descriptive | [0,1] | 0.05 |
| S2 | **Severity-weighted vuln burden** | does epic avoid CRITICAL/HIGH, not just LOW? | CWE-weighted (CRITICAL=10/HIGH=5/MED=2/LOW=1) mean new-findings per resolved security instance; per-CWE breakdown. CRITICAL-tier N<10 → descriptive | pts/inst | 0.04 |
| S3 | **Skill engagement (mechanism, epic-only)** | did the skill fire? (interprets S1/S2 null) | Engaged iff event-stream tool_use is a **security scanner invocation** (semgrep/bandit/pip-audit/cargo-audit as executed binary) OR an **epic skill token** (`/epic:secure`, `/epic:vuln-scan`, `/epic:threat-model`, `/epic:triage`) OR artifact file present. Drops the regex grep on file content (false-fires) and drops the dispatch-log condition (no reliable writer). Cross-tab: clean_rate(engaged) vs (not-engaged) vs bare | [0,1] | 0.03 |
| S4 | **Critical-intro safety rate (ALL resolved)** — co-headline | does an arm inject a CRITICAL/HIGH into a non-security fix? | across ALL resolved, Semgrep p/owasp-top-ten + Bandit CRITICAL+HIGH on base vs patched, non-test paths; `safety_rate = mean(1 − critical_intro)`. Largest N; **epic < bare here = serious red flag regardless of other dims** | [0,1] | 0.08 |

### 3.4 Process Efficiency (weight 0.15)

**[BLOCKER]** Runner must emit `--output-format stream-json` (§6) before these are computable.

| # | Metric | Measures | Computation | Units | W |
|---|--------|----------|-------------|-------|---|
| P1 | **Cost-per-resolved-instance (CPRI), full pool + hard band** | value-normalized cost | `CPRI(a) = Σ total_cost_usd / #resolved`; ratio `epic/bare`; hard-band = B3∪B4; bootstrap 95% CI. **Min-n rule:** if `#resolved < 5/arm/stratum`, report "unstable (low n)" and fall back to absolute resolved-rate delta + absolute cost | $/resolved | 0.06 |
| P2 | **Rework ratio** | wasted tool calls | `WastedPerResolved = Σ wasted_calls / #resolved`, wasted = tool_result `is_error=true` OR same-file edit later reverted OR ≥5 consecutive same-file edits without a passing test; from stream-json, identically for both arms | calls/resolved | 0.05 |
| P3 | **Turns-to-solution slope (single-shot surface only)** | does epic scale better with difficulty? | `num_turns ~ band` regression per arm; compare slopes. **Single-shot `-p` only** — if orbit is driven, num_turns becomes orchestrator-turns and cross-arm TTS is forbidden | turns/band | 0.04 |

### 3.5 Statistical Power & Localization (weight 0.15)

| # | Metric | Measures | Computation | Units | W |
|---|--------|----------|-------------|-------|---|
| D1 | **Per-band McNemar + discordance** | per-band resolve delta with honest inferential test | McNemar exact per band; report `delta_B`, discordant counts, exact p, 95% exact CI; aggregate = fixed-effects secondary only | pp + p | 0.05 |
| D2 | **Achieved power / MDD** | did we actually have power? | `q_B = discordants_B/n_B`, `f_B = epic_only/discordants`; MDD_B from McNemar power curves; flag UNDERPOWERED if MDD_B > SEOI (+9pp pooled, +18pp/band) | pp | 0.03 |
| D3 | **arm × band interaction (effect localization)** | does epic's benefit scale with difficulty? | logistic `resolved ~ 1 + arm + band(c) + arm:band + (1|repo)`; positive interaction = epic value scales with hardness (value hypothesis) | coef ± SE | 0.04 |
| D4 | **NETS coverage (pre-flight gate)** | is the sample capable of engaging skills? | non-trivial fraction; must be > 85% pre-run | [0,1] | 0.03 |

### 3.6 Composite "value-per-cost" score

`VPC = (Σ value components normalized to [0,1], epic-favorable direction) − λ × CPRI_hard_ratio`

where `CPRI_hard_ratio = CPRI_epic_hard / CPRI_bare_hard` (capped at 3.0 to avoid infinity at
zero; reported "unstable" if low-n). `λ = 0.15` (cost is one input, not the headline).
**VPC > 0 ⇒ value exceeds cost; VPC ≤ 0 ⇒ cost exceeds value.** VPC is reported alongside its
components — never as an opaque single number.

**Multiplicity:** apply **Benjamini–Hochberg FDR=0.05 across ~5 independent families**
(4 per-band McNemar tables + 1 pooled), not a nominal "20 tests."

---

## 4. Value-justifies-cost decision rule

Pre-registered. The benchmark concludes one of three states:

**STATE A — epic's value exceeds cost (VPC > 0):** requires ALL of
1. **Net-new resolution delta > 0** pooled (C1), AND per-discordant McNemar p < 0.05 in at
   least one of B3/B4, AND
2. **CPRI ratio ≤ 1.5** on the hard band (B3∪B4) OR a positive C1 large enough that
   `cost / max(1, N_epic_only)` is acceptable, AND
3. **S4 critical-intro safety: epic ≥ bare** (no red flag), AND
4. **Skill engagement (S3) measurably high** on the security subset (else a null S1/S2 is
   "skill didn't fire," not "skill failed" — see §5).

**STATE B — cost exceeds value (VPC ≤ 0):** any of
1. Net-new delta ≤ 0 pooled AND per-band McNemar non-significant after BH-FDR, OR
2. CPRI ratio > 1.5 on hard band with C1 ≈ 0 (extra tokens buy nothing), OR
3. S4 epic < bare (actively harmful on critical-intro).

**STATE C — inconclusive by design:** if D2 flags B3/B4 as UNDERPOWERED (MDD > +18pp) AND the
observed delta is inside that interval, the verdict is **"benchmark not powered to resolve this
band; directional only"** — never re-purposed as a null.

**Cost is reported value-normalized (CPRI, $/net-new-resolved) as the headline cost number,
never absolute tokens.** The absolute per-attempt token delta (~85k) is reported as context only.

---

## 5. Pre-registered hypotheses

See `benchmarks/ab/preregistration.md` for the full table. Headline interpretation rules:

- **Localization value hypothesis:** a positive `arm:band` interaction (D3) is the cleanest
  single signal that epic's value scales with difficulty — i.e. that the overhead is buying
  capability that activates on hard tasks.
- **Security null rule (pre-committed):** if S3 engagement is low on single-shot `-p` runs
  (skills may not trigger without explicit invocation — the smoke's own caveat), an S1/S2 null
  is recorded as **"security path not exercised by this harness; run an explicit
  `/epic:secure`- or `/epic:orbit`-driven arm before concluding,"** NOT "epic provides no
  security value."
- **Surface under test (pre-committed):** the main run is **single-shot `-p`** (matches the
  smoke, makes num_turns comparable across arms, isolates the plugin's *skills* from its
  *orchestrator*). Orbit/council are evaluated in a separate, explicitly-invoked arm if the
  single-shot arm shows low skill engagement — because num_turns under orbit counts
  orchestrator-turns (different unit) and cross-arm TTS is then invalid.

---

## 6. Concrete revision to PR #96

### 6.1 SMOKE-REPORT framing fix (no new runs needed)

The current report is a valid feasibility test mislabeled as a value finding. Three edits
(applied in this PR):

1. **Rename the headline.** "On these tasks, epic = pure overhead" → a statement that on
   trivial tasks the only measurable axis is cost (feasibility ceiling effect, not a value
   finding); the value test requires SWE-bench Verified (this document).
2. **Move the absolute-cost framing to context.** "9–20× cost" stays as the *per-attempt*
   overhead of context injection, with an explicit note that per-attempt cost penalizes the
   context-injecting arm regardless of whether that context buys value; the fair denominator is
   per-resolved-instance (CPRI), undefined on a tie.
3. **Add the toggle-impurity caveat.** `--bare` strips LSP/MCP/hooks *in addition to* the
   plugin, so the smoke is "epic+tooling vs stripped," not "plugin vs not-plugin."

### 6.2 Main-run runner changes — `run_smoke.sh` → `run_swebench.sh`

- **[BLOCKER 1 — output format]** `--output-format json` → **`stream-json`**, tee full NDJSON to
  `runs/{arm}/{instance}_{seed}.ndjson`. Without this, R1/R2/R3, P2, P3-slope, and S3-engagement
  are uncomputable for either arm.
- **[BLOCKER 2 — toggle purity]** `claude --bare` → **per-plugin `enabledPlugins` toggle via
  `--settings`** so ONLY `epic@epicsagas` varies; both arms retain LSP/MCP/hooks. Satisfies
  issue #94's "plugin is the only variable" and the smoke report's own requirement #3.
- **[BLOCKER 3 — transcript-replay symmetry]** one `replay_transcript(path) -> Vec<ObsRecord>`
  helper (`benchmarks/ab/replay.py` or Rust) running both arms' NDJSON through
  `src/shared/classify.rs` `classify_failure`/`classify_tool` + `normalize_error`/`hash_string`.
  Pin the exact `CONFIG.pattern` thresholds into the report as a versioned artifact.
- **[BLOCKER 4 — per-cell isolation]** each `(arm, instance, seed)` cell: fresh `mktemp`
  workdir, launched from inside it.
- **[BLOCKER 5 — host arch]** grade on x86_64, or pre-commit `gradeable_subset.json` + record
  `host_arch` per artifact row.

### 6.3 New committed artifacts (built by the runner, not prose)

| Artifact | Path | Contents |
|----------|------|----------|
| Difficulty map | `benchmarks/ab/difficulty_map.json` | `{instance_id: {band, files_touched, net_loc, f2p_count, trivial}}` |
| Gradeable subset | `benchmarks/ab/gradeable_subset.json` | `host_arch` + gradeable instance list |
| Security subset | `benchmarks/ab/security_subset.json` | `security_relevant` flag + actual N |
| Per-instance verdict table | `benchmarks/ab/verdicts.jsonl` | `instance_id, repo, band, host_arch, arm{f2p_coverage, p2p_failed, resolved, cost, turns, is_error, rc, failure_mode, seed}` |
| Per-cell transcripts | `runs/{arm}/{instance}_{seed}.ndjson` | full stream-json |
| Preregistration | `benchmarks/ab/preregistration.md` | hypotheses, SEOI, band assignments — before pilot |
| Pattern-threshold snapshot | in report output | `CONFIG.pattern` values used for scoring |

### 6.4 Grading-adjacent additions (post-grade, both arms)

- **Patch source for security scan:** read `model_patch` from `predictions.json`, apply to a
  clean base-commit checkout (exactly what the swebench container does), scan that tree with
  Semgrep p/owasp-top-ten + p/python + Bandit + pip-audit. Exclude `*.md`/security-artifact files.
- **Failure-mode tagging:** tag each unresolved cell as `wrong_patch | no_patch_budget |
  no_patch_turns | no_patch_error`. Report McNemar on resolved-by-tests **and** on
  "produced any/correct patch with budget-trimmed excluded" so "epic spent more → no patch" is
  visible, not buried in a tie.

---

## Engineer build order

1. **[BLOCKER]** Runner: stream-json + enabledPlugins toggle + fresh-workdir + x86_64 host
   (§6.2). Re-run smoke to reconfirm token delta is plugin-attributable.
2. Difficulty map + gradeable subset + security subset (§2.1, 2.2, 2.3, 6.3) — before pilot.
3. `replay_transcript` + pinned thresholds (§6.2 BLOCKER 3).
4. 20-instance pilot: estimate `|E_i|`, seed-variance, NETS coverage; confirm power or adjust
   SEOI (§2.4).
5. Full 500 paired run; emit `verdicts.jsonl` + per-cell NDJSON.
6. Compute all metrics from artifacts; apply BH-FDR; render verdict table + VPC.

---

## Verified source references

These were checked against the tree at methodology-design time (all confirmed present with the
cited signatures/lines):

- `src/shared/classify.rs:58` `classify_failure`, `:71` `classify_tool`
- `src/shared/helpers.rs:245` `hash_string`, `:256` `normalize_error`
- `src/evolve/analysis.rs:225` `detect_patterns` (emits `repeated_same_error`,
  `fix_then_break`, `long_debug_loop`, `thrashing`)
- `src/hooks/observe.rs` (epic-only obs hook; absent under a toggle-pure bare arm)
- Dispatch JSONL: documented in `registry/skills/_dispatch/SKILL.md` and `AGENTS.md` but has
  **no Rust writer** (created only if the LLM follows the skill instruction) → not used as a
  scored instrument.
