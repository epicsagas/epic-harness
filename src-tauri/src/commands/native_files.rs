//! Contained read-only sidecar access for native dashboard commands.

use serde_json::Value;
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

fn checked_file(project: &Path, name: &str) -> Result<PathBuf, String> {
    let path = project.join(name);
    if !path.exists() {
        return Ok(path);
    }
    let root = project.canonicalize().map_err(|e| e.to_string())?;
    let file = path.canonicalize().map_err(|e| e.to_string())?;
    if !file.starts_with(root) || !file.is_file() {
        return Err("sidecar escapes project directory".into());
    }
    Ok(file)
}

pub fn read_json(project: &Path, name: &str, default: Value) -> Result<Value, String> {
    let path = checked_file(project, name)?;
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("invalid {name}: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(default),
        Err(e) => Err(format!("read {name}: {e}")),
    }
}

pub fn manifests(project: &Path, limit: usize) -> Result<Vec<Value>, String> {
    read_jsonl(project, "manifests.jsonl", limit)
}

pub fn read_jsonl(project: &Path, name: &str, limit: usize) -> Result<Vec<Value>, String> {
    let path = checked_file(project, name)?;
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    let mut records = VecDeque::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if let Ok(value) = serde_json::from_str(&line) {
            records.push_back(value);
            if records.len() > limit {
                records.pop_front();
            }
        }
    }
    Ok(records.into_iter().rev().collect())
}
