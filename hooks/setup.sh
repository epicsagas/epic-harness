#!/usr/bin/env bash
# Bootstrap: install epic-harness binary if missing, then delegate to `update`.
set -euo pipefail

EH=""
command -v epic-harness >/dev/null 2>&1 && EH="$(command -v epic-harness)"

if test -z "$EH"; then
  echo "[epic] Installing epic-harness..."
  if command -v brew >/dev/null 2>&1; then
    brew install epic-harness 2>/dev/null || cargo install epic-harness
  elif command -v cargo-binstall >/dev/null 2>&1; then
    cargo binstall -y --no-confirm epic-harness
  elif command -v cargo >/dev/null 2>&1; then
    cargo install epic-harness
  else
    echo "[epic] Install Rust first: https://rustup.rs" >&2
    exit 0
  fi
fi

exec epic-harness update
