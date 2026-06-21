#!/usr/bin/env bash
# benchmarks/ab/run_swebench.sh
#
# Bare-vs-epic A/B runner over real SWE-bench Verified instances.
# Implements METHODOLOGY §6.2 BLOCKERs 1/2/4:
#   - stream-json transcript capture (BLOCKER 1)
#   - per-plugin enabledPlugins toggle so ONLY epic@epicsagas varies (BLOCKER 2)
#   - fresh mktemp workdir per (arm, instance, seed) (BLOCKER 4)
# Emits per-cell stream-json NDJSON, per-arm predictions.jsonl (for swebench grading),
# and a verdicts.jsonl row (resolved/p2p/f2p are joined AFTER swebench grading).
#
# Usage:
#   MANIFEST=manifest.jsonl ./run_swebench.sh                 # real run
#   MANIFEST=manifest.jsonl DRY_RUN=1 ./run_swebench.sh       # validate mechanics, no model call
# Env: ARMS (bare,epic)  SEEDS (1)  PROFILE (zai)  MAX_TURNS (50)  RUN_TIMEOUT (1800)
#      COST_CAP (5.0)  CLONE_CACHE  RUNS_DIR  PREDS_DIR  VERDICTS  KEEP_WORKDIR (0)
set -uo pipefail

MANIFEST="${MANIFEST:-manifest.jsonl}"
ARMS="${ARMS:-bare,epic}"
SEEDS="${SEEDS:-1}"
PROFILE="${PROFILE:-zai}"
MAX_TURNS="${MAX_TURNS:-50}"
RUN_TIMEOUT="${RUN_TIMEOUT:-1800}"
COST_CAP="${COST_CAP:-5.0}"
CLONE_CACHE="${CLONE_CACHE:-./clone-cache}"
RUNS_DIR="${RUNS_DIR:-./runs}"
PREDS_DIR="${PREDS_DIR:-./predictions}"
VERDICTS="${VERDICTS:-./verdicts.jsonl}"
DRY_RUN="${DRY_RUN:-0}"
KEEP_WORKDIR="${KEEP_WORKDIR:-0}"
HOST_ARCH="$(uname -m)"

[ -f "$MANIFEST" ] || { echo "ERR: manifest not found: $MANIFEST" >&2; exit 2; }
command -v jq >/dev/null || { echo "ERR: jq required" >&2; exit 2; }
mkdir -p "$CLONE_CACHE" "$RUNS_DIR" "$PREDS_DIR/bare" "$PREDS_DIR/epic"
: > "$VERDICTS"

# --- per-plugin toggle: only epic@epicsagas varies; LSP/mcp/hooks stay in BOTH arms ---
settings_for() {
  case "$1" in
    bare) printf '%s' '{"enabledPlugins":{"epic@epicsagas":false}}' ;;
    epic) printf '%s' '{"enabledPlugins":{"epic@epicsagas":true}}'  ;;
    *) echo "ERR: unknown arm $1" >&2; return 1 ;;
  esac
}

# --- standard SWE-bench agent instruction + the issue text ---
build_prompt() {
  local ps="$1"
  cat <<PROMPT
You are an expert software engineer working in the repository at the current working
directory, which is checked out at a specific commit. Resolve the real GitHub issue
described below by editing the repository code. Then run the relevant tests yourself to
verify. Rules:
- Modify only the source code needed to fix the issue. Do NOT modify or add tests.
- Keep changes minimal and idiomatic to the codebase.
- When done, stop; your working-tree diff is captured as your solution.

ISSUE:
$ps
PROMPT
}

ensure_clone() {  # $1 = repo (org/name)
  local repo="$1"
  local safe="${1//\//_}"
  local cache="$CLONE_CACHE/$safe.git"
  if [ ! -d "$cache" ]; then
    echo "[clone] mirroring https://github.com/$repo -> $cache" >&2
    git clone --mirror "https://github.com/$repo" "$cache" >&2 || return 1
  fi
}

emit_verdict() {  # many fields via globals; failure_mode via $1
  jq -n \
    --arg instance_id "$IID" --arg repo "$REPO" --arg band "$BAND" --arg host_arch "$HOST_ARCH" \
    --arg arm "$ARM" --arg seed "$SEED" --arg failure_mode "$1" \
    --argjson cost "${COST:-0}" --argjson turns "${TURNS:-0}" --argjson is_error "${IS_ERR:-false}" \
    --argjson rc "${RC:-0}" --argjson patch_bytes "${PATCH_BYTES:-0}" \
    --arg ndjson "$NDJSON" --arg workdir "$WORK" --arg profile "$PROFILE" \
    '{instance_id:$instance_id, repo:$repo, band:$band, host_arch:$host_arch,
      arm:$arm, seed:$seed, profile:$profile, failure_mode:$failure_mode,
      cost_usd:$cost, num_turns:$turns, is_error:$is_error, rc:$rc,
      patch_bytes:$patch_bytes, ndjson_path:$ndjson, workdir:$workdir}' >> "$VERDICTS"
}

run_cell() {  # $1=arm  $2=instance-json-line  $3=seed
  ARM="$1"; local ij="$2"; SEED="$3"
  IID=$(echo "$ij" | jq -r '.instance_id')
  REPO=$(echo "$ij" | jq -r '.repo')
  local base; base=$(echo "$ij" | jq -r '.base_commit')
  BAND=$(echo "$ij" | jq -r '.band')
  local ps; ps=$(echo "$ij" | jq -r '.problem_statement')
  local safe="${REPO//\//_}"
  NDJSON="$RUNS_DIR/$ARM/${IID}_${SEED}.ndjson"
  WORK="$(mktemp -d -t "swb-${ARM}-${IID}-${SEED}.XXXX")"
  echo "[$IID/$ARM/seed$SEED] workdir=$WORK" >&2

  if ! ensure_clone "$REPO"; then
    RC=1; emit_verdict "clone_failed"; return; fi
  # linked worktree at base_commit: shares the mirror's objects (no hardlink/copy),
  # cross-device safe, fresh isolated working tree per cell.
  if ! git -C "$CLONE_CACHE/$safe.git" worktree add --detach --quiet "$WORK" "$base"; then
    RC=1; emit_verdict "checkout_failed"; return; fi

  local settings; settings=$(settings_for "$ARM") || return
  local prompt; prompt=$(build_prompt "$ps")
  local start end
  # shellcheck disable=SC2086  # settings is a single --settings arg
  if [ "$DRY_RUN" = "1" ]; then
    echo "[dry-run] would run: claudy $PROFILE --settings $settings --output-format stream-json -p <prompt> --max-turns $MAX_TURNS --permission-mode bypassPermissions" >&2
    RC=0; COST=0; TURNS=0; IS_ERR=false; PATCH_BYTES=0; emit_verdict "dry_run"
  else
    start=$(date +%s)
    ( cd "$WORK" && timeout "$RUN_TIMEOUT" \
        claudy "$PROFILE" --settings "$settings" --output-format stream-json \
        -p "$prompt" --max-turns "$MAX_TURNS" --permission-mode bypassPermissions \
        > "$NDJSON" 2>/dev/null )
    RC=$?
    end=$(date +%s)
    local res; res=$(grep '"type":"result"' "$NDJSON" | tail -1 || true)
    COST=$(echo "$res" | jq -r '.total_cost_usd // 0' 2>/dev/null || echo 0)
    TURNS=$(echo "$res" | jq -r '.num_turns // 0' 2>/dev/null || echo 0)
    IS_ERR=$(echo "$res" | jq -r '.is_error // false' 2>/dev/null || echo false)
    local patch; patch=$(git -C "$WORK" --no-pager diff 2>/dev/null || true)
    PATCH_BYTES=${#patch}
    # prediction for swebench grading
    jq -n --arg iid "$IID" --arg arm "$ARM" --arg patch "$patch" \
      '{instance_id:$iid, model_name_or_path:$arm, model_patch:$patch}' \
      >> "$PREDS_DIR/$ARM/predictions.jsonl"
    local fmode="ok"
    [ "$RC" -ne 0 ] && fmode="nonzero_rc"
    [ "$PATCH_BYTES" -eq 0 ] && fmode="no_patch"
    echo "[$IID/$ARM/seed$SEED] rc=$RC turns=$TURNS cost=\$${COST} patch=${PATCH_BYTES}B (${end}-${start}s) mode=$fmode" >&2
    emit_verdict "$fmode"
  fi

  if [ "$KEEP_WORKDIR" != "1" ]; then
    git -C "$CLONE_CACHE/$safe.git" worktree remove --force "$WORK" 2>/dev/null || true
    rm -rf "$WORK"
  fi
}

echo "=== run_swebench: manifest=$MANIFEST arms=$ARMS seeds=$SEEDS profile=$PROFILE max_turns=$MAX_TURNS arch=$HOST_ARCH dry=$DRY_RUN ===" >&2
while IFS= read -r line; do
  [ -z "$line" ] && continue
  for arm in ${ARMS//,/ }; do
    for seed in ${SEEDS//,/ }; do
      run_cell "$arm" "$line" "$seed"
    done
  done
done < "$MANIFEST"
echo "=== done. verdicts=$VERDICTS predictions=$PREDS_DIR/<arm>/predictions.jsonl transcripts=$RUNS_DIR ===" >&2
