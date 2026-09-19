#!/usr/bin/env bash
# benchmarks/ab/arcagi/selftest.sh
#
# Pre-flight for the ARC-AGI-3 probe BEFORE spending agent tokens: boot
# bridge.py, start a run, send ONE action, read the score, close the
# (throwaway) scorecard. Validates: key accepted, game found, frame render,
# step plumbing, result-file write. ~2 non-scoring steps of budget.
set -uo pipefail
cd "$(dirname "$0")"

PORT="${ARCAGI_PORT:-8765}"
BASE="http://127.0.0.1:${PORT}"
LOG=/tmp/arcagi-selftest-bridge.log
GAME_ID="${GAME_ID:-ls20}"

command -v jq >/dev/null || { echo "ERR: jq required" >&2; exit 2; }
[ -n "${ARC_API_KEY:-${ARCPRIZE_API_KEY:-}}" ] || { echo "ERR: key not set" >&2; exit 2; }
export ARC_API_KEY="${ARC_API_KEY:-$ARCPRIZE_API_KEY}"
export GAME_ID ACTION_CAP=5 MAX_NEW=1 RESULT_JSON=/tmp/arcagi-selftest-result.json PORT

uv run --quiet --with arc-agi python bridge.py >"$LOG" 2>&1 &
BRIDGE=$!
trap 'kill $BRIDGE 2>/dev/null' EXIT
for i in $(seq 1 60); do
  curl -sf "$BASE/healthcheck" >/dev/null 2>&1 && break
  kill -0 $BRIDGE 2>/dev/null || { echo "ERR: bridge died"; tail -5 "$LOG"; exit 1; }
  sleep 0.5
done

NEW=$(curl -s -X POST "$BASE/new" -H 'Content-Type: application/json' -d '{}')
GUID=$(echo "$NEW" | jq -r '.guid // empty')
echo "$NEW" | jq '{guid, state, level, win_levels, available_actions, layers,
  frame_h: (.frame[0]|length), frame_w: (.frame[0][0]|length)}' 2>/dev/null \
  || { echo "ERR: /new failed"; tail -5 "$LOG"; exit 1; }
[ -n "$GUID" ] || { echo "ERR: no guid"; exit 1; }

AVAIL=$(echo "$NEW" | jq -r '.available_actions[0]')
echo "== ACTION$AVAIL =="
curl -s -X POST "$BASE/act/$AVAIL" -H 'Content-Type: application/json' -d "{\"guid\":\"$GUID\"}" \
  | jq '{action_used, state, level, frame_h: (.frame[0]|length)}' 2>/dev/null

echo "== score =="
curl -s "$BASE/score" | jq '{score, probe_steps, runs: [.runs[] | {levels_completed, actions, state}]}'

curl -s -X POST "$BASE/close" -H 'Content-Type: application/json' -d '{}' | jq '{closed, written}'
echo "result file:"; cat /tmp/arcagi-selftest-result.json | jq -c '{game, steps_used, lvls: [.final.runs[].levels_completed]}'
echo "== SELFTEST OK =="
