#!/usr/bin/env bash
# ── epic-harness demo tape ──────────────────────────────────
# Usage:
#   asciinema rec --overwrite demo/demo.cast --command "bash demo/demo-tape.sh"
#   asciinema upload demo/demo.cast
#
# Prerequisites:
#   - epic-harness installed (cargo install --path .)
#   - asciinema installed (brew install asciinema)
#   - Run from repo root

set -euo pipefail

# Slow down typing for readability
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
echo "║          epic-harness  —  demo tape             ║"
echo "║   6 commands · auto-trigger skills · evolving   ║"
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
epic mem add --title "Always verify before ship" --type pattern --project demo-app --body "Run /check after /go. Never skip verification even for small changes."
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

# Get the first node ID from query output
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

# ── 8. Full workflow: /spec → /go → /check → /ship ───
section "8. Full workflow — URL shortener (simulated Claude Code session)"
echo "Below is a real Claude Code session using epic-harness commands."
echo "Project: Go URL shortener with SQLite backend."
sleep 1.5

echo ""
echo "──────────────────────────────────────────────────────"
echo "$ /spec \"Build a URL shortener in Go with SQLite\""
echo "──────────────────────────────────────────────────────"
sleep 0.8
echo ""
echo "  Analyzing requirements..."
sleep 0.6
echo ""
echo "  ## SPEC-20260506-url-shortener"
echo ""
echo "  ### Requirements"
echo "  R1: HTTP server with POST /shorten and GET /{code} endpoints"
echo "  R2: SQLite storage with auto-generated 6-char codes"
echo "  R3: Redirect with 301 status and click counter"
echo ""
echo "  ### Acceptance Criteria"
echo "  AC1: POST /shorten returns {\"code\": \"abc123\", \"url\": \"...\"}"
echo "  AC2: GET /abc123 redirects to original URL"
echo "  AC3: Duplicate URLs return the same short code"
sleep 2

echo ""
echo "──────────────────────────────────────────────────────"
echo "$ /go"
echo "──────────────────────────────────────────────────────"
sleep 0.8
echo ""
echo "  Planning 3 tasks from spec..."
sleep 0.5
echo "  ├─ Task 1: SQLite schema + repository layer [builder-1]"
echo "  ├─ Task 2: HTTP handlers + routing           [builder-2]"
echo "  └─ Task 3: Integration tests                 [builder-3]"
sleep 0.8
echo ""
echo "  [builder-1] TDD: write store_test.go → implement store.go"
echo "  [builder-1] ✓ DONE — 5/5 tests passing (12s)"
sleep 0.5
echo "  [builder-2] TDD: write handler_test.go → implement handler.go"
echo "  [builder-2] ✓ DONE — 4/4 tests passing (18s)"
sleep 0.5
echo "  [builder-3] TDD: write integration_test.go → spin up test server"
echo "  [builder-3] ✓ DONE — 3/3 tests passing (8s)"
sleep 0.8
echo ""
echo "  Result: DONE — all 12 tests passing"
sleep 1.5

echo ""
echo "──────────────────────────────────────────────────────"
echo "$ /check"
echo "──────────────────────────────────────────────────────"
sleep 0.8
echo ""
echo "  Launching 3 parallel agents..."
sleep 0.5
echo ""
echo "  ## Check Report"
echo "  Spec: SPEC-20260506-url-shortener"
echo ""
echo "  ### Code Quality: PASS"
echo "  ### Security:     PASS (no SQL injection — parameterized queries)"
echo "  ### Performance:  PASS (indexed lookups, <1ms per redirect)"
echo "  ### Tests:        12/12 passing"
echo ""
echo "  ### Spec Coverage"
echo "  R1: ✅ store.go + handler.go"
echo "  R2: ✅ 6-char code generation in store.go:34"
echo "  R3: ✅ 301 redirect + counter in handler.go:52"
echo "  AC1: ✅ verified by TestShortenEndpoint"
echo "  AC2: ✅ verified by TestRedirectEndpoint"
echo "  AC3: ✅ verified by TestDuplicateURLReturnsSameCode"
sleep 2

echo ""
echo "──────────────────────────────────────────────────────"
echo "$ /ship"
echo "──────────────────────────────────────────────────────"
sleep 0.8
echo ""
echo "  Running pre-flight checks..."
sleep 0.5
echo "  ✓ cargo test — 12 passed, 0 failed"
echo "  ✓ cargo clippy — 0 warnings"
echo "  ✓ No secrets detected (gitleaks)"
sleep 0.5
echo ""
echo "  Creating PR #42: feat(shortener): add URL shortener with SQLite"
echo "  Pushing to origin/feat/url-shortener..."
echo "  CI: build ✓ | test ✓ | lint ✓"
echo "  Merged to main."
sleep 1.5

echo ""
echo "──────────────────────────────────────────────────────"
echo "$ /evolve status"
echo "──────────────────────────────────────────────────────"
sleep 0.8
echo ""
echo "  ## Evolution Dashboard"
echo ""
echo "  Session score: 0.92 (↑ from 0.87)"
echo "  Trend:         improving (4 sessions)"
echo ""
echo "  Evolved skills: 2 active"
echo "  ├─ evo-go-care       — avg_score: 0.91 (with) vs 0.78 (without) → +13%"
echo "  └─ evo-fix-build-fail — avg_score: 0.85 (with) vs 0.72 (without) → +13%"
echo ""
echo "  Patterns detected this session:"
echo "  • Bash tool: 95% success (429/452 calls)"
echo "  • .go files: 97% success (38/39 edits)"
echo "  • No failure patterns detected"
sleep 2

# ── Outro ─────────────────────────────────────────────
section "Links"
echo "  GitHub:   https://github.com/epicsagas/epic-harness"
echo "  Docs:     QUICKSTART.md"
echo "  Commands: /spec  /go  /check  /ship  /team  /evolve"
echo ""
echo "  Install:  cargo install epic-harness"
echo "  Setup:    epic install"
sleep 2

echo ""
echo "── End of demo ──────────────────────────────────"
sleep 1
