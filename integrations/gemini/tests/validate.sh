#!/usr/bin/env bash
# Validate the gemini integration structure and content
set -euo pipefail

BASE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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
assert 'BeforeModel' in h, 'missing BeforeModel'
# AfterModel must be async
for item in h['AfterModel']:
    assert item.get('async') == True, 'AfterModel not async'
# All hooks must have timeout
for event_items in h.values():
    for item in event_items:
        assert 'timeout' in item, f'{item.get(\"name\", \"unknown\")} missing timeout'
        assert item['timeout'] == 30000, f'{item[\"name\"]} timeout not 30000ms'
# hooks must be flat (not nested)
for event_name, event_items in h.items():
    for item in event_items:
        assert 'hooks' not in item, f'nested hooks structure in {event_name}'
# commands must reference epic-harness subcommands
cmds = [item['command'] for items in h.values() for item in items]
assert any('resume'  in c for c in cmds), 'no resume'
assert any('reflect' in c for c in cmds), 'no reflect'
assert any('observe' in c for c in cmds), 'no observe'
assert any('guard'   in c for c in cmds), 'no guard'
# mcpServers must exist
assert 'mcpServers' in d, 'missing mcpServers'
assert 'harness-mem' in d['mcpServers'], 'missing harness-mem in mcpServers'
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
# Verify no stale .toml files remain
for cmd in spec go check ship evolve team; do
  f="$BASE/commands/$cmd.toml"
  [ -f "$f" ] && fail "$cmd.toml still exists (should be .md)" || ok "$cmd.toml correctly removed"
done

# go.md must NOT say "parallel subagents" (adapted for Gemini)
grep -qi "parallel subagent" "$BASE/commands/go.md" && fail "go.md still says 'parallel subagents'" || ok "go.md adapted for sequential execution"
# go.md must mention sequential
grep -qi "sequential" "$BASE/commands/go.md" && ok "go.md mentions sequential" || fail "go.md missing sequential mention"

# check.md must NOT say "run_in_background" (not supported in Gemini)
grep -q "run_in_background" "$BASE/commands/check.md" && fail "check.md still uses run_in_background" || ok "check.md adapted"

# spec.md must NOT reference CLAUDE.md
grep -qi "CLAUDE\.md" "$BASE/commands/spec.md" && fail "spec.md still references CLAUDE.md" || ok "spec.md uses GEMINI.md refs"

# team.md must reference .gemini/agents/ not .claude/agents/
grep -q '\.claude/agents/' "$BASE/commands/team.md" && fail "team.md still references .claude/agents/" || ok "team.md uses .gemini/agents/ refs"

# ── skills/ ────────────────────────────────────────────────────────────────
# TODO: Skills will be provided in a future release. Re-enable when available.
echo "=== skills/ (skipped — future release) ==="
ok "skills check skipped (future release)"

# ── agents/ ────────────────────────────────────────────────────────────────
# TODO: Agents will be provided in a future release. Re-enable when available.
echo "=== agents/ (skipped — future release) ==="
ok "agents check skipped (future release)"

# ── GEMINI.md ──────────────────────────────────────────────────────────────
echo "=== GEMINI.md ==="
f="$BASE/GEMINI.md"
[ -f "$f" ] && ok "GEMINI.md exists" || fail "GEMINI.md missing"
grep -q "epic-harness" "$f" && ok "GEMINI.md mentions epic-harness" || fail "GEMINI.md missing epic-harness"
grep -q "/spec" "$f" && ok "GEMINI.md lists /spec" || fail "GEMINI.md missing /spec"
grep -q "/go"   "$f" && ok "GEMINI.md lists /go"   || fail "GEMINI.md missing /go"
grep -q "/team" "$f" && ok "GEMINI.md lists /team" || fail "GEMINI.md missing /team"

# ── install.md ─────────────────────────────────────────────────────────────
echo "=== install.md ==="
f="$BASE/install.md"
[ -f "$f" ] && ok "install.md exists" || fail "install.md missing"
grep -q "settings.json"   "$f" && ok "mentions settings.json"   || fail "missing settings.json mention"
grep -q "GEMINI.md"       "$f" && ok "mentions GEMINI.md"       || fail "missing GEMINI.md mention"
grep -q "epic-harness"    "$f" && ok "mentions epic-harness binary" || fail "missing binary mention"
grep -q '\.md' "$f" && grep -qv '\.toml' "$f" && ok "install.md references .md (not .toml)" || fail "install.md still references .toml"

# ── summary ────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
