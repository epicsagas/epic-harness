//! Child-only process metadata prevents Eval cycles without a global run lock.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub(super) const CHAIN_ENV: &str = "EPIC_EVAL_CONTEXT_CHAIN";

#[derive(Deserialize, Serialize, PartialEq, Eq)]
struct EvaluationContext {
    project: PathBuf,
    config: PathBuf,
}

/// The Git common directory identifies main/linked checkouts and subdirectories
/// consistently. Non-Git projects use their canonical working directory.
fn project_identity(cwd: &Path) -> Result<PathBuf, String> {
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(cwd)
        .output()
        && output.status.success()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        if let Ok(path) = Path::new(text.trim()).canonicalize() {
            return Ok(path);
        }
    }
    cwd.canonicalize()
        .map_err(|_| "could not resolve the evaluation project context".into())
}

fn append_context(current: EvaluationContext, inherited: Option<&str>) -> Result<String, String> {
    let mut ancestors: Vec<EvaluationContext> = inherited
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| "invalid inherited Epic eval context chain".to_string())?
        .unwrap_or_default();
    if ancestors.contains(&current) {
        return Err(
            "recursive eval rejected: this evaluation context is already active in a parent workload"
                .into(),
        );
    }
    ancestors.push(current);
    serde_json::to_string(&ancestors)
        .map_err(|_| "could not encode the child evaluation context".into())
}

/// Return metadata for workload children only. The current process environment
/// is never mutated, and sibling measurements receive the same ancestor chain.
fn current_context(cwd: &Path, config: &Path) -> Result<EvaluationContext, String> {
    Ok(EvaluationContext {
        project: project_identity(cwd)?,
        config: config
            .canonicalize()
            .map_err(|_| "could not resolve the evaluation configuration context".to_string())?,
    })
}

pub(super) fn child_environment(cwd: &Path, config: &Path) -> Result<String, String> {
    let current = current_context(cwd, config)?;
    let inherited = match std::env::var(CHAIN_ENV) {
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => return Err("invalid inherited Epic eval context chain".into()),
    };
    append_context(current, inherited.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(project: &str, config: &str) -> EvaluationContext {
        EvaluationContext {
            project: project.into(),
            config: config.into(),
        }
    }

    #[test]
    fn context_cycles_are_rejected_without_blocking_distinct_projects_or_configs() {
        let first = append_context(context("project a", "config a"), None).unwrap();
        assert!(append_context(context("project a", "config a"), Some(&first)).is_err());
        assert!(append_context(context("project a", "config b"), Some(&first)).is_ok());
        let second = append_context(context("project b", "config b"), Some(&first)).unwrap();
        assert!(append_context(context("project a", "config a"), Some(&second)).is_err());
    }

    #[test]
    fn malformed_inherited_context_is_an_error() {
        for input in ["{broken", "null", "[{}]", r#"["project"]"#] {
            assert!(append_context(context("project", "config"), Some(input)).is_err());
        }
    }

    #[test]
    fn path_aliases_resolve_to_the_same_evaluation_context() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project with spaces");
        std::fs::create_dir_all(project.join("sub")).unwrap();
        let config = temp.path().join("eval.yaml");
        std::fs::write(&config, "{}").unwrap();
        let first = append_context(current_context(&project, &config).unwrap(), None).unwrap();
        let alias =
            current_context(&project.join("sub/.."), &temp.path().join("./eval.yaml")).unwrap();
        assert!(append_context(alias, Some(&first)).is_err());
    }

    #[test]
    fn sibling_workloads_receive_a_stable_chain_without_global_mutation() {
        let parent = append_context(context("parent", "parent config"), None).unwrap();
        let first = append_context(context("child", "child config"), Some(&parent)).unwrap();
        let second = append_context(context("child", "child config"), Some(&parent)).unwrap();
        assert_eq!(first, second);
    }
}
