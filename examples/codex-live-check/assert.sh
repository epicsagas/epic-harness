#!/usr/bin/env bash
# codex-live-check verdicts: offline assertions, zero tokens.
# usage: assert.sh [1|2|3|4|all]
set -uo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
. "$DIR/.envrc"
DB="$HARNESS_DIR/harness.db"
PROJ="$HARNESS_DIR/projects/epic-harness"
FAILS=0

ok()   { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; FAILS=$((FAILS+1)); }
note() { echo "  SKIP  $1"; }

q() { sqlite3 "$DB" "$1" 2>/dev/null; }

case1() {
  echo "[case 1] apply_patch payload shape"
  [ -f "$DB" ] || { bad "harness.db missing; did any turn run?"; return; }
  local total body
  total=$(q "select count(*) from observations where tool='apply_patch';")
  [ "${total:-0}" -gt 0 ] || { bad "no apply_patch observations (hook not firing or Codex did not edit)"; return; }
  ok "$total apply_patch observation(s)"
  body=$(q "select count(*) from observations where tool='apply_patch' and action like '*** Begin Patch%';")
  [ "${body:-1}" -eq 0 ] && ok "no action stores a patch body" || bad "$body action(s) store patch bodies"
  local badext leaked
  badext=$(q "select count(*) from observations where tool='apply_patch' and file_ext != '.py';")
  [ "${badext:-0}" -eq 0 ] && ok "file_ext correct" || bad "$badext row(s) with wrong file_ext"
  leaked=$(q "select count(*) from observations where action like '%$HOME%';")
  [ "${leaked:-0}" -eq 0 ] && ok "no home dir in actions" || bad "$leaked action(s) leak the home dir"
}

case2() {
  echo "[case 2] resume injection / holdout stderr"
  local log="${2:-$DIR/stderr.log}"
  [ -f "$log" ] || { note "no stderr.log; launch codex with: codex 2> $DIR/stderr.log"; return; }
  # Scope to hook-emitted lines: codex echoes the prompt and the model's own
  # answer (which quotes a marker by design) to its stderr, so a bare
  # HARNESS-CHECK grep false-positives on the success case itself.
  if grep -qE "^\[(resume|polish|guard)\].*HARNESS-CHECK" "$log"; then
    bad "hook-emitted markers leaked on stderr: injection should be stdout-only"
  else
    ok "no hook-emitted marker text on stderr"
  fi
  grep -qi "holdout" "$log" \
    && note "holdout announcement present on stderr (expected if a seed landed holdout)" \
    || ok "no holdout announcement (all seeds active, or host drops stderr)"
  echo "        manual check: the model must quote a HARNESS-CHECK marker verbatim. NONE = fail."
}

case3() {
  echo "[case 3] subagent events + orchestrator state"
  local run subs done
  run=$(find "$PROJ" -name run.json -path "*orchestrator*" 2>/dev/null | head -1)
  if [ -z "$run" ]; then
    note "no orchestrator run.json (did the team turn run? epic team link first?)"
    return
  fi
  ok "run.json at $run"
  jq -r '.. | objects | select(has("agent_id")) | .agent_id' "$run" 2>/dev/null | sort -u | head -3
  # The load-bearing claim (#14/#15): codex subagents reach Running keyed by
  # the host agent_id and complete. Lifecycle lives in run.json; observe
  # handles Subagent* for orchestration without recording observations, so
  # their count is informational only.
  done=$(jq -r '[.. | objects | select(.status? == "done" and .started_at? != null and .completed_at? != null)] | length' "$run" 2>/dev/null)
  [ "${done:-0}" -gt 0 ] && ok "$done agent(s) went running->done with host agent ids" \
    || bad "no agent completed a running->done lifecycle in run.json"
  subs=$(q "select count(*) from observations where tool like 'Subagent%';")
  [ "${subs:-0}" -gt 0 ] && note "$subs Subagent* observation(s) also recorded" \
    || note "no Subagent* observations (observe consumes these for orchestration only)"
}

case4() {
  echo "[case 4] watermark, attribution, reflect"
  local w="$PROJ/reflect_watermark.txt" nulls
  [ -f "$w" ] && ok "watermark: $(cat "$w")" || note "no watermark yet (needs a Stop with >= 3 observations)"
  nulls=$(q "select count(*) from observations where project is null or project = '';")
  [ "${nulls:-1}" -eq 0 ] && ok "observations project-attributed" || bad "$nulls observation(s) with empty project"
  [ -f "$PROJ/metrics.json" ] && ok "metrics: total_sessions=$(jq -r '.total_sessions // 0' "$PROJ/metrics.json")" \
    || note "no metrics.json yet"
}

# `all` must run every case — a `for c in all` loop stops after `1|all`
# matches, silently skipping cases 2-4 (measured on the first live run).
case "${1:-all}" in
  all) case1; case2; case3; case4 ;;
  1) case1 ;;
  2) case2 ;;
  3) case3 ;;
  4) case4 ;;
esac

echo
if [ "$FAILS" -gt 0 ]; then echo "RESULT: $FAILS FAIL"; exit 1; fi
echo "RESULT: PASS (or SKIP-only)"
