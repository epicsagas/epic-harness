#!/usr/bin/env bash
# codex-live-check: isolated live-Codex verification env for epic-harness.
# Runs from this directory; .envrc holds the environment. See README.md.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$DIR/.envrc"
REAL_CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
# A CODEX_HOME left over from a sourced .envrc must not become the auth source.
[ "$REAL_CODEX_HOME" = "$CODEX_HOME" ] && REAL_CODEX_HOME="$HOME/.codex"

case "${1:-setup}" in
  clean)
    rm -rf "$CODEX_HOME" "$HARNESS_DIR" "$DIR/scratch" "$DIR/stderr.log"
    echo "removed $CODEX_HOME $HARNESS_DIR $DIR/scratch"
    exit 0
    ;;
  setup) ;;
  --fresh)
    rm -rf "$CODEX_HOME" "$HARNESS_DIR" "$DIR/scratch" "$DIR/stderr.log"
    ;;
  *)
    echo "usage: $0 [setup|--fresh|clean]" >&2
    exit 64
    ;;
esac

command -v epic >/dev/null || { echo "epic not on PATH; build the branch under test first" >&2; exit 1; }

mkdir -p "$CODEX_HOME" "$HARNESS_DIR" "$DIR/scratch/src"

# --- CODEX_HOME: fresh codex home, auth copied read-only from the real one ---
if [ ! -f "$CODEX_HOME/auth.json" ]; then
  [ -f "$REAL_CODEX_HOME/auth.json" ] || { echo "no auth.json in $REAL_CODEX_HOME; log in once first" >&2; exit 1; }
  cp "$REAL_CODEX_HOME/auth.json" "$CODEX_HOME/auth.json"
fi
[ -f "$CODEX_HOME/config.toml" ] || printf '# codex-live-check isolated home\n' > "$CODEX_HOME/config.toml"

# The TUI plugin screen needs the appserver payload (codex-managed, ~268M);
# symlink instead of copy. The test itself never uses plugins.
if [ -d "$REAL_CODEX_HOME/plugins/.plugin-appserver" ] && [ ! -e "$CODEX_HOME/plugins/.plugin-appserver" ]; then
  mkdir -p "$CODEX_HOME/plugins"
  ln -s "$REAL_CODEX_HOME/plugins/.plugin-appserver" "$CODEX_HOME/plugins/.plugin-appserver"
fi

# Hook manifest from HARNESS_SRC, PLUGIN_ROOT resolved to it.
# Marketplace is bypassed on purpose: it tracks remote HEAD (pre-merge code).
sed "s|\${PLUGIN_ROOT}|$HARNESS_SRC|g" "$HARNESS_SRC/.codex-plugin/hooks.json" \
  > "$CODEX_HOME/hooks.json"

# Register HARNESS_SRC as a local marketplace and install the plugin so codex
# serves the skills/prompts the autonomous (/orbit) flow needs. Local path:
# no network, tracks the checkout as-is.
codex plugin marketplace add "$HARNESS_SRC" >/dev/null 2>&1 || true
codex plugin add epic@epicsagas >/dev/null 2>&1 || true

# --- scratch project: py targets so polish has formatting work ---
cd "$DIR/scratch"
[ -f src/a.py ] || printf 'def add(a, b):\n    return a+b\n\nif __name__ == "__main__":\n    print(add(1, 2))\n' > src/a.py
[ -f src/b.py ] || printf 'def mul(a, b):\n    return a*b\n' > src/b.py

# --- evolved marker skills for the case-2 injection probe ---
# slug derives from the git toplevel name: "epic-harness"
EV="$HARNESS_DIR/projects/epic-harness/evolved"
if [ ! -d "$EV" ]; then
  mkdir -p "$EV"
  for pair in a:A7K b:B3M c:C9Q; do
    n="${pair%%:*}"; m="${pair##*:}"
    mkdir -p "$EV/evo-check-$n"
    printf -- '---\nname: evo-check-%s\ndescription: codex-live-check injection probe %s\n---\n\nHARNESS-CHECK-%s: This sentence proves evolved skill injection reached the model context. Marker %s.\n' \
      "$n" "$n" "$m" "$m" > "$EV/evo-check-$n/SKILL.md"
  done
fi

EPIC_V="$(epic --version 2>&1 | head -1)"
echo "env:        $DIR  (direnv allow once, then every cd here is identical)"
echo "epic:       $(command -v epic) ($EPIC_V)"
echo "manifest:   $HARNESS_SRC ($(git -C "$HARNESS_SRC" log -1 --format=%h 2>/dev/null || echo 'no git'))"
echo "hooks:      $CODEX_HOME/hooks.json ($(jq '.hooks | keys | length' "$CODEX_HOME/hooks.json") events)"
echo "next:       cd $DIR/scratch && codex 2> $DIR/stderr.log"
echo "prompts:    $DIR/prompts.txt   verdicts: $DIR/assert.sh"
