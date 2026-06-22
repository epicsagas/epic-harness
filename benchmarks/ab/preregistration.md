# Pre-registration — Bare-vs-Epic A/B (SWE-bench Verified)

**Locked before the pilot run.** Any change to hypotheses, metrics, SEOI, band assignment, or
the decision rule after data collection begins must be recorded in the **Amendments** section
with date + reason. This exists to prevent the smoke's misread (a feasibility ceiling effect
sold as a value finding) from recurring.

Full methodology: [`METHODOLOGY.md`](./METHODOLOGY.md). This file is the locked subset: what we
predict, what counts as evidence for each side, and the stopping/decision rules.

## Constants (locked)

- **Model/router held fixed:** GLM-5.2[1m] via `claudy zai`. Only `epic@epicsagas` plugin
  varies between arms.
- **Surface under test:** single-shot `-p` coding prompts (matches the smoke; keeps
  `num_turns` comparable across arms; isolates plugin *skills* from its *orchestrator*).
  Orbit/council get a separate, explicitly-invoked arm if single-shot engagement is low.
- **Smallest effect of interest (SEOI):** **+9pp aggregate**, **+18pp per band**. Bands whose
  achieved MDD exceeds this are reported **inconclusive-by-design**, never as null.
- **Difficulty bands:** B1 easy / B2 medium / B3 hard / B4 hardest, from committed
  `difficulty_map.json` (4 quantiles of `z(net_loc)+z(files_touched)+z(f2p_count)`).
- **NETS pre-flight gate:** non-trivial fraction must be **> 85%** of the drawn sample.
- **Multiplicity:** Benjamini–Hochberg FDR=0.05 across ~5 independent families (4 per-band
  McNemar + 1 pooled).
- **Cost headline:** value-normalized **CPRI** ($/resolved), never absolute tokens.

## Hypotheses

| Dim | Null H₀ | Favors epic | Favors bare |
|-----|---------|-------------|-------------|
| **Correctness** | C1 net-new delta = 0 and C2 per-band deltas = 0 | C1 > 0 concentrated in B4; C2 B4 McNemar p<0.05; C3 conditional partial-credit higher for epic | C1 < 0 (bare resolves more), or C1=0 with higher C5 refusals for epic (over-cautious) |
| **Robustness** | R1 recovery (genuine errors) equal; R2 loop-incidence equal; R4 PTP_involvement equal | R1 higher for epic; R2 lower incidence AND higher loop-recovery; R4 PTP_involvement lower | R1 lower; R2 higher; R4 higher |
| **Security** | S1 clean-rate equal on resolved∩security subset; S4 critical-intro equal | S1 higher for epic on engaged subset; S2 weight gap driven by CRITICAL/HIGH; S4 epic ≥ bare | S1 lower or equal-with-low-engagement (skills fire but don't prevent); **S4 epic < bare = red flag** |
| **Process** | CPRI ratio = 1 on hard band; P2 rework equal; P3 slope equal | CPRI ≤ 1.5 with C1>0; P2 lower; P3 slope more negative for epic | CPRI > 1.5; P2 higher; P3 slope flat/positive for epic |
| **Localization** | arm:band interaction = 0 (D3) | Positive interaction (epic benefit grows with difficulty) | Negative/flat interaction (overhead uniform) |

## Decision rule (locked) — see METHODOLOGY.md §4

- **STATE A (value > cost, VPC > 0):** net-new delta > 0 pooled AND McNemar p<0.05 in B3 or B4
  AND CPRI ≤ 1.5 on hard band (or C1 large enough) AND S4 epic ≥ bare AND S3 engagement high.
- **STATE B (cost > value, VPC ≤ 0):** net-new delta ≤ 0 with non-significant per-band tests,
  OR CPRI > 1.5 with C1 ≈ 0, OR S4 epic < bare.
- **STATE C (inconclusive by design):** B3/B4 underpowered (MDD > +18pp) and observed delta
  inside that interval → "directional only," never null.

## Interpretation rules (locked, anti-p-hack)

- **Localization hypothesis is the cleanest value signal:** a positive `arm:band` interaction
  (D3) means epic's benefit scales with difficulty — overhead buying capability on hard tasks.
- **Security null rule:** if S3 skill engagement is low on single-shot runs, an S1/S2 null is
  recorded as *"security path not exercised by this harness; run an explicit
  `/epic:secure`- or `/epic:orbit`-driven arm before concluding,"* **not** "epic provides no
  security value."
- **A tie is not evidence of zero value on underpowered bands:** STATE C is a first-class
  outcome.

## Pilot (gate to full run)

20 instances (5 per band), ≥3 seeds per (arm, instance). Estimates GLM run-to-run variance,
`|E_i|` (error instances for R1), and NETS coverage. **Go to full 500 only if** pilot confirms
the design has power at SEOI; if seed-variance flips the net-new delta sign, require ≥3 seeds
on the full B3∪B4 band.

## Amendments

(none yet)
