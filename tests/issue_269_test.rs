use epic_harness::shared::helpers::copy_dir_counted;
use std::path::Path;

fn link_directory(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let out = std::process::Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn counted_copy_rejects_external_directory_links_without_copying_target() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("legacy");
    let target = temp.path().join("target");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(source.join("ordinary.txt"), "ordinary").unwrap();
    std::fs::write(outside.join("external.txt"), "external-original").unwrap();
    link_directory(&outside, &source.join("nested"));
    let result = copy_dir_counted(&source, &target);
    assert_eq!(result.ok, 1);
    assert_eq!(
        result.errors, 1,
        "unsupported links must prevent successful destructive migration"
    );
    assert!(!target.join("nested/external.txt").exists());
    assert_eq!(
        std::fs::read_to_string(outside.join("external.txt")).unwrap(),
        "external-original"
    );
    assert!(source.join("nested").symlink_metadata().is_ok());
}
