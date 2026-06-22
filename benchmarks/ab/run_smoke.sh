#!/usr/bin/env bash
# benchmarks/ab/run_smoke.sh
#
# Bare-vs-epic A/B smoke runner for GLM via claudy (epic-harness issue #94).
#
# Runs one coding task in two configurations against the SAME model+router
# (claudy <profile> = GLM), with the epic-harness plugin as the only variable:
#   bare : `claudy <profile> --bare ...`   (claude --bare skips plugins/hooks/LSP/mcp)
#   epic : `claudy <profile> ...`          (normal launch, epic plugin loaded)
#
# Captures per arm: pass@1 (mechanical pytest grade), cost_usd, num_turns,
# duration_ms, input_tokens. Emits a comparison table + result JSONs.
#
# Usage:  ./run_smoke.sh <task_dir> [max_turns] [profile]
# Env:    COST_CAP (default 5.0)  RUN_TIMEOUT (default 600s)
set -uo pipefail

TASK_DIR="${1:?usage: run_smoke.sh <task_dir> [max_turns] [profile]}"
MAX_TURNS="${2:-15}"
PROFILE="${3:-zai}"
COST_CAP="${COST_CAP:-5.0}"
RUN_TIMEOUT="${RUN_TIMEOUT:-600}"

TASK_NAME="$(basename "$TASK_DIR")"
PROMPT_FILE="$TASK_DIR/task.md"
REPO_SRC="$TASK_DIR/repo"
[ -f "$PROMPT_FILE" ] || { echo "ERR: no task.md in $TASK_DIR" >&2; exit 2; }
[ -d "$REPO_SRC" ]    || { echo "ERR: no repo/ in $TASK_DIR" >&2; exit 2; }
PROMPT="$(cat "$PROMPT_FILE")"

run_arm() {
  local arm="$1" extra_flag="$2"
  local work; work="$(mktemp -d -t "ab-${arm}-${TASK_NAME}.XXXX")"
  cp -R "$REPO_SRC/." "$work/"
  chmod -R u+w "$work"
  echo "[$TASK_NAME/$arm] workdir=$work" >&2

  # sanity: tests must FAIL before the agent runs (validates the fixture)
  if (cd "$work" && python3 -m pytest -q >/dev/null 2>&1); then
    echo "[$TASK_NAME/$arm] WARN: tests pass pre-run — fixture is not failing" >&2
  fi

  local out="$work/run.stdout" err="$work/run.stderr"
  local start; start=$(date +%s)
  # Launch inside the per-arm workdir so the agent edits the isolated copy,
  # not the shared template. ($out/$err are absolute, stay valid after cd.)
  # shellcheck disable=SC2086  # $extra_flag intentional: "" must vanish, "--bare" must be one arg
  ( cd "$work" && timeout "$RUN_TIMEOUT" claudy "$PROFILE" $extra_flag -p "$PROMPT" \
      --output-format json --max-turns "$MAX_TURNS" \
      --permission-mode bypassPermissions \
      >"$out" 2>"$err" )
  local rc=$?
  local end; end=$(date +%s); local wall=$((end-start))

  local result_line; result_line="$(grep '"type":"result"' "$out" | tail -1 || true)"
  if [ -z "$result_line" ]; then
    jq -n --arg arm "$arm" --arg task "$TASK_NAME" --argjson wall "$wall" --argjson rc "$rc" --arg work "$work" \
      '{arm:$arm, task:$task, ok:false, error:"no_result_line", rc:$rc, wall_s:$wall, workdir:$work}'
    cat "$err" | tail -20 | sed 's/^/    stderr: /' >&2
    return
  fi

  local cost turns dur intokens is_error
  cost="$(echo "$result_line" | jq -r '.total_cost_usd // 0')"
  turns="$(echo "$result_line" | jq -r '.num_turns // 0')"
  dur="$(echo "$result_line" | jq -r '.duration_ms // 0')"
  intokens="$(echo "$result_line" | jq -r '.usage.input_tokens // 0')"
  is_error="$(echo "$result_line" | jq -r '.is_error // false')"

  # grade: do ALL tests pass now? (independent mechanical check)
  local pass=0
  if (cd "$work" && python3 -m pytest -q >/dev/null 2>&1); then pass=1; fi

  # cost guard
  local capped=0
  if awk -v c="$cost" -v cap="$COST_CAP" 'BEGIN{exit !(c+0 > cap+0)}'; then
    capped=1; echo "[$TASK_NAME/$arm] COST CAP EXCEEDED: \$${cost} > \$${COST_CAP}" >&2
  fi

  jq -n \
    --arg arm "$arm" --arg task "$TASK_NAME" \
    --argjson ok true --argjson pass "$pass" --argjson is_error "$is_error" \
    --argjson cost "$cost" --argjson turns "$turns" \
    --argjson dur "$dur" --argjson intokens "$intokens" \
    --argjson wall "$wall" --argjson rc "$rc" --argjson capped "$capped" \
    --arg work "$work" \
    '{arm:$arm, task:$task, ok:true, pass1:$pass, is_error:$is_error,
      cost_usd:$cost, num_turns:$turns, duration_ms:$dur, input_tokens:$intokens,
      wall_s:$wall, rc:$rc, cost_capped:$capped, workdir:$work}'
}

echo "=== A/B smoke: $TASK_NAME (profile=$PROFILE max_turns=$MAX_TURNS cost_cap=\$$COST_CAP) ===" >&2
BARE="$(run_arm bare  "--bare")"
EPIC="$(run_arm epic  "")"
echo "$BARE" > "$TASK_DIR/result-bare.json"
echo "$EPIC" > "$TASK_DIR/result-epic.json"

{
  echo "## $TASK_NAME — bare vs epic"
  echo ""
  echo "| arm | pass@1 | cost(\$) | turns | dur(ms) | input_tok | wall(s) | capped |"
  echo "|-----|--------|---------|-------|---------|-----------|---------|--------|"
  echo "$BARE" | jq -r '"| bare | \(.pass1) | \(.cost_usd) | \(.num_turns) | \(.duration_ms) | \(.input_tokens) | \(.wall_s) | \(.cost_capped) |"'
  echo "$EPIC" | jq -r '"| epic | \(.pass1) | \(.cost_usd) | \(.num_turns) | \(.duration_ms) | \(.input_tokens) | \(.wall_s) | \(.cost_capped) |"'
} | tee "$TASK_DIR/comparison.md"
