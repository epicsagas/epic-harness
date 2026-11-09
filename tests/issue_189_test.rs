use epic_harness::shared::classify::extract_file;

#[test]
fn issue_189_native_separators_keep_directory_identity() {
    let paths = [
        r"src\component0\index.rs",
        r"src\component1\index.rs",
        r"src\component2\index.rs",
    ];
    for path in paths {
        assert_eq!(extract_file(path), Some(path));
    }
    assert_ne!(extract_file(paths[0]), extract_file(paths[1]));
    assert_eq!(
        extract_file("src/component0/index.rs"),
        Some("src/component0/index.rs")
    );
}
