#!/usr/bin/env zsh
# ── Episteme + orbit live recording setup ──────────────────────
#
# Usage:
#   zsh docs/demo/prepare.sh           # prepare only
#   zsh docs/demo/prepare.sh --record  # prepare + asciinema -> auto-launch claudy

set -eo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEMO_SRC="$REPO_ROOT/docs/demo/examples/user-api"
DEMO_DST="$REPO_ROOT/examples/user-api-demo"
DEMO_ROWS=40
DEMO_COLS=120

echo "-- Episteme + orbit recording setup ---------------"
echo ""

# 1. Copy target project (clean state)
echo "1. Preparing target project..."
rm -rf "$DEMO_DST"
mkdir -p "$DEMO_DST"
cp "$DEMO_SRC/user_service.py" "$DEMO_DST/user_service.py"
echo "   -> examples/user-api-demo/user_service.py ($(wc -l < "$DEMO_DST/user_service.py") lines)"
sleep 0.3

# 2. Set terminal size
echo "2. Terminal size: ${DEMO_COLS}x${DEMO_ROWS}"
printf "\033[8;%s;%st" "$DEMO_ROWS" "$DEMO_COLS"
sleep 0.3

# 3. Check tools
echo "3. Checking tools..."
for cmd in asciinema agg claudy epic; do
    if command -v "$cmd" &>/dev/null; then
        echo "   OK  $cmd"
    else
        echo "   MISSING $cmd"
    fi
done
sleep 0.3

# 4. Prompt guide
echo ""
echo "4. Prompt guide: docs/demo/demo-prompt.md"
echo ""

# 5. Record or manual
if [ "${1:-}" = "--record" ]; then
    echo "-- launching asciinema + claudy --------------------"
    echo "   Target: examples/user-api-demo"
    echo "   Cast:   docs/demo/episteme-orbit.cast"
    echo "   Stop:   Ctrl+C or /exit in Claude Code"
    echo ""
    sleep 1
    exec asciinema rec --overwrite "$REPO_ROOT/docs/demo/episteme-orbit.cast" \
      --command "cd '$DEMO_DST' && claudy zai --yolo"
else
    echo "-- manual recording -------------------------------"
    echo ""
    echo "  asciinema rec docs/demo/episteme-orbit.cast \\"
    echo "    --command 'cd examples/user-api-demo && claudy zai --yolo'"
    echo ""
    echo "  After recording, convert:"
    echo "    make demo-gif"
fi
