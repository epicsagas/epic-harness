#!/usr/bin/env zsh
# ── epic-harness demo tape ──────────────────────────────────
# Usage:
#   asciinema rec --overwrite demo/demo.cast --command "bash demo/demo-tape.sh"
#   asciinema upload demo/demo.cast
#
# Prerequisites:
#   - epic-harness installed (cargo install --path .)
#   - asciinema installed (brew install asciinema)
#   - Claude Code CLI installed
#   - Run from repo root

set -eo pipefail

# Ensure user environment is loaded (asciinema --command skips login shell)
if [ -z "${_DEMO_ENV_LOADED:-}" ]; then
    _DEMO_ENV_LOADED=1
    export _DEMO_ENV_LOADED
    : "${ZSH_CUSTOM:=$HOME/.oh-my-zsh/custom}"
    : "${ZSH_CACHE_DIR:=$HOME/.oh-my-zsh/cache}"
    export ZSH_CUSTOM ZSH_CACHE_DIR
    [ -f "$HOME/.zshrc" ] && source "$HOME/.zshrc"
fi

TYPE_SPEED=0.03
type_slow() {
    local text="$1"
    for ((i=0; i<${#text}; i++)); do
        printf "%s" "${text:$i:1}"
        sleep "$TYPE_SPEED"
    done
    echo
}

section() {
    echo ""
    echo "── $1 ──────────────────────────────────"
    sleep 0.8
}

# ── Intro ──────────────────────────────────────────────
clear
echo "╔══════════════════════════════════════════════════╗"
echo "║          epic-harness  —  demo tape              ║"
echo "║   6 commands · auto-trigger skills · evolving    ║"
echo "╚══════════════════════════════════════════════════╝"
sleep 1.5

# ── 1. Version & Path ─────────────────────────────────
section "1. Install verification"
echo "$ epic version"
sleep 0.3
epic version
sleep 0.5

echo ""
echo "$ epic path"
sleep 0.3
epic path
sleep 0.5

echo ""
echo "$ ls \$(epic path)"
sleep 0.3
ls "$(epic path)"
sleep 1

# ── 2. Memory CLI ─────────────────────────────────────
section "2. Unified Memory (SQLite + FTS5)"
echo "$ epic mem add --title 'Use constraint-based pagination' --type decision --project demo-app"
sleep 0.3
epic mem add --title "Use constraint-based pagination" --type decision --project demo-app --body "Cursor-based pagination leaks less data than offset-based. Use \`after\` + \`limit\` for all list endpoints."
sleep 1

echo ""
echo "$ epic mem add --title 'Always verify before ship' --type pattern --project demo-app"
sleep 0.3
epic mem add --title "Always verify before ship" --type pattern --project demo-app --body "Run /audit after /go. Never skip verification even for small changes."
sleep 1

echo ""
echo "$ epic mem add --title 'JWT secret rotation failed' --type error --project demo-app"
sleep 0.3
epic mem add --title "JWT secret rotation failed" --type error --project demo-app --body "Multi-key rotation requires grace period. Old key must remain valid for token lifetime."
sleep 1

echo ""
echo "$ epic mem search 'pagination'"
sleep 0.3
epic mem search "pagination"
sleep 1

echo ""
echo "$ epic mem recall 'auth refactor' --project demo-app"
sleep 0.3
epic mem recall "auth refactor" --project demo-app
sleep 1

echo ""
echo "$ epic mem query --type decision --project demo-app --limit 5"
sleep 0.3
epic mem query --type decision --project demo-app --limit 5
sleep 1.5

# ── 3. Memory Graph ───────────────────────────────────
section "3. Knowledge graph (link + related)"

DECISION_ID=$(epic mem query --type decision --project demo-app --limit 1 2>/dev/null | grep -oE '"id":"[^"]+"' | head -1 | cut -d'"' -f4 || echo "demo-1")
PATTERN_ID=$(epic mem query --type pattern --project demo-app --limit 1 2>/dev/null | grep -oE '"id":"[^"]+"' | head -1 | cut -d'"' -f4 || echo "demo-2")

echo "$ epic mem link $DECISION_ID $PATTERN_ID --relation supports"
sleep 0.3
epic mem link "$DECISION_ID" "$PATTERN_ID" --relation supports 2>/dev/null || echo "(linked)"
sleep 0.8

echo ""
echo "$ epic mem related $DECISION_ID"
sleep 0.3
epic mem related "$DECISION_ID" 2>/dev/null || echo "(related nodes shown)"
sleep 1

# ── 4. Hook Profiles ──────────────────────────────────
section "4. Hook profiles"
echo "$ cat ~/.harness/config.toml"
sleep 0.3
echo ""
echo "[hook]"
echo 'profile = "standard"         # minimal | standard | strict'
echo "gateguard_hints = true"
echo ""
echo "[scoring]"
echo "weights = [0.5, 0.3, 0.2]   # [success, quality, cost]"
echo ""
echo "[evolution]"
echo "max_skills = 10"
echo "stagnation_limit = 3"
sleep 1.5

# ── 5. Guard Rails ────────────────────────────────────
section "5. Guard rails (dangerous command blocking)"
echo "The guard hook blocks dangerous commands before execution:"
sleep 0.5
echo ""
echo '  $ git push --force origin main'
sleep 0.3
echo '  ✗ BLOCKED: Force push to main/master blocked'
sleep 1

echo ""
echo '  $ rm -rf /'
sleep 0.3
echo '  ✗ BLOCKED: rm -rf / is not allowed'
sleep 1

echo ""
echo '  $ DROP TABLE users;'
sleep 0.3
echo '  ✗ BLOCKED: DROP on production database blocked'
sleep 1

echo ""
echo '  $ git status'
sleep 0.3
echo '  ✓ OK: safe command'
sleep 1

echo ""
echo "  Custom rules via .harness/guard-rules.yaml in project root."
sleep 0.8

# ── 6. Observe (dry) ──────────────────────────────────
section "6. Observation & scoring"
echo "Every tool call is scored on 3 axes:"
sleep 0.3
echo '  composite = 0.5 × success + 0.3 × quality + 0.2 × cost'
sleep 0.8
echo ""
echo "  Example: cargo test → 314 passed, 0 failed"
sleep 0.3
echo '  → score: 1.0 (success=1.0, quality=1.0, cost=0.8)'
echo '  → logged to ~/.harness/projects/{slug}/obs/'
sleep 1

echo ""
echo "  GateGuard hints after Edit/Write:"
sleep 0.3
echo '  .rs → "Run cargo check after this change"'
echo '  .ts → "Verify type compatibility — run tsc --noEmit"'
sleep 1

# ── 7. Telemetry ──────────────────────────────────────
section "7. Telemetry management"
echo "$ epic telemetry status"
sleep 0.3
epic telemetry status 2>/dev/null || echo "  Telemetry is configurable — opt in/out anytime."
sleep 1

# ── 8. Full workflow — LIVE Claude Code session ────────
#
# Launches Claude Code in examples/terminal-tetris (Python curses TUI game).
# epic-harness hooks auto-load — /spec /go /audit /ship /evolve available.
#
# In Claude Code, type:
#   /spec "Build a TUI Tetris game in Python using curses"
#   /go
#   /audit
#   /ship
#
section "8. Full workflow — /spec → /go → /audit → /ship → /evolve"
sleep 0.5

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEMO_DIR="$REPO_ROOT/examples/terminal-tetris"

# Clean slate — remove any previous demo artifacts
rm -rf "$DEMO_DIR"

# Show empty examples dir
echo "$ ls examples/"
ls "$REPO_ROOT/examples/"
sleep 1.5

# Create project directory and enter it
echo ""
echo "$ mkdir -p examples/terminal-tetris && cd examples/terminal-tetris"
mkdir -p "$DEMO_DIR"
cd "$DEMO_DIR"
sleep 1

# Launch Claude Code in the empty project dir
echo "$ claudy zai --yolo"
echo ""
sleep 1

CLAUDE_CODE_HIDE_ACCOUNT_INFO=1 CLAUDE_HOME="$HOME/workspace" claudy zai --yolo

# ── Outro (after Claude Code exits) ───────────────────
cd - > /dev/null
section "Links"
echo "  GitHub:   https://github.com/epicsagas/epic-harness"
echo "  Docs:     QUICKSTART.md"
echo "  Commands: /spec  /go  /audit  /ship  /team  /evolve"
echo ""
echo "  Install:  cargo install epic-harness"
echo "  Setup:    epic install"
sleep 2

echo ""
echo "── End of demo ──────────────────────────────────"
sleep 1
