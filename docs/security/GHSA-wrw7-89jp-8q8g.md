# glib VariantStrIter safety backport

Issue: #138. Advisory: [RUSTSEC-2024-0429 / GHSA-wrw7-89jp-8q8g](https://github.com/advisories/GHSA-wrw7-89jp-8q8g).

Tauri 2's Linux GTK/WebKit dependency family requires glib 0.18. A glib 0.20 lockfile bump cannot satisfy those constraints. Moving the entire desktop to a Tauri 3 prerelease is unnecessary for this defect and would change unrelated APIs.

The workspace patches glib to the MIT-licensed upstream 0.18.5 crate in `vendor/glib-0.18.5`. The downloaded archive was SHA-256 verified against the original crates.io lock entry; original license, copyright and VCS metadata are retained. `EPIC_BACKPORT.json` records archive/source hashes and the fix provenance.

The production source change is exactly the fix from [gtk-rs pull request 1343](https://github.com/gtk-rs/gtk-rs-core/pull/1343), upstream merge `05dff0ee696f9bcd8617cd48c4b812d046d440cb`: make the C string out-pointer mutable and pass `&mut p` to `g_variant_get_child`. Passing an immutable reference into a variadic C out-argument was undefined behavior; a safe-looking release build could subsequently dereference NULL. No public API or stored data format changes.

Verification:

```sh
node scripts/check-glib-backport.mjs
# Linux with pkg-config and libglib2.0-dev installed:
cargo test --manifest-path vendor/glib-0.18.5/Cargo.toml --locked --release --test issue_138_variant_str_iter
```

The optimized regression exercises `next`, `nth`, `next_back`, `nth_back`, collection and reversed collection against actual GLib string-array variants. The unmodified source crashed with SIGSEGV on Rust 1.99.1/Linux; the backport must pass the same test. Debug-only tests are insufficient for this advisory.

The crate intentionally keeps version 0.18.5 for GTK-family compatibility. Registry/version-only audit tools may therefore still flag it; they cannot infer a reviewed local backport. A source exception for **only RUSTSEC-2024-0429** is justified only after `check-glib-backport.mjs` verifies the pinned archive/fix identity, exact patched source hash, Cargo path override and non-registry lock resolution, and the optimized Linux regression passes. Do not suppress other advisories or describe this as an upstream 0.20 upgrade. Remove the backport once a compatible upstream fixed family is adopted and these tests pass.
