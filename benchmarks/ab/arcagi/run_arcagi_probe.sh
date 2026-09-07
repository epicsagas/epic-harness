#!/usr/bin/env bash
# benchmarks/ab/arcagi/run_arcagi_probe.sh
#
# ARC-AGI-3 floor probe (benchmarks/ab follow-up to SWE-bench main run).
# Question: does the base model make ANY level progress on one public game
# when driven headless by the same bare-arm driver as the A/B harness?
# If levels_completed == 0 on the bare arm, the ARC-AGI-3 A/B track is dead
# (floor effect, no dynamic range) — kill it cheaply here.
#
# Pipeline: boot arcagi/bridge.py (ONLINE mode, one scorecard) -> run
# `claudy <profile> [--bare] -p task.md` headless -> capture cost/turns JSON ->
# fetch final scorecard (self-closing if the agent forgot) -> probe result JSON.
#
# Usage:
#   ARC_API_KEY/ARCPRIZE_API_KEY must be set.
#   ./run_arcagi_probe.sh                 # bare arm, game ft09
# Env:
#   PROFILE     claudy profile (default zai = GLM-5.2[1m])
#   ARM         bare | epic   (default bare)
#   GAME_ID     game id prefix (default ls20; games whose ACTION6 is the only
#               lever, e.g. ft09, 500 on coordinate-less ACTION6 server-side)
#   ACTION_CAP  bridge-side step cap (default 60)
#   MAX_TURNS   claudy max-turns (default 80)
#   RUN_TIMEOUT wall seconds for the agent run (default 1800)
#   COST_CAP    USD cap per run (default 8.0)
set -uo pipefail
cd "$(dirname "$0")"

PROFILE="${PROFILE:-zai}"
ARM="${ARM:-bare}"
GAME_ID="${GAME_ID:-ls20}"
ACTION_CAP="${ACTION_CAP:-60}"
MAX_TURNS="${MAX_TURNS:-80}"
RUN_TIMEOUT="${RUN_TIMEOUT:-1800}"
COST_CAP="${COST_CAP:-8.0}"
PORT="${ARCAGI_PORT:-8765}"
BASE="http://127.0.0.1:${PORT}"
TS="$(date +%Y%m%dT%H%M%S)"
RESULT_JSON="results/probe-${TS}-${GAME_ID}-${ARM}.json"
BRIDGE_LOG="/tmp/arcagi-probe-bridge.log"
RUN_LOG="/tmp/arcagi-probe-agent.log"

[ -n "${ARC_API_KEY:-${ARCPRIZE_API_KEY:-}}" ] || { echo "ERR: ARC_API_KEY/ARCPRIZE_API_KEY not set" >&2; exit 2; }
[ -f "task-arcagi-probe/task.md" ] || { echo "ERR: task.md missing" >&2; exit 2; }
command -v jq >/dev/null || { echo "ERR: jq required" >&2; exit 2; }
PROMPT="$(cat task-arcagi-probe/task.md)"
export ARC_API_KEY="${ARC_API_KEY:-$ARCPRIZE_API_KEY}"
export GAME_ID ACTION_CAP MAX_NEW=3 RESULT_JSON PORT

# --- boot bridge ---
uv run --quiet --with arc-agi python bridge.py >"$BRIDGE_LOG" 2>&1 &
BRIDGE=$!
trap 'kill $BRIDGE 2>/dev/null' EXIT
for i in $(seq 1 60); do
  curl -sf "$BASE/healthcheck" >/dev/null 2>&1 && break
  kill -0 $BRIDGE 2>/dev/null || { echo "ERR: bridge died"; tail -5 "$BRIDGE_LOG"; exit 1; }
  sleep 0.5
done
grep -o "BRIDGE READY.*" "$BRIDGE_LOG" | head -1

# --- agent run (same driver conventions as run_smoke.sh) ---
EXTRA=""; [ "$ARM" = "bare" ] && EXTRA="--bare"
echo "[arcagi/$GAME_ID/$PROFILE/$ARM] agent starting (max_turns=$MAX_TURNS cost_cap=\$$COST_CAP)"
START=$(date +%s)
timeout "$RUN_TIMEOUT" claudy "$PROFILE" $EXTRA -p "$PROMPT" \
  --output-format json --max-turns "$MAX_TURNS" \
  --permission-mode bypassPermissions >"$RUN_LOG" 2>&1
RC=$?
WALL=$(( $(date +%s) - START ))

# --- finalize scorecard even if the agent forgot to /close ---
curl -s -X POST "$BASE/close" -o /dev/null 2>/dev/null
sleep 1

# --- collect ---
RESULT_LINE="$(grep '"type":"result"' "$RUN_LOG" | tail -1 || true)"
if [ -n "$RESULT_LINE" ]; then
  COST="$(echo "$RESULT_LINE" | jq -r '.total_cost_usd // 0')"
  TURNS="$(echo "$RESULT_LINE" | jq -r '.num_turns // 0')"
  DUR="$(echo "$RESULT_LINE" | jq -r '.duration_ms // 0')"
  INTOK="$(echo "$RESULT_LINE" | jq -r '.usage.input_tokens // 0')"
  IS_ERR="$(echo "$RESULT_LINE" | jq -r '.is_error // false')"
else
  COST=0; TURNS=0; DUR=0; INTOK=0; IS_ERR="no_result_line"
  tail -10 "$RUN_LOG" >&2
fi

[ -f "$RESULT_JSON" ] || { echo "ERR: no result JSON written" >&2; exit 1; }

jq -n --slurpfile score "$RESULT_JSON" \
  --arg arm "$ARM" --arg profile "$PROFILE" --arg game "$GAME_ID" \
  --argjson cost "$COST" --argjson turns "$TURNS" --argjson dur "$DUR" \
  --argjson intok "$INTOK" --argjson wall "$WALL" --argjson rc "$RC" \
  --argjson capped "$(awk -v c="$COST" -v cap="$COST_CAP" 'BEGIN{print (c+0>cap+0)?1:0}')" \
  '{arm:$arm, profile:$profile, game:$game, agent_rc:$rc, cost_usd:$cost,
    num_turns:$turns, duration_ms:$dur, input_tokens:$intok, wall_s:$wall,
    cost_capped:$capped, scorecard:$score[0]}'

echo "[arcagi] result: $RESULT_JSON"
