#!/usr/bin/env bash
# Validate the gemini integration structure and content
set -euo pipefail

BASE="/Volumes/Micron/projects/epic-harness/integrations/gemini"
PASS=0
FAIL=0

ok()   { echo "  PASS: $1"; PASS=$((PASS+1)); }
fail() { echo "  FAIL: $1"; FAIL=$((FAIL+1)); }

# ── settings.json ──────────────────────────────────────────────────────────
echo "=== settings.json ==="
f="$BASE/settings.json"
[ -f "$f" ] && ok "file exists" || fail "file missing"
python3 -c "import json,sys; json.load(open('$f'))" 2>/dev/null && ok "valid JSON" || fail "invalid JSON"
python3 -c "
import json
d=json.load(open('$f'))
h=d.get('hooks',{})
assert 'BeforeAgent' in h, 'missing BeforeAgent'
assert 'AfterAgent'  in h, 'missing AfterAgent'
assert 'AfterModel'  in h, 'missing AfterModel'
# AfterModel must be async
for item in h['AfterModel']:
    assert item.get('async') == True, 'AfterModel not async'
# commands must reference epic-harness subcommands
cmds = [item['command'] for items in h.values() for item in items]
assert any('resume'  in c for c in cmds), 'no resume'
assert any('reflect' in c for c in cmds), 'no reflect'
assert any('observe' in c for c in cmds), 'no observe'
assert any('guard'   in c for c in cmds), 'no guard'
print('hooks ok')
" 2>/dev/null && ok "hooks structure valid" || fail "hooks structure invalid"

# ── commands/ ──────────────────────────────────────────────────────────────
echo "=== commands/ ==="
for cmd in spec go check ship evolve team; do
  f="$BASE/commands/$cmd.md"
  [ -f "$f" ] && ok "$cmd.md exists" || fail "$cmd.md missing"
  # must have frontmatter
  head -1 "$f" | grep -q "^---" && ok "$cmd.md has frontmatter" || fail "$cmd.md missing frontmatter"
done

# go.md must NOT say "parallel subagents" (adapted for Gemini)
grep -qi "parallel subagent" "$BASE/commands/go.md" && fail "go.md still says 'parallel subagents'" || ok "go.md adapted for sequential execution"
# go.md must mention sequential
grep -qi "sequential" "$BASE/commands/go.md" && ok "go.md mentions sequential" || fail "go.md missing sequential mention"

# check.md must NOT say "run_in_background" (not supported in Gemini)
grep -q "run_in_background" "$BASE/commands/check.md" && fail "check.md still uses run_in_background" || ok "check.md adapted"

# CLAUDE.md references replaced with GEMINI.md
grep -qi "CLAUDE\.md" "$BASE/commands/spec.md" && fail "spec.md still references CLAUDE.md" || ok "spec.md uses GEMINI.md refs"

# ── skills/ ────────────────────────────────────────────────────────────────
echo "=== skills/ ==="
for skill in tdd secure verify simplify perf document context; do
  f="$BASE/skills/$skill/SKILL.md"
  [ -f "$f" ] && ok "$skill/SKILL.md exists" || fail "$skill/SKILL.md missing"
  head -1 "$f" | grep -q "^---" && ok "$skill/SKILL.md has frontmatter" || fail "$skill/SKILL.md missing frontmatter"
  grep -q "name:" "$f" && ok "$skill/SKILL.md has name field" || fail "$skill/SKILL.md missing name field"
done

# ── agents/ ────────────────────────────────────────────────────────────────
echo "=== agents/ ==="
for agent in builder reviewer auditor planner; do
  f="$BASE/agents/$agent.md"
  [ -f "$f" ] && ok "$agent.md exists" || fail "$agent.md missing"
  head -1 "$f" | grep -q "^---" && ok "$agent.md has frontmatter" || fail "$agent.md missing frontmatter"
done
# agents must note sequential execution (no parallel)
grep -qi "sequential" "$BASE/agents/builder.md" || grep -qi "sequential" "$BASE/agents/planner.md" && ok "agents note sequential execution" || fail "agents missing sequential note"

# ── GEMINI.md ──────────────────────────────────────────────────────────────
echo "=== GEMINI.md ==="
f="$BASE/GEMINI.md"
[ -f "$f" ] && ok "GEMINI.md exists" || fail "GEMINI.md missing"
grep -q "epic-harness" "$f" && ok "GEMINI.md mentions epic-harness" || fail "GEMINI.md missing epic-harness"
grep -q "/spec" "$f" && ok "GEMINI.md lists /spec" || fail "GEMINI.md missing /spec"
grep -q "/go"   "$f" && ok "GEMINI.md lists /go"   || fail "GEMINI.md missing /go"

# ── install.md ─────────────────────────────────────────────────────────────
echo "=== install.md ==="
f="$BASE/install.md"
[ -f "$f" ] && ok "install.md exists" || fail "install.md missing"
grep -q "settings.json"   "$f" && ok "mentions settings.json"   || fail "missing settings.json mention"
grep -q "GEMINI.md"       "$f" && ok "mentions GEMINI.md"       || fail "missing GEMINI.md mention"
grep -q "epic-harness"    "$f" && ok "mentions epic-harness binary" || fail "missing binary mention"

# ── summary ────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
