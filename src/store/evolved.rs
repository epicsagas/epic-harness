//! evolved.rs — bounded, project-scoped evolved-skill filesystem reads

use sqlx::AnyPool;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_EVOLVED_SKILLS: usize = 200;
const MAX_EVOLVED_TRAVERSAL_ENTRIES: usize = 512;
const MAX_EVOLVED_META_BYTES: usize = 32 * 1024;
const MAX_EVOLVED_SKILL_BYTES: usize = 256 * 1024;

/// Evolved skill metadata (mirrors evolve::skills::SkillMeta).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvolvedSkillRow {
    pub name: String,
    pub origin: String,
    pub confidence: f64,
    pub project: String,
    pub skill_md: String,
    pub active: bool,
    pub created: String,
    pub updated: String,
}

#[derive(serde::Deserialize)]
struct SkillMeta {
    name: String,
    origin: String,
    confidence: f64,
    project: String,
    created: String,
    updated: String,
    active: bool,
}

fn project_directories(
    projects_root: &Path,
    selected_project: Option<&str>,
) -> io::Result<Vec<(String, PathBuf)>> {
    if selected_project.is_some_and(|project| project.is_empty() || project == "__all__") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a selected project must be a concrete project slug",
        ));
    }
    if !projects_root.is_dir() {
        return if selected_project.is_some() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "harness projects directory does not exist",
            ))
        } else {
            Ok(Vec::new())
        };
    }
    if let Some(selected) = selected_project {
        let mut components = Path::new(selected).components();
        if !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a selected project must be one path component",
            ));
        }
        let path = projects_root.join(selected);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("unknown harness project slug: {selected}"),
                )
            } else {
                error
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("project directory must be regular: {selected}"),
            ));
        }
        return Ok(vec![(selected.to_string(), path)]);
    }

    let mut projects = Vec::new();
    for (index, entry) in fs::read_dir(projects_root)?.enumerate() {
        if index == MAX_EVOLVED_TRAVERSAL_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "project traversal limit of {MAX_EVOLVED_TRAVERSAL_ENTRIES} entries exceeded"
                ),
            ));
        }
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("project directory must not be a symlink: {name}"),
            ));
        }
        if metadata.is_dir() {
            projects.push((name, entry.path()));
        }
    }
    projects.sort_by(|left, right| left.0.cmp(&right.0));

    Ok(projects)
}

fn regular_file(path: &Path, label: &str) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} is not a regular file: {}", path.display()),
        ));
    }
    Ok(())
}

fn read_regular_utf8_bounded(path: &Path, label: &str, max_bytes: usize) -> io::Result<String> {
    regular_file(path, label)?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.len() > max_bytes as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} exceeds {max_bytes} byte content limit"),
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} exceeds {max_bytes} byte content limit"),
        ));
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn list_skills_scoped_at(
    projects_root: &Path,
    project: Option<&str>,
    limit: usize,
) -> io::Result<Vec<EvolvedSkillRow>> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let projects = project_directories(projects_root, project)?;
    let mut skills = Vec::new();
    for (project_name, project_dir) in projects {
        let evolved_dir = project_dir.join("evolved");
        let metadata = match fs::symlink_metadata(&evolved_dir) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "evolved-skill path is not a regular directory: {}",
                    evolved_dir.display()
                ),
            ));
        }

        let mut entries = Vec::new();
        for (index, entry) in fs::read_dir(&evolved_dir)?.enumerate() {
            if index == MAX_EVOLVED_TRAVERSAL_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "evolved-skill traversal limit of {MAX_EVOLVED_TRAVERSAL_ENTRIES} entries exceeded"
                    ),
                ));
            }
            entries.push(entry?);
        }
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "evolved-skill directory must not be a symlink: {}",
                        entry.path().display()
                    ),
                ));
            }
            if !metadata.is_dir() {
                continue;
            }
            let Some(directory_name) = entry.file_name().to_str().map(str::to_owned) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "evolved-skill directory name is not valid UTF-8",
                ));
            };
            let meta_path = entry.path().join("meta.json");
            let skill_path = entry.path().join("SKILL.md");

            let meta_json = read_regular_utf8_bounded(
                &meta_path,
                "evolved-skill metadata",
                MAX_EVOLVED_META_BYTES,
            )?;
            let meta: SkillMeta = serde_json::from_str(&meta_json)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if meta.name != directory_name || meta.project != project_name {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "evolved-skill ownership mismatch in {}",
                        entry.path().display()
                    ),
                ));
            }
            skills.push(EvolvedSkillRow {
                name: meta.name,
                origin: meta.origin,
                confidence: meta.confidence,
                project: meta.project,
                skill_md: read_regular_utf8_bounded(
                    &skill_path,
                    "evolved-skill body",
                    MAX_EVOLVED_SKILL_BYTES,
                )?,
                active: meta.active,
                created: meta.created,
                updated: meta.updated,
            });
            if skills.len() == limit {
                return Ok(skills);
            }
        }
    }
    Ok(skills)
}

/// Load full evolved-skill data from known project directories.
///
/// The pool parameter remains for dashboard API compatibility. Evolved skills
/// are file-owned and are not read from SQLite.
pub async fn list_skills_full_scoped_pool(
    _pool: &AnyPool,
    project: Option<&str>,
) -> io::Result<Vec<EvolvedSkillRow>> {
    list_skills_scoped_at(
        &crate::shared::paths::harness_projects_root(),
        project,
        MAX_EVOLVED_SKILLS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, project: &str, name: &str) {
        let skill_dir = root.join(project).join("evolved").join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("meta.json"),
            serde_json::json!({
                "name": name,
                "origin": "pattern",
                "confidence": 0.8,
                "project": project,
                "created": "2026-06-02T10:00:00Z",
                "updated": "2026-06-02T11:00:00Z",
                "active": true
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\n---\n\n## Process\nDo things."),
        )
        .unwrap();
    }

    #[test]
    fn selected_project_returns_only_its_skills() {
        let root = tempfile::tempdir().unwrap();
        write_skill(root.path(), "project-a", "same");
        write_skill(root.path(), "project-b", "same");

        let skills = list_skills_scoped_at(root.path(), Some("project-a"), 10).unwrap();

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].project, "project-a");
        assert_eq!(skills[0].name, "same");
    }

    #[test]
    fn same_skill_name_coexists_across_projects() {
        let root = tempfile::tempdir().unwrap();
        write_skill(root.path(), "project-a", "same");
        write_skill(root.path(), "project-b", "same");

        let skills = list_skills_scoped_at(root.path(), None, 10).unwrap();

        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "same");
        assert_eq!(skills[1].name, "same");
        assert_ne!(skills[0].project, skills[1].project);
    }

    #[test]
    fn aggregate_limit_is_enforced() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..5 {
            write_skill(root.path(), "project-a", &format!("skill-{index}"));
        }

        let skills = list_skills_scoped_at(root.path(), None, 3).unwrap();

        assert_eq!(skills.len(), 3);
    }

    #[test]
    fn oversized_skill_body_is_rejected_before_full_read() {
        let root = tempfile::tempdir().unwrap();
        write_skill(root.path(), "project-a", "large");
        fs::write(
            root.path().join("project-a/evolved/large/SKILL.md"),
            vec![b'x'; 256 * 1024 + 1],
        )
        .unwrap();

        let error = list_skills_scoped_at(root.path(), Some("project-a"), 10).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("content limit"));
    }

    #[test]
    fn oversized_skill_metadata_is_rejected_before_full_read() {
        let root = tempfile::tempdir().unwrap();
        write_skill(root.path(), "project-a", "large-meta");
        let path = root.path().join("project-a/evolved/large-meta/meta.json");
        let mut metadata = fs::read_to_string(&path).unwrap();
        metadata.push_str(&" ".repeat(32 * 1024));
        fs::write(path, metadata).unwrap();

        let error = list_skills_scoped_at(root.path(), Some("project-a"), 10).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("content limit"));
    }

    #[test]
    fn evolved_skill_directory_traversal_is_bounded_before_collection() {
        let root = tempfile::tempdir().unwrap();
        let evolved = root.path().join("project-a/evolved");
        for index in 0..=512 {
            fs::create_dir_all(evolved.join(format!("skill-{index:04}"))).unwrap();
        }

        let error = list_skills_scoped_at(root.path(), Some("project-a"), 1).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("traversal limit"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_skill_directory_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let foreign = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("project-a/evolved")).unwrap();
        write_skill(foreign.path(), "foreign", "linked");
        symlink(
            foreign.path().join("foreign/evolved/linked"),
            root.path().join("project-a/evolved/linked"),
        )
        .unwrap();

        assert!(list_skills_scoped_at(root.path(), Some("project-a"), 10).is_err());
    }
}
