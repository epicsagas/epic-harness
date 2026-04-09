---
name: rs-care
description: "Preset: Rust files need cargo check after edits."
---

# Rust file care (preset)

## Process
1. Run `cargo check` after editing .rs files
2. Run `cargo clippy` for idiomatic patterns
3. Check borrow checker errors carefully — read the full message

## Red Flags
- Editing Rust without cargo check
- Using `.unwrap()` in non-test code
