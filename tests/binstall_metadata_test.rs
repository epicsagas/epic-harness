//! cargo-binstall metadata must describe the archives cargo-dist publishes:
//! `epic-harness-<target>.tar.xz` with a top-level `epic-harness-<target>/`
//! directory on Unix, and a flat `epic-harness-<target>.zip` on Windows
//! (install.sh and install.ps1 extract them the same way).

fn binstall() -> toml::Value {
    let manifest: toml::Value = toml::from_str(include_str!("../Cargo.toml")).unwrap();
    manifest["package"]["metadata"]["binstall"].clone()
}

#[test]
fn unix_archive_is_tar_xz_with_top_level_dir() {
    let b = binstall();
    let url = b["pkg-url"].as_str().unwrap();
    assert!(url.ends_with("/epic-harness-{ target }.tar.xz"), "{url}");
    assert_eq!(b["pkg-fmt"].as_str(), Some("txz"));
    assert_eq!(
        b["bin-dir"].as_str(),
        Some("epic-harness-{ target }/{ bin }")
    );
}

#[test]
fn windows_archive_is_flat_zip_for_every_binary() {
    let w = &binstall()["overrides"]["x86_64-pc-windows-msvc"];
    let url = w["pkg-url"].as_str().unwrap();
    assert!(url.ends_with("/epic-harness-{ target }.zip"), "{url}");
    assert_eq!(w["pkg-fmt"].as_str(), Some("zip"));
    // The zip holds epic.exe and epic-harness.exe at its root: no directory
    // prefix, and the binary name comes from `{ bin }`, not a fixed file.
    assert_eq!(w["bin-dir"].as_str(), Some("{ bin }{ binary-ext }"));
}
