use epic_harness::shared::classify::{classify_bash_failure, is_read_only_command};

#[test]
fn issue_193_mutating_display_command_modes_are_not_quoted_reads() {
    for command in [
        "git branch -D synthetic",
        "git branch new-branch",
        "git branch --move old new",
        "git remote remove synthetic",
        "git remote add synthetic local",
        "find synthetic -delete",
        "find synthetic -exec echo synthetic ;",
        "sed -I .bak s/a/b/ absent.txt",
    ] {
        assert!(!is_read_only_command(command), "mutating mode: {command}");
        assert_eq!(
            classify_bash_failure(command, "Error: synthetic mutation failed", "", false),
            Some("runtime_error")
        );
    }
    for command in [
        "git branch",
        "git branch --all",
        "git branch --list synthetic",
        "git remote -v",
        "git remote get-url origin",
        "find src -name '*.rs'",
        "sed -n 1,20p source.txt",
    ] {
        assert!(is_read_only_command(command), "display control: {command}");
        assert_eq!(
            classify_bash_failure(command, "Error: quoted source", "", false),
            None
        );
    }
}
