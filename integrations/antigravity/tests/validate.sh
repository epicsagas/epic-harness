#!/usr/bin/env bash
# Validate the antigravity integration structure and content
set -euo pipefail

BASE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PASS=0
FAIL=0

ok()   { echo "  PASS: $1"; PASS=$((PASS+1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL+1)); }

# ── AGENTS.md ──────────────────────────────────────────────────────────────
echo "=== AGENTS.md ==="
f="$BASE/AGENTS.md"
[ -f "$f" ] && ok "file exists" || fail "file missing"
grep -q "epic-harness" "$f" && ok "mentions epic-harness" || fail "missing epic-harness"
grep -q "epic-harness resume" "$f" && ok "mentions resume command" || fail "missing resume command"
grep -q "epic-harness reflect" "$f" && ok "mentions reflect command" || fail "missing reflect command"
grep -q "/spec" "$f" && ok "lists /spec" || fail "missing /spec"
grep -q "/go"   "$f" && ok "lists /go"   || fail "missing /go"
# Must NOT mention hooks (Antigravity has no hooks system)
grep -qi "PreToolUse\|PostToolUse\|SessionEnd\|hooks\.json" "$f" && fail "AGENTS.md incorrectly references hooks events" || ok "no hooks references"
# Must have guard rules section
grep -qi "Forbidden\|Guard\|never execute" "$f" && ok "has guard/forbidden section" || fail "missing guard rules"
# Must mention git push --force as forbidden
grep -q "force" "$f" && ok "mentions force-push guard" || fail "missing force-push guard"

# ── workflows/ ─────────────────────────────────────────────────────────────
echo "=== workflows/ ==="
for cmd in spec go check ship evolve team; do
  f="$BASE/workflows/$cmd.md"
  [ -f "$f" ] && ok "$cmd.md exists" || fail "$cmd.md missing"
  head -1 "$f" | grep -q "^---" && ok "$cmd.md has frontmatter" || fail "$cmd.md missing frontmatter"
  grep -q "name:" "$f" && ok "$cmd.md has name field" || fail "$cmd.md missing name field"
  grep -q "command:" "$f" && ok "$cmd.md has command field" || fail "$cmd.md missing command field"
done

# go.md must mention Manager view (Antigravity key differentiator)
grep -qi "manager view\|Manager View" "$BASE/workflows/go.md" && ok "go.md mentions Manager view" || fail "go.md missing Manager view"
# go.md must NOT say "run_in_background" (Claude Code concept, not Antigravity)
grep -q "run_in_background" "$BASE/workflows/go.md" && fail "go.md uses run_in_background (Claude Code API — not Antigravity)" || ok "go.md uses Antigravity patterns"

# check.md must mention Manager view
grep -qi "manager view\|Manager View" "$BASE/workflows/check.md" && ok "check.md mentions Manager view" || fail "check.md missing Manager view"
# check.md must NOT say "run_in_background"
grep -q "run_in_background" "$BASE/workflows/check.md" && fail "check.md uses run_in_background" || ok "check.md adapted for Antigravity"

# evolve.md must mention epic-harness reflect
grep -q "epic-harness reflect" "$BASE/workflows/evolve.md" && ok "evolve.md calls epic-harness reflect" || fail "evolve.md missing reflect call"

# CLAUDE.md references must be replaced with AGENTS.md
grep -qi "CLAUDE\.md" "$BASE/workflows/spec.md" && fail "spec.md still references CLAUDE.md" || ok "spec.md uses AGENTS.md refs"

# ── skills/ ────────────────────────────────────────────────────────────────
echo "=== skills/ ==="
for skill in tdd secure verify simplify perf document; do
  f="$BASE/skills/$skill.md"
  [ -f "$f" ] && ok "$skill.md exists" || fail "$skill.md missing"
  head -1 "$f" | grep -q "^---" && ok "$skill.md has frontmatter" || fail "$skill.md missing frontmatter"
  grep -q "name:" "$f" && ok "$skill.md has name field" || fail "$skill.md missing name field"
  grep -q "trigger:" "$f" && ok "$skill.md has trigger field" || fail "$skill.md missing trigger field"
done

# ── agents/ ────────────────────────────────────────────────────────────────
echo "=== agents/ ==="
for agent in builder reviewer auditor planner; do
  f="$BASE/agents/$agent.md"
  [ -f "$f" ] && ok "$agent.md exists" || fail "$agent.md missing"
  head -1 "$f" | grep -q "^---" && ok "$agent.md has frontmatter" || fail "$agent.md missing frontmatter"
  grep -q "name:" "$f" && ok "$agent.md has name field" || fail "$agent.md missing name field"
done

# agents must reference Manager view (parallel execution)
grep -qi "manager view\|Manager View\|parallel" "$BASE/agents/planner.md" && ok "planner notes parallel/manager capability" || fail "planner missing parallel/manager note"

# ── install.md ─────────────────────────────────────────────────────────────
echo "=== install.md ==="
f="$BASE/install.md"
[ -f "$f" ] && ok "install.md exists" || fail "install.md missing"
grep -q "AGENTS.md"    "$f" && ok "mentions AGENTS.md"    || fail "missing AGENTS.md mention"
grep -q "epic-harness" "$f" && ok "mentions epic-harness binary" || fail "missing binary mention"
grep -q ".agents/"     "$f" && ok "mentions .agents/ directory" || fail "missing .agents/ directory"
# Must note that Ring 0 (hooks) is not available
grep -qi "Ring 0\|no hooks\|hooks.*not\|without hooks" "$f" && ok "notes hooks not available" || fail "missing hooks limitation note"

# ── summary ────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
