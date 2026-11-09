# deepmerge prototype-poisoning backport

Issue: #156. Advisory: [GHSA-gcqx-w48m-w2h4](https://github.com/advisories/GHSA-gcqx-w48m-w2h4), CVE-2026-93753.

`@sveltejs/vite-plugin-svelte` consumes `deepmerge` 4.3.1 during dashboard development/build. This is not loaded by the shipped single-file dashboard or Rust CLI. As of this repair, 4.3.1 remains the latest release, and even the newest Vite Svelte plugin still requires it. Upgrading the entire frontend does not remove the vulnerable implementation.

`app/patches/deepmerge@4.3.1.patch` preserves the existing merge API and applies the remediation described in [upstream issue 273](https://github.com/TehShrike/deepmerge/issues/273):

- Reject `__proto__`, `constructor`, and `prototype` by key name in both target-copy and source-merge paths.
- Define returned own properties with `Object.defineProperty`, so assigning a key cannot invoke the prototype setter.
- Fail closed when target property introspection throws.
- Apply the same change to source, CommonJS, and UMD entry points.

The original package and MIT license remain installed; this is a source patch, not an unrelated replacement package. Original source and copyright: [TehShrike/deepmerge](https://github.com/TehShrike/deepmerge), MIT, James Halliday and contributors. The patch is declared in `app/pnpm-workspace.yaml` and its exact hash is recorded in `app/pnpm-lock.yaml`. A frozen install therefore cannot silently drop it.

Run after `pnpm --dir app install --frozen-lockfile`:

```sh
node --test scripts/deepmerge-security.test.mjs
node --test scripts/dependency-audit-policy.test.mjs
node scripts/audit-dashboard-dependencies.mjs
```

The tests resolve `deepmerge` from the actual Svelte plugin, not a test-only copy. They cover null-prototype targets, nested null values, target-side poisoning, nested forbidden keys, symbols, normal cloning/arrays, and custom merge options. Four behavioral tests failed against the original package; all pass with the patch. An additional check verifies lockfile hash and actual patched-module resolution.

Registry-only scanners may continue flagging version 4.3.1 because no upstream patched version exists. That is not evidence the patch was applied: require the hash/resolution checks and adversarial tests before accepting a narrowly scoped source-backport exception. Do not suppress unrelated advisories. Remove this backport only when an upstream fixed version is adopted and the same behavioral tests pass unpatched.

The audit command runs those real-consumer checks before the registry audit. It reports the exact verified source backport explicitly, rejects every other advisory, and rejects missing/inconsistent audit data. Benign null/primitive placeholders and throwing proxies have separate controls so security hardening does not silently discard normal compiler configuration.
