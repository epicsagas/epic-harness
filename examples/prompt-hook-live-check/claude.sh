#!/usr/bin/env bash
# Live-host nonce probe for the prompt hook (ADR 0008, issue #276). Not run in CI.
#
# Seeds one synthetic memory with a random nonce in a disposable HARNESS_DIR,
# runs `claude -p` with this plugin loaded, and checks the hook itself, not the
# reply (SessionStart resume also injects matching memories, so the reply can
# quote the nonce with the hook off). Enabled: the claude debug file shows the
# prompt hook delivered additionalContext with the nonce, and the dispatch log
# has a prompt_hook line with recalled >= 1. Disabled: the hook finished with
# status 0, delivered no additionalContext, and added no prompt_hook dispatch
# line. Prints PASS or FAIL, never a memory body. Everything lives under one
# temp dir that is removed on exit; the real ~/.harness is never used.
#
# usage: EPIC_BIN=/path/to/epic-harness examples/prompt-hook-live-check/claude.sh
# Needs: claude (able to authenticate with a disposable HOME, for example via an
# API key or OAuth token in the environment), node, git.
#
# KEEP_HOME=1 keeps the real HOME so claude can authenticate with an OAuth login
# (macOS keychain). User settings and user plugins are not loaded
# (--setting-sources project), and a temp bin dir with an epic symlink goes first
# on PATH so the SessionStart bootstrap finds an up-to-date epic and installs
# nothing. It refuses to run if EPIC_BIN is older than the plugin or if
# ~/.config/epic-harness/telemetry-consent is missing (epic would write it).
# `A && echo PASS || { FAIL }` is intended: echo cannot fail.
# shellcheck disable=SC2015
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EPIC_BIN="${EPIC_BIN:-$(command -v epic-harness || true)}"
[ -n "$EPIC_BIN" ] && [ -x "$EPIC_BIN" ] || { echo "FAIL: set EPIC_BIN to an epic-harness binary"; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/harness" "$WORK/repo"
if [ "${KEEP_HOME:-}" = 1 ]; then
  [ -e "$HOME/.config/epic-harness/telemetry-consent" ] \
    || { echo "FAIL: KEEP_HOME needs an existing ~/.config/epic-harness/telemetry-consent"; exit 1; }
  # install.cjs main() returns without writing only when the epic it finds is
  # not older than the plugin version; otherwise it installs into ~/.local/bin.
  PLUGIN_VERSION="$(node -p 'require(process.argv[1]).version' "$REPO/.claude-plugin/plugin.json")"
  EPIC_VERSION="$("$EPIC_BIN" --version | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' | head -1)"
  [ "$(printf '%s\n%s\n' "$PLUGIN_VERSION" "$EPIC_VERSION" | sort -V | head -1)" = "$PLUGIN_VERSION" ] \
    || { echo "FAIL: EPIC_BIN ($EPIC_VERSION) is older than the plugin ($PLUGIN_VERSION)"; exit 1; }
  # run.mjs runs ~/.local/bin/epic when it exists, whatever PATH says.
  [ -e "$HOME/.local/bin/epic" ] && [ ! "$HOME/.local/bin/epic" -ef "$EPIC_BIN" ] \
    && echo "note: hooks will run ~/.local/bin/epic, not EPIC_BIN"
  mkdir -p "$WORK/bin"
  ln -s "$EPIC_BIN" "$WORK/bin/epic"
  export PATH="$WORK/bin:$PATH"
else
  mkdir -p "$WORK/home/.config/epic-harness" "$WORK/home/.local/bin"
  echo off > "$WORK/home/.config/epic-harness/telemetry-consent"
  # run.mjs prefers ~/.local/bin/epic, and the SessionStart bootstrap leaves a
  # copy at the plugin version alone, so nothing is downloaded.
  ln -s "$EPIC_BIN" "$WORK/home/.local/bin/epic"
  export HOME="$WORK/home"
  export PATH="$WORK/home/.local/bin:$PATH"
fi

NONCE="NONCE-$(od -An -N6 -tx1 /dev/urandom | tr -d ' \n')"
KEYWORD="kestrelprobe"

export HARNESS_DIR="$WORK/harness"
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
    --plugin-dir "$REPO" --setting-sources project --debug-file "$DEBUG_FILE" > "$REPLY_FILE"
}

# claude debug lines: "UserPromptSubmit [node ".../run.mjs" prompt] ... finished with
# status 0" and, when the hook printed context, the line before
# "UserPromptSubmit (node ".../run.mjs" prompt) provided additionalContext (N chars)"
# carries that context.
prompt_status0() { grep -Eq 'UserPromptSubmit \[.*run\.mjs" prompt\].*finished with status 0' "$DEBUG_FILE"; }
PROVIDED='run\.mjs" prompt\) provided additionalContext'
dispatch_lines() { cat "$HARNESS_DIR/projects/$SLUG/dispatch/"dispatch_*.jsonl 2>/dev/null | grep '"source":"prompt_hook"'; }

FAILS=0
ask enabled
prompt_status0 \
  && echo "  PASS  enabled: prompt hook finished with status 0" \
  || { echo "  FAIL  enabled: no prompt hook status 0 in the claude debug file"; FAILS=$((FAILS+1)); }
grep -B1 -E "$PROVIDED" "$DEBUG_FILE" | grep -q "$NONCE" \
  && echo "  PASS  enabled: prompt hook provided additionalContext with the nonce" \
  || { echo "  FAIL  enabled: no prompt hook additionalContext with the nonce"; FAILS=$((FAILS+1)); }
dispatch_lines | grep -Eq '"recalled":[1-9]' \
  && echo "  PASS  enabled: dispatch log has a prompt_hook line with recalled >= 1" \
  || { echo "  FAIL  enabled: no prompt_hook dispatch line with recalled >= 1"; FAILS=$((FAILS+1)); }
grep -q "$NONCE" "$REPLY_FILE" \
  && echo "  INFO  enabled: reply quotes the nonce" \
  || echo "  INFO  enabled: reply does not quote the nonce"

BEFORE="$(dispatch_lines | wc -l)"
ask disabled
prompt_status0 \
  && echo "  PASS  disabled: prompt hook finished with status 0" \
  || { echo "  FAIL  disabled: no prompt hook status 0 in the claude debug file"; FAILS=$((FAILS+1)); }
grep -Eq "$PROVIDED" "$DEBUG_FILE" \
  && { echo "  FAIL  disabled: prompt hook provided additionalContext"; FAILS=$((FAILS+1)); } \
  || echo "  PASS  disabled: prompt hook provided no additionalContext"
[ "$(dispatch_lines | wc -l)" = "$BEFORE" ] \
  && echo "  PASS  disabled: no new prompt_hook dispatch line" \
  || { echo "  FAIL  disabled: new prompt_hook dispatch line"; FAILS=$((FAILS+1)); }

[ "$FAILS" -eq 0 ] && echo PASS || echo FAIL
[ "$FAILS" -eq 0 ]
