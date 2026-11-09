# SQLite WAL page initialization (issue #278)

Epic's SQLx 0.9 graph pins `libsqlite3-sys` 0.30.1, whose bundled SQLite 3.46.0
is affected by RUSTSEC-2026-0459 / CVE-2026-119375. Opening a less trusted
database together with a WAL whose page size differs can disclose uninitialized
page-cache bytes. SQLite 3.54.0 fixes this; Epic bundles official SQLite 3.54.1.

The source override keeps the SQLx-compatible 0.30.1 Rust bindings and build
features. Only the three engine C/header files and the binding version/source
constants change. Existing database formats and CLI interfaces are preserved.
`vendor/libsqlite3-sys-0.30.1/EPIC_BACKPORT.json` records both official crate
archive SHA256s, the engine source identity, hashes, and upstream fix commit
`e5bf119821f25be1d7f10ef0a2edbe175289166e`.

This applies to Epic's bundled SQLite configuration. Optional SQLCipher sources
are unchanged and are not enabled by Epic. A system SQLite selected explicitly
through upstream environment overrides requires independent version review.

Cargo audit does not inspect path dependencies. Consequently its zero finding
result alone cannot establish that this remediation is present. CI must also run
the strict source guard, actual WAL-reader regression, and actual SQLx tests:

```sh
node scripts/check-sqlite-backport.mjs
cc -O2 scripts/sqlite-wal-initialization-test.c -o /tmp/epic-sqlite-wal-test -ldl -lpthread -lm
/tmp/epic-sqlite-wal-test
cargo test -p epic-harness --locked --test sqlite_security_test
```

The C regression compiles the actual engine and feeds a synthetic 512-byte WAL
frame to its WAL reader with a 4096-byte output page. The original engine leaves
the output tail at sentinel `0xa5` (FAIL); the fixed engine initializes every tail
byte to zero (PASS). It does not create an exploit database or inspect process
memory. The SQLx tests verify `sqlite_version() == 3.54.1`, an older synthetic
database's read/write compatibility, and a copied normal database/WAL pair.

Windows GNU can compile the C regression with `gcc -O2 ... -o ...exe`;
the Rust regression runs on Windows MSVC and Linux in the normal build matrix.
The complete Epic memory tests provide additional schema/migration coverage.
