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
  local log="$DIR/stderr.log"
  [ -f "$log" ] || { note "no stderr.log; launch codex with: codex 2> $DIR/stderr.log"; return; }
  if grep -q "HARNESS-CHECK" "$log"; then
    bad "markers leaked on stderr: injection should be stdout-only"
  else
    ok "no marker text on stderr"
  fi
  grep -qi "holdout" "$log" \
    && note "holdout announcement present on stderr (expected if a seed landed holdout)" \
    || ok "no holdout announcement (all seeds active, or host drops stderr)"
  echo "        manual check: the model must quote a HARNESS-CHECK marker verbatim. NONE = fail."
}

case3() {
  echo "[case 3] subagent events + orchestrator state"
  local run subs
  run=$(find "$PROJ" -name run.json -path "*orchestrator*" 2>/dev/null | head -1)
  if [ -z "$run" ]; then
    note "no orchestrator run.json (did the team turn run? epic team link first?)"
  else
    ok "run.json at $run"
    jq -r '.. | objects | select(has("agent_id")) | .agent_id' "$run" 2>/dev/null | sort -u | head -3
  fi
  subs=$(q "select count(*) from observations where tool like 'Subagent%';")
  [ "${subs:-0}" -gt 0 ] && ok "$subs SubagentStart/Stop observation(s) (host emits the events)" \
    || bad "no Subagent* observations: this Codex CLI version may not emit them (report on the PR)"
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

for c in ${1:-all}; do
  case "$c" in
    1|all) case1 ;;
    2|all) case2 ;;
    3|all) case3 ;;
    4|all) case4 ;;
  esac
done

echo
if [ "$FAILS" -gt 0 ]; then echo "RESULT: $FAILS FAIL"; exit 1; fi
echo "RESULT: PASS (or SKIP-only)"
