#!/usr/bin/env bash
# Test: commit skill exists in all 5 integrations
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FAIL=0

check() {
  local file="$1" pattern="$2" label="$3"
  if [[ ! -f "$file" ]]; then
    echo "FAIL: $label — file not found: $file"; FAIL=1; return
  fi
  if ! grep -q "$pattern" "$file"; then
    echo "FAIL: $label — pattern '$pattern' not found"; FAIL=1; return
  fi
  echo "PASS: $label"
}

check "$ROOT/integrations/codex/skills/commit/SKILL.md" "type(scope): description" "codex commit skill"
check "$ROOT/integrations/gemini/skills/commit/SKILL.md" "type(scope): description" "gemini commit skill"
check "$ROOT/integrations/cursor/rules/harness-skills.mdc" "## Commit" "cursor commit section"
check "$ROOT/integrations/opencode/plugins/epic-harness.js" "commit" "opencode commit skill"
check "$ROOT/integrations/cline/rules/epic-harness.md" "Commit" "cline commit section"

# Verify CC types present
for f in \
  "$ROOT/integrations/codex/skills/commit/SKILL.md" \
  "$ROOT/integrations/gemini/skills/commit/SKILL.md"; do
  check "$f" "feat" "CC types in $(basename "$(dirname "$(dirname "$f")")")"
done

exit $FAIL
