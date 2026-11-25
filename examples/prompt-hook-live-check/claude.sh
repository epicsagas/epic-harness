#!/usr/bin/env bash
# Live-host nonce probe for the prompt hook (ADR 0008, issue #276). Not run in CI.
#
# Seeds one synthetic memory with a random nonce in a disposable HARNESS_DIR,
# runs `claude -p` with this plugin loaded, and checks that the reply quotes the
# nonce while the hook enabled, and does not when it is disabled. Prints PASS or
# FAIL, never a memory body. Everything lives under one temp dir that is removed
# on exit; the real ~/.harness and ~/.claude are never used.
#
# usage: EPIC_BIN=/path/to/epic-harness examples/prompt-hook-live-check/claude.sh
# Needs: claude (able to authenticate with a disposable HOME, for example via an
# API key or OAuth token in the environment), node, git.
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EPIC_BIN="${EPIC_BIN:-$(command -v epic-harness || true)}"
[ -n "$EPIC_BIN" ] && [ -x "$EPIC_BIN" ] || { echo "FAIL: set EPIC_BIN to an epic-harness binary"; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/home/.config/epic-harness" "$WORK/home/.local/bin" "$WORK/harness" "$WORK/repo"
echo off > "$WORK/home/.config/epic-harness/telemetry-consent"
# run.mjs prefers ~/.local/bin/epic, and the SessionStart bootstrap leaves a
# copy at the plugin version alone, so nothing is downloaded.
ln -s "$EPIC_BIN" "$WORK/home/.local/bin/epic"

NONCE="NONCE-$(od -An -N6 -tx1 /dev/urandom | tr -d ' \n')"
KEYWORD="kestrelprobe"

export HOME="$WORK/home"
export HARNESS_DIR="$WORK/harness"
export PATH="$WORK/home/.local/bin:$PATH"
export GIT_CEILING_DIRECTORIES="$WORK"
unset HARNESS_ROOT HARNESS_MEM_URL HARNESS_DB_URL EPIC_HOOK_PROFILE

cd "$WORK/repo" && git init -q . || { echo "FAIL: git init"; exit 1; }
SLUG="$(epic slug)"
mkdir -p "$HARNESS_DIR/projects/$SLUG"
epic mem add --title "Refresh policy $NONCE" --type decision --project "$SLUG" \
  --body "The $KEYWORD service rotates refresh tokens every 15 minutes." > /dev/null \
  || { echo "FAIL: could not seed memory"; exit 1; }

ask() { # $1 = enabled|disabled; sets REPLY_FILE and DEBUG_FILE
  printf '[dashboard]\nport = 0\nauto_open = false\n[prompt]\nenabled = %s\n' \
    "$([ "$1" = enabled ] && echo true || echo false)" > "$HARNESS_DIR/config.toml"
  REPLY_FILE="$WORK/reply-$1.txt"
  DEBUG_FILE="$WORK/debug-$1.txt"
  claude -p "What does our project memory say about $KEYWORD? Quote any identifier you see." \
    --plugin-dir "$REPO" --debug > "$REPLY_FILE" 2> "$DEBUG_FILE"
}

FAILS=0
ask enabled
grep -q "$NONCE" "$REPLY_FILE" && echo "  PASS  enabled: reply quotes the nonce" \
  || { echo "  FAIL  enabled: reply does not quote the nonce"; FAILS=$((FAILS+1)); }
grep -E "UserPromptSubmit" "$DEBUG_FILE" | grep -Eqi "status 0|exit(ed)? (code )?0" \
  && echo "  PASS  enabled: UserPromptSubmit hook finished with status 0" \
  || { echo "  FAIL  enabled: no UserPromptSubmit status 0 in claude --debug output"; FAILS=$((FAILS+1)); }

ask disabled
grep -q "$NONCE" "$REPLY_FILE" \
  && { echo "  FAIL  disabled: reply quotes the nonce"; FAILS=$((FAILS+1)); } \
  || echo "  PASS  disabled: reply does not quote the nonce"

[ "$FAILS" -eq 0 ] && echo PASS || echo FAIL
[ "$FAILS" -eq 0 ]
