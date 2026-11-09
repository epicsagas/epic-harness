/// Embedded allowlist: payload text can never select a filesystem path or executable.
pub(super) fn select_skills(prompt: &str) -> Vec<(&'static str, &'static str)> {
    let words: Vec<String> = prompt
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let has = |signals: &[&str]| words.iter().any(|w| signals.contains(&w.as_str()));
    let mut skills = Vec::new();
    if has(&[
        "debug", "crash", "broken", "error", "failure", "failing", "bug", "fix", "repair",
    ]) {
        skills.push(("debug", include_str!("../../../skills/debug/SKILL.md")));
    }
    if has(&[
        "auth",
        "authentication",
        "authorization",
        "login",
        "password",
        "security",
        "secure",
        "token",
        "credentials",
        "permission",
        "database",
        "api",
    ]) {
        skills.push(("secure", include_str!("../../../skills/secure/SKILL.md")));
    }
    if has(&[
        "performance",
        "perf",
        "slow",
        "latency",
        "optimize",
        "optimization",
        "bottleneck",
    ]) {
        skills.push(("perf", include_str!("../../../skills/perf/SKILL.md")));
    }
    if has(&["document", "documentation", "docs", "docstrings"]) {
        skills.push((
            "document",
            include_str!("../../../skills/document/SKILL.md"),
        ));
    }
    // At most two specialist skills, leaving room for implementation/verification guidance.
    skills.truncate(2);
    let implementation = has(&[
        "implement",
        "build",
        "add",
        "create",
        "write",
        "change",
        "edit",
        "update",
        "feature",
        "fix",
        "repair",
        "refactor",
        "test",
        "tests",
    ]);
    if implementation {
        skills.push(("tdd", include_str!("../../../skills/tdd/SKILL.md")));
    }
    if implementation || !skills.is_empty() {
        skills.push(("verify", include_str!("../../../skills/verify/SKILL.md")));
    }
    skills
}
