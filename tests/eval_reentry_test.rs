//! All subprocess recursion has an immutable depth fence in this test fixture.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    outer: PathBuf,
    inner: PathBuf,
    outer_data: PathBuf,
    inner_data: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let outer = root.join("a/project");
        let inner = root.join("b/project");
        let outer_data = root.join("a/data");
        let inner_data = root.join("b/data");
        for directory in [
            &outer,
            &inner,
            &outer_data,
            &inner_data,
            &root.join("home/.config/epic-harness"),
        ] {
            fs::create_dir_all(directory).unwrap();
        }
        fs::create_dir(outer.join("sub")).unwrap();
        fs::write(
            root.join("home/.config/epic-harness/telemetry-consent"),
            "off",
        )
        .unwrap();
        fs::write(root.join("home/gitconfig"), "").unwrap();
        for data in [&outer_data, &inner_data] {
            fs::write(
                data.join("config.toml"),
                "[dashboard]\nport=0\nauto_open=false\n",
            )
            .unwrap();
        }
        for project in [&outer, &inner] {
            let status = Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(project)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", root.join("home/gitconfig"))
                .status()
                .unwrap();
            assert!(status.success());
        }
        Self {
            _temp: temp,
            root,
            outer,
            inner,
            outer_data,
            inner_data,
        }
    }

    fn command(&self) -> String {
        format!(
            "\"{}\" --exact fixture_command --nocapture",
            std::env::current_exe().unwrap().display()
        )
    }

    fn config_path(&self, data: &Path) -> PathBuf {
        data.join("projects/project/eval/eval.yaml")
    }

    fn configure(&self) {
        let yaml = format!(
            "benchmarks:\n  - name: controlled_workload\n    command: {}\ndimensions:\n  regression:\n    enabled: false\n",
            serde_json::to_string(&self.command()).unwrap()
        );
        for data in [&self.outer_data, &self.inner_data] {
            let path = self.config_path(data);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, &yaml).unwrap();
        }
    }

    fn run(&self, case: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_epic-harness"))
            .args(args)
            .current_dir(&self.outer)
            .env("HOME", self.root.join("home"))
            .env("USERPROFILE", self.root.join("home"))
            .env("XDG_CONFIG_HOME", self.root.join("home/.config"))
            .env("HARNESS_DIR", &self.outer_data)
            .env("HARNESS_ROOT", &self.root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.root.join("home/gitconfig"))
            .env("EPIC_EVAL_REENTRY_CASE", case)
            .env("EPIC_EVAL_REENTRY_DEPTH", "0")
            .env("EPIC_EVAL_REENTRY_TRACE", self.root.join("trace.txt"))
            .env("EPIC_EVAL_REENTRY_OUTER", &self.outer)
            .env("EPIC_EVAL_REENTRY_INNER", &self.inner)
            .env("EPIC_EVAL_REENTRY_OUTER_DATA", &self.outer_data)
            .env("EPIC_EVAL_REENTRY_INNER_DATA", &self.inner_data)
            .env_remove("EPIC_EVAL_CONTEXT_CHAIN")
            .env_remove("HARNESS_DB_URL")
            .env_remove("HARNESS_MEM_URL")
            .env_remove("DATABASE_URL")
            .output()
            .unwrap()
    }

    fn trace(&self) -> String {
        fs::read_to_string(self.root.join("trace.txt")).unwrap_or_default()
    }
}

#[test]
fn fixture_command() {
    let Ok(case) = std::env::var("EPIC_EVAL_REENTRY_CASE") else {
        return;
    };
    let depth: u32 = std::env::var("EPIC_EVAL_REENTRY_DEPTH")
        .unwrap()
        .parse()
        .unwrap();
    let trace = std::env::var_os("EPIC_EVAL_REENTRY_TRACE").unwrap();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(trace)
        .unwrap();
    writeln!(
        file,
        "{depth}|{case}|{}",
        std::env::current_dir().unwrap().display()
    )
    .unwrap();
    drop(file);
    if depth >= 3 {
        eprintln!("IMMUTABLE_TEST_DEPTH_LIMIT");
        std::process::exit(86);
    }
    if case == "leaf" {
        println!("MEASURED_INDEPENDENT_WORKLOAD");
        std::process::exit(0);
    }
    let outer = PathBuf::from(std::env::var_os("EPIC_EVAL_REENTRY_OUTER").unwrap());
    let inner = PathBuf::from(std::env::var_os("EPIC_EVAL_REENTRY_INNER").unwrap());
    let outer_data = PathBuf::from(std::env::var_os("EPIC_EVAL_REENTRY_OUTER_DATA").unwrap());
    let inner_data = PathBuf::from(std::env::var_os("EPIC_EVAL_REENTRY_INNER_DATA").unwrap());
    let (target, data, next_case) = match case.as_str() {
        "same" => (outer.join("sub"), outer_data, "same"),
        "independent" => (inner, inner_data, "leaf"),
        "independent_shared_config" => (inner, outer_data, "leaf"),
        "independent_config" => (outer, inner_data, "leaf"),
        "cross"
            if std::env::current_dir().unwrap().canonicalize().unwrap()
                == outer.canonicalize().unwrap() =>
        {
            (inner, inner_data, "cross")
        }
        "cross" => (outer, outer_data, "cross"),
        _ => panic!("unknown controlled case"),
    };
    let child = Command::new(env!("CARGO_BIN_EXE_epic-harness"))
        .args(["eval", "--json"])
        .current_dir(target)
        .env("HARNESS_DIR", data)
        .env("EPIC_EVAL_REENTRY_CASE", next_case)
        .env("EPIC_EVAL_REENTRY_DEPTH", (depth + 1).to_string())
        .output()
        .unwrap();
    print!("{}", String::from_utf8_lossy(&child.stdout));
    eprint!("{}", String::from_utf8_lossy(&child.stderr));
    std::process::exit(child.status.code().unwrap_or(1));
}

#[test]
fn initialization_excludes_direct_make_and_just_eval_wrappers() {
    let f = Fixture::new();
    fs::write(
        f.outer.join("Makefile"),
        "eval:\n\t@echo start\n\t@epic-harness eval --json\n",
    )
    .unwrap();
    fs::write(
        f.outer.join("justfile"),
        "eval:\n    @\"C:\\Program Files\\Epic\\epic.exe\" eval --json\n",
    )
    .unwrap();
    let output = f.run("unused", &["eval", "--init"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let yaml = fs::read_to_string(f.config_path(&f.outer_data)).unwrap();
    let value: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    assert!(
        value["benchmarks"].as_sequence().unwrap().is_empty(),
        "self wrappers discovered: {yaml}"
    );
}

#[test]
fn initialization_preserves_real_make_and_just_workloads() {
    let f = Fixture::new();
    fs::write(f.outer.join("Makefile"), "eval:\n\t@echo 'epic eval'\n\t@epic eval --init\n\t@python benchmark.py\nother:\n\t@epic eval --json\n").unwrap();
    fs::write(f.outer.join("justfile"), "eval:\n    cargo bench\n").unwrap();
    assert!(f.run("unused", &["eval", "--init"]).status.success());
    let yaml = fs::read_to_string(f.config_path(&f.outer_data)).unwrap();
    let value: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    let names: Vec<_> = value["benchmarks"]
        .as_sequence()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["make_eval", "just_eval"]);
}

#[test]
fn same_project_reentry_from_a_subdirectory_stops_before_depth_fence() {
    let f = Fixture::new();
    f.configure();
    let output = f.run("same", &["eval", "--json"]);
    assert!(!output.status.success());
    assert_eq!(
        f.trace().lines().count(),
        1,
        "recursive workloads ran: {}",
        f.trace()
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("recursive eval"));
}

#[test]
fn independent_same_named_project_evaluation_remains_supported() {
    let f = Fixture::new();
    f.configure();
    let output = f.run("independent", &["eval", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(f.trace().lines().count(), 2);
    assert!(f.trace().contains("1|leaf|"));
}

#[test]
fn independent_project_using_the_same_config_remains_supported() {
    let f = Fixture::new();
    f.configure();
    let output = f.run("independent_shared_config", &["eval", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(f.trace().lines().count(), 2);
    assert!(f.trace().contains("1|leaf|"));
}

#[test]
fn single_shell_recipes_may_evaluate_an_independent_project() {
    for (file, recipe) in [
        (
            "justfile",
            "eval:\n    #!/usr/bin/env bash\n    cd ../independent-project\n    epic eval --json\n",
        ),
        (
            "Makefile",
            ".ONESHELL:\neval:\n\tcd ../independent-project\n\tepic-harness eval --json\n",
        ),
    ] {
        let f = Fixture::new();
        fs::write(f.outer.join(file), recipe).unwrap();
        assert!(f.run("unused", &["eval", "--init"]).status.success());
        let yaml = fs::read_to_string(f.config_path(&f.outer_data)).unwrap();
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            value["benchmarks"].as_sequence().unwrap().len(),
            1,
            "independent script recipe suppressed: {yaml}"
        );
    }
}

#[test]
fn independent_configuration_in_the_same_project_remains_supported() {
    let f = Fixture::new();
    f.configure();
    let output = f.run("independent_config", &["eval", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(f.trace().lines().count(), 2);
    assert!(f.trace().contains("1|leaf|"));
}

#[test]
fn cross_project_cycle_stops_when_the_first_context_reappears() {
    let f = Fixture::new();
    f.configure();
    let output = f.run("cross", &["eval", "--json"]);
    assert!(!output.status.success());
    assert_eq!(
        f.trace().lines().count(),
        2,
        "cross-project cycle ran: {}",
        f.trace()
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("recursive eval"));
}
