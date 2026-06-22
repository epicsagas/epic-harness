#!/usr/bin/env bash
# benchmarks/ab/run_smoke.sh
#
# Bare-vs-epic A/B smoke runner for multiple LLM provider profiles via claudy
# (epic-harness issue #94). Multi-model matrix.
#
# Runs one coding task across a LIST of claudy profiles, in two configurations
# per profile, with the epic-harness plugin as the only within-model variable:
#   bare : `claudy <profile> --bare ...`   (claude --bare skips plugins/hooks/LSP/mcp)
#   epic : `claudy <profile> ...`          (normal launch, epic plugin loaded)
#
# The bare/epic comparison is valid within a single model (plugin = only
# variable); cross-model rows are informational (how epic's overhead/score
# varies across model families), NOT the controlled variable of issue #94.
#
# Captures per (profile, arm): pass@1 (mechanical pytest grade), cost_usd,
# num_turns, duration_ms, input_tokens + identity (profile/model/family).
# Emits per-cell result JSONs + a combined comparison table.
#
# Usage:
#   ./run_smoke.sh <task_dir> [max_turns] [profile]   # single profile (legacy)
#   MODELS="zai native" ./run_smoke.sh <task_dir> [max_turns]   # multi-model
# Env:
#   MODELS      space- or comma-separated profile list (default zai). Ignored if $3 set.
#   COST_CAP    USD cap per cell (default 5.0)
#   RUN_TIMEOUT per-cell wall seconds (default 600)
#   DRY_RUN     1 = skip claudy invocation, synthesize zeroed results (validate loop, $0)
set -uo pipefail

TASK_DIR="${1:?usage: run_smoke.sh <task_dir> [max_turns] [profile]}"
MAX_TURNS="${2:-15}"
COST_CAP="${COST_CAP:-5.0}"
RUN_TIMEOUT="${RUN_TIMEOUT:-600}"
DRY_RUN="${DRY_RUN:-0}"

TASK_NAME="$(basename "$TASK_DIR")"
PROMPT_FILE="$TASK_DIR/task.md"
REPO_SRC="$TASK_DIR/repo"
[ -f "$PROMPT_FILE" ] || { echo "ERR: no task.md in $TASK_DIR" >&2; exit 2; }
[ -d "$REPO_SRC" ]    || { echo "ERR: no repo/ in $TASK_DIR" >&2; exit 2; }
PROMPT="$(cat "$PROMPT_FILE")"

# --- profile list resolution: $3 (single) > MODELS env > default zai ---
SINGLE_PROFILE=0
if [ "${3:-}" != "" ]; then
  MODELS_LIST=( "$3" )
  SINGLE_PROFILE=1
elif [ "${MODELS:-}" != "" ]; then
  # commas → spaces, then word-split into array
  IFS=$' \t,' read -r -a MODELS_LIST <<< "$MODELS"
else
  MODELS_LIST=( zai )
fi

command -v jq >/dev/null || { echo "ERR: jq required" >&2; exit 2; }

# --- which profiles are actually configured (claudy list) ---
# fail-soft: if `claudy list` is unavailable/unparseable, treat all as configured.
declare -A CONFIGURED
CLAUDY_LIST="$(claudy list 2>/dev/null || true)"
if [ -n "$CLAUDY_LIST" ]; then
  while IFS= read -r _line; do
    # lines: "  native             configured" / "  openai    not configured"
    _trimmed="${_line#"${_line%%[![:space:]]*}"}"   # ltrim
    [ -z "$_trimmed" ] && continue
    case "$_trimmed" in Available*|"Run:"*) continue ;; esac
    _p="${_trimmed%%[![:alnum:]_-]*}"                # first token (profile name)
    # must end with " configured" but NOT "not configured"
    [ -n "$_p" ] && [[ "$_trimmed" == *" configured" ]] \
      && [[ "$_trimmed" != *"not configured" ]] && CONFIGURED["$_p"]=1
  done <<< "$CLAUDY_LIST"
fi
CLAUDY_LIST_OK=1
[ ${#CONFIGURED[@]} -eq 0 ] && CLAUDY_LIST_OK=0   # list failed → assume all configured

# --- resolve model/family per profile via `claudy show` (cached) ---
declare -A RESOLVED_MODEL RESOLVED_FAMILY
resolve_profile() {  # $1 = profile
  local p="$1" info model family
  if [ -n "${RESOLVED_MODEL[$p]+x}" ]; then return; fi   # cached
  info="$(claudy show "$p" 2>/dev/null || true)"
  # claudy show prefixes lines with "INFO ". head -1 avoids sub-model rows.
  model="$(echo "$info"  | grep -E 'INFO[[:space:]]+Model:'  | head -1 | sed -E 's/.*Model:[[:space:]]*//')"
  family="$(echo "$info" | grep -E 'INFO[[:space:]]+Family:' | head -1 | sed -E 's/.*Family:[[:space:]]*//')"
  RESOLVED_MODEL[$p]="${model:-unknown}"
  RESOLVED_FAMILY[$p]="${family:-unknown}"
}

run_arm() {  # $1=arm  $2=extra_flag  $3=profile
  local arm="$1" extra_flag="$2" profile="$3"
  local model="${RESOLVED_MODEL[$profile]}" family="${RESOLVED_FAMILY[$profile]}"
  local work; work="$(mktemp -d -t "ab-${profile}-${arm}-${TASK_NAME}.XXXX")"
  cp -R "$REPO_SRC/." "$work/"
  chmod -R u+w "$work"
  echo "[$TASK_NAME/$profile/$arm] workdir=$work" >&2

  # sanity: tests must FAIL before the agent runs (validates the fixture)
  if (cd "$work" && python3 -m pytest -q >/dev/null 2>&1); then
    echo "[$TASK_NAME/$profile/$arm] WARN: tests pass pre-run — fixture is not failing" >&2
  fi

  if [ "$DRY_RUN" = "1" ]; then
    echo "[$TASK_NAME/$profile/$arm] dry-run: would run claudy $profile $extra_flag -p <prompt> --max-turns $MAX_TURNS" >&2
    local pass=0
    (cd "$work" && python3 -m pytest -q >/dev/null 2>&1) && pass=1
    jq -n \
      --arg arm "$arm" --arg task "$TASK_NAME" --arg profile "$profile" \
      --arg model "$model" --arg family "$family" \
      --argjson ok true --argjson dry true --argjson pass "$pass" --argjson is_error false \
      --argjson cost 0 --argjson turns 0 --argjson dur 0 --argjson intokens 0 \
      --argjson wall 0 --argjson rc 0 --argjson capped 0 --arg work "$work" \
      '{arm:$arm, task:$task, ok:true, dry_run:$dry, pass1:$pass, is_error:$is_error,
        cost_usd:$cost, num_turns:$turns, duration_ms:$dur, input_tokens:$intokens,
        wall_s:$wall, rc:$rc, cost_capped:$capped,
        profile:$profile, model:$model, family:$family, workdir:$work}'
    return
  fi

  local out="$work/run.stdout" err="$work/run.stderr"
  local start; start=$(date +%s)
  # Launch inside the per-arm workdir so the agent edits the isolated copy,
  # not the shared template. ($out/$err are absolute, stay valid after cd.)
  # shellcheck disable=SC2086  # $extra_flag intentional: "" must vanish, "--bare" must be one arg
  ( cd "$work" && timeout "$RUN_TIMEOUT" claudy "$profile" $extra_flag -p "$PROMPT" \
      --output-format json --max-turns "$MAX_TURNS" \
      --permission-mode bypassPermissions \
      >"$out" 2>"$err" )
  local rc=$?
  local end; end=$(date +%s); local wall=$((end-start))

  local result_line; result_line="$(grep '"type":"result"' "$out" | tail -1 || true)"
  if [ -z "$result_line" ]; then
    jq -n --arg arm "$arm" --arg task "$TASK_NAME" --arg profile "$profile" \
      --arg model "$model" --arg family "$family" \
      --argjson wall "$wall" --argjson rc "$rc" --arg work "$work" \
      '{arm:$arm, task:$task, ok:false, error:"no_result_line", rc:$rc, wall_s:$wall,
        profile:$profile, model:$model, family:$family, workdir:$work}'
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
    capped=1; echo "[$TASK_NAME/$profile/$arm] COST CAP EXCEEDED: \$${cost} > \$${COST_CAP}" >&2
  fi

  jq -n \
    --arg arm "$arm" --arg task "$TASK_NAME" --arg profile "$profile" \
    --arg model "$model" --arg family "$family" \
    --argjson ok true --argjson pass "$pass" --argjson is_error "$is_error" \
    --argjson cost "$cost" --argjson turns "$turns" \
    --argjson dur "$dur" --argjson intokens "$intokens" \
    --argjson wall "$wall" --argjson rc "$rc" --argjson capped "$capped" \
    --arg work "$work" \
    '{arm:$arm, task:$task, ok:true, pass1:$pass, is_error:$is_error,
      cost_usd:$cost, num_turns:$turns, duration_ms:$dur, input_tokens:$intokens,
      wall_s:$wall, rc:$rc, cost_capped:$capped,
      profile:$profile, model:$model, family:$family, workdir:$work}'
}

echo "=== A/B smoke: $TASK_NAME (models=[${MODELS_LIST[*]}] max_turns=$MAX_TURNS cost_cap=\$$COST_CAP dry=$DRY_RUN) ===" >&2

# --- main loop: profiles × arms. profile outer, arm inner (log adjacency, abort-safe). ---
RESULTS=()
RAN_PROFILES=()
for profile in "${MODELS_LIST[@]}"; do
  if [ "$CLAUDY_LIST_OK" = "1" ] && [ -z "${CONFIGURED[$profile]:-}" ]; then
    echo "[skip] $profile not configured (run \`claudy $profile\` then exit to set up)" >&2
    continue
  fi
  resolve_profile "$profile"
  RAN_PROFILES+=( "$profile" )
  for arm in bare epic; do
    if [ "$arm" = "bare" ]; then extra="--bare"; else extra=""; fi
    result="$(run_arm "$arm" "$extra" "$profile")"
    echo "$result" > "$TASK_DIR/result-${profile}-${arm}.json"
    RESULTS+=( "$result" )
  done
done

[ ${#RESULTS[@]} -gt 0 ] || { echo "ERR: no profiles ran (all skipped/unconfigured?)" >&2; exit 1; }

# --- legacy filename shims: only in single-profile mode ---
if [ "$SINGLE_PROFILE" = "1" ] && [ ${#RAN_PROFILES[@]} -eq 1 ]; then
  sp="${RAN_PROFILES[0]}"
  ( cd "$TASK_DIR" && ln -sf "result-${sp}-bare.json" "result-bare.json" \
                       && ln -sf "result-${sp}-epic.json" "result-epic.json" )
fi

# --- combined comparison table (all ran profiles × arms) ---
NMODELS=${#RAN_PROFILES[@]}
{
  echo "## $TASK_NAME — bare vs epic ($NMODELS model$([ "$NMODELS" = "1" ] || echo s))"
  echo ""
  echo "| family | model | profile | arm | pass@1 | cost(\$) | turns | dur(ms) | input_tok | wall(s) | capped |"
  echo "|--------|-------|---------|-----|--------|---------|-------|---------|-----------|---------|--------|"
  for r in "${RESULTS[@]}"; do
    echo "$r" | jq -r \
      '"| \(.family) | \(.model) | \(.profile) | \(.arm) | \(.pass1) | \(.cost_usd) | \(.num_turns) | \(.duration_ms) | \(.input_tokens) | \(.wall_s) | \(.cost_capped) |"'
  done
} | tee "$TASK_DIR/comparison.md"
