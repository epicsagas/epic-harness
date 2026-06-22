#!/usr/bin/env bash
# record_tui_result.sh — Write a Mode-B TUI result JSON after fixing a smoke task in-session.
#
# Usage:
#   record_tui_result.sh <task_dir> <bench_start_iso> <bench_end_iso> <pass1> <arm> [profile]
#
#   task_dir         Path to the task dir (e.g. benchmarks/ab/tasks/task1-palindrome)
#   bench_start_iso  ISO8601 timestamp recorded BEFORE performing the task
#   bench_end_iso    ISO8601 timestamp recorded AFTER grading with pytest
#   pass1            1 (all tests passed) or 0 (at least one failed)
#   arm              epic | bare
#   profile          claudy profile name (default: native for epic, bare for bare)
#
# Reads the most-recent session JSONL from the Claude Code project directory,
# aggregates assistant turns between start and end, then writes:
#   <task_dir>/result-<model>-<arm>.json
#
# Depends on: capture_tui_tokens.py (sibling script), jq, python3

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ $# -lt 5 ]; then
  echo "Usage: $0 <task_dir> <bench_start_iso> <bench_end_iso> <pass1> <arm> [profile]" >&2
  exit 1
fi

TASK_DIR="$1"
BENCH_START="$2"
BENCH_END="$3"
PASS1="$4"
ARM="$5"
PROFILE="${6:-}"

if [ -z "$PROFILE" ]; then
  [ "$ARM" = "epic" ] && PROFILE="native" || PROFILE="bare"
fi

TASK_NAME="$(basename "$TASK_DIR")"

# Locate the most-recent session JSONL for the current project
PROJECT_SLUG=$(python3 -c "
import os
cwd = os.getcwd()
# Claude Code slug: every '/' → '-' (leading '/' becomes leading '-')
slug = cwd.replace('/', '-')
print(slug)
")
JSONL_DIR="$HOME/.claude/projects/${PROJECT_SLUG}"
JSONL=$(ls -t "$JSONL_DIR"/*.jsonl 2>/dev/null | head -1)
if [ -z "$JSONL" ]; then
  echo "ERROR: no session JSONL found in $JSONL_DIR" >&2
  exit 1
fi

# Aggregate tokens
TOKENS_JSON=$("$SCRIPT_DIR/capture_tui_tokens.py" \
  --jsonl "$JSONL" --start "$BENCH_START" --end "$BENCH_END")

MODEL=$(echo "$TOKENS_JSON" | jq -r '.model')
INPUT_TOKENS=$(echo "$TOKENS_JSON" | jq '.input_tokens')
OUTPUT_TOKENS=$(echo "$TOKENS_JSON" | jq '.output_tokens')
NUM_TURNS=$(echo "$TOKENS_JSON" | jq '.num_turns')
DURATION_MS=$(echo "$TOKENS_JSON" | jq '.duration_ms')

# Derive family from model name
if echo "$MODEL" | grep -qi 'claude'; then
  FAMILY="claude"
elif echo "$MODEL" | grep -qi 'glm'; then
  FAMILY="glm"
elif echo "$MODEL" | grep -qi 'gpt\|openai'; then
  FAMILY="openai"
else
  FAMILY="unknown"
fi

OUT_FILE="$TASK_DIR/result-${MODEL}-${ARM}.json"

jq -n \
  --arg arm "$ARM" \
  --arg task "$TASK_NAME" \
  --argjson pass1 "$PASS1" \
  --arg model "$MODEL" \
  --arg family "$FAMILY" \
  --arg profile "$PROFILE" \
  --argjson input_tokens "$INPUT_TOKENS" \
  --argjson output_tokens "$OUTPUT_TOKENS" \
  --argjson num_turns "$NUM_TURNS" \
  --argjson duration_ms "$DURATION_MS" \
  '{arm:$arm, task:$task, ok:($pass1==1), pass1:$pass1,
    is_error:false, model:$model, family:$family, profile:$profile,
    input_tokens:$input_tokens, output_tokens:$output_tokens,
    num_turns:$num_turns, duration_ms:$duration_ms,
    wall_s:($duration_ms/1000|floor), cost_usd:null, cost_capped:0,
    workdir:null, tui_mode:true}' \
  | tee "$OUT_FILE"

echo "" >&2
echo "[bench] result written: $OUT_FILE" >&2
