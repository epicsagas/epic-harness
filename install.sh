#!/usr/bin/env bash
# epic-harness multi-tool installer
# Usage: ./install.sh --tool=<codex|gemini|cursor|antigravity> [--global]
#
# Options:
#   --tool=NAME    Target tool to install integration for (required)
#   --global       Install to user-global config dir instead of project-local
#   --dry-run      Show what would be installed without doing it
#   --help         Show this help

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INTEGRATIONS_DIR="$SCRIPT_DIR/integrations"

TOOL=""
GLOBAL=false
DRY_RUN=false

# ── Argument parsing ──────────────────────────────────────────────────────────

for arg in "$@"; do
  case "$arg" in
    --tool=*)  TOOL="${arg#--tool=}" ;;
    --global)  GLOBAL=true ;;
    --dry-run) DRY_RUN=true ;;
    --help|-h)
      sed -n '2,10p' "$0" | sed 's/^# //'
      exit 0
      ;;
    *)
      echo "Unknown option: $arg" >&2
      exit 1
      ;;
  esac
done

if [[ -z "$TOOL" ]]; then
  echo "Error: --tool is required. Use one of: codex, gemini, cursor, antigravity" >&2
  exit 1
fi

# ── Helpers ───────────────────────────────────────────────────────────────────

info()  { echo "[harness] $*"; }
ok()    { echo "[harness] ✓ $*"; }
skip()  { echo "[harness] ~ $* (already exists, skipping)"; }
run()   { if $DRY_RUN; then echo "[dry-run] $*"; else "$@"; fi; }
copy()  {
  local src="$1" dest="$2"
  if $DRY_RUN; then
    echo "[dry-run] cp -r $src → $dest"
    return
  fi
  if [[ -d "$src" ]]; then
    # Avoid cp -r src/ dest/ nesting: copy contents into dest/
    mkdir -p "$dest"
    cp -rP "$src/." "$dest/"
  else
    mkdir -p "$(dirname "$dest")"
    cp -P "$src" "$dest"
  fi
  ok "$(basename "$src") → $dest"
}
copy_if_missing() {
  local src="$1" dest="$2"
  if [[ -e "$dest" ]]; then
    skip "$dest"
  else
    copy "$src" "$dest"
  fi
}

check_binary() {
  if command -v epic-harness &>/dev/null; then
    ok "epic-harness binary found: $(command -v epic-harness)"
  else
    echo ""
    echo "[harness] WARNING: epic-harness binary not found in PATH."
    echo "[harness]   Install via:"
    echo "[harness]     brew install epicsagas/tap/epic-harness"
    echo "[harness]     cargo install epic-harness"
    echo ""
  fi
}

# ── Tool-specific install functions ──────────────────────────────────────────

install_codex() {
  local src="$INTEGRATIONS_DIR/codex"
  local project_dir="${CODEX_CONFIG_DIR:-$HOME/.codex}"
  local target_dir
  if $GLOBAL; then
    target_dir="$project_dir"
  else
    target_dir="${PWD}/.codex"
  fi

  info "Installing Codex CLI integration → $target_dir"

  copy_if_missing "$src/hooks.json"   "$target_dir/hooks.json"
  copy_if_missing "$src/commands"     "$target_dir/commands"
  copy_if_missing "$src/skills"       "$target_dir/skills"
  copy_if_missing "$src/agents"       "$target_dir/agents"

  check_binary
  info "Done. See integrations/codex/install.md for details."
}

install_gemini() {
  local src="$INTEGRATIONS_DIR/gemini"
  local target_dir
  if $GLOBAL; then
    target_dir="$HOME/.gemini"
  else
    target_dir="${PWD}/.gemini"
  fi

  info "Installing Gemini CLI integration → $target_dir"

  copy_if_missing "$src/settings.json" "$target_dir/settings.json"
  copy_if_missing "$src/commands"      "$target_dir/commands"
  copy_if_missing "$src/skills"        "$target_dir/skills"
  copy_if_missing "$src/agents"        "$target_dir/agents"

  local gemini_md="${PWD}/GEMINI.md"
  if [[ -f "$gemini_md" ]]; then
    info "GEMINI.md already exists. Append snippet manually from integrations/gemini/GEMINI.md"
  else
    copy_if_missing "$src/GEMINI.md" "$gemini_md"
  fi

  check_binary
  info "Done. See integrations/gemini/install.md for details."
}

install_cursor() {
  local src="$INTEGRATIONS_DIR/cursor"
  local target_dir
  if $GLOBAL; then
    target_dir="$HOME/.cursor"
  else
    target_dir="${PWD}/.cursor"
  fi

  info "Installing Cursor integration → $target_dir (requires Cursor 1.7+)"

  copy_if_missing "$src/hooks.json"  "$target_dir/hooks.json"
  copy_if_missing "$src/rules"       "$target_dir/rules"
  copy_if_missing "$src/commands"    "$target_dir/commands"
  copy_if_missing "$src/agents"      "$target_dir/agents"

  check_binary
  info "Done. See integrations/cursor/install.md for details."
}

install_antigravity() {
  local src="$INTEGRATIONS_DIR/antigravity"
  local target_dir
  if $GLOBAL; then
    target_dir="$HOME/.agents"
  else
    target_dir="${PWD}/.agents"
  fi

  info "Installing Antigravity integration → $target_dir"
  info "Note: Ring 0 hooks not available — using AGENTS.md + skills/workflows instead."

  copy_if_missing "$src/skills"     "$target_dir/skills"
  copy_if_missing "$src/workflows"  "$target_dir/workflows"
  copy_if_missing "$src/agents"     "$target_dir/agents"

  local agents_md="${PWD}/AGENTS.md"
  if [[ -f "$agents_md" ]]; then
    info "AGENTS.md already exists. Append epic-harness section manually from integrations/antigravity/AGENTS.md"
  else
    copy_if_missing "$src/AGENTS.md" "$agents_md"
  fi

  check_binary
  info "Done. See integrations/antigravity/install.md for details."
}

# ── Dispatch ──────────────────────────────────────────────────────────────────

case "$TOOL" in
  codex)        install_codex ;;
  gemini)       install_gemini ;;
  cursor)       install_cursor ;;
  antigravity)  install_antigravity ;;
  *)
    echo "Error: unknown tool '$TOOL'. Use one of: codex, gemini, cursor, antigravity" >&2
    exit 1
    ;;
esac
