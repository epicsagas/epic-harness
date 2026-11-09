use super::*;

#[test]
fn issue_184_different_observation_collision_is_an_error_and_preserves_both() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source/obs");
    let target = temp.path().join("target/obs");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    let name = "session_20261109_fixture.jsonl";
    let a = "{\"action\":\"source-only\"}\n";
    let b = "{\"action\":\"target-only\"}\n";
    std::fs::write(source.join(name), a).unwrap();
    std::fs::write(target.join(name), b).unwrap();
    let result = copy_dir_files(&source, &target, false);
    assert!(
        result.is_err(),
        "distinct colliding histories must never be treated as transferred"
    );
    assert_eq!(std::fs::read_to_string(source.join(name)).unwrap(), a);
    assert_eq!(std::fs::read_to_string(target.join(name)).unwrap(), b);
    std::fs::write(source.join(name), b).unwrap();
    assert_eq!(
        copy_dir_files(&source, &target, false).unwrap(),
        0,
        "equal copies are an idempotent retry"
    );
}

#[test]
fn issue_184_different_evolved_skill_collision_is_not_success() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::create_dir_all(source.join("skill")).unwrap();
    std::fs::create_dir_all(target.join("skill")).unwrap();
    std::fs::write(source.join("skill/SKILL.md"), "source guidance").unwrap();
    std::fs::write(target.join("skill/SKILL.md"), "target guidance").unwrap();
    assert!(copy_evolved_dir(&source, &target, false).is_err());
    std::fs::write(source.join("skill/SKILL.md"), "target guidance").unwrap();
    assert_eq!(copy_evolved_dir(&source, &target, false).unwrap(), 0);
}
