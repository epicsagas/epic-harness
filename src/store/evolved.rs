//! evolved.rs — Evolved skills SQLite I/O
#![allow(dead_code)]

use rusqlite::Connection;
use std::collections::HashMap;
use std::io;

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

/// Insert or update an evolved skill.
pub fn upsert_skill_conn(conn: &Connection, skill: &EvolvedSkillRow) -> io::Result<()> {
    let active_int = if skill.active { 1 } else { 0 };
    conn.execute(
        "INSERT OR REPLACE INTO evolved_skills
         (name, origin, confidence, project, skill_md, active, created, updated)
         VALUES (?1,?2,?3,?4,?5,?6,
                 COALESCE((SELECT created FROM evolved_skills WHERE name = ?1), ?7),
                 ?8)",
        rusqlite::params![
            skill.name,
            skill.origin,
            skill.confidence,
            skill.project,
            skill.skill_md,
            active_int,
            skill.created,
            skill.updated,
        ],
    )
    .map_err(io::Error::other)?;
    Ok(())
}

/// List all evolved skills (metadata only, no skill_md body).
pub fn list_skills_conn(conn: &Connection) -> io::Result<Vec<EvolvedSkillRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT name, origin, confidence, project, active, created, updated
             FROM evolved_skills ORDER BY name",
        )
        .map_err(io::Error::other)?;

    let rows = stmt
        .query_map([], |row| {
            Ok(EvolvedSkillRow {
                name: row.get(0)?,
                origin: row.get(1)?,
                confidence: row.get(2)?,
                project: row.get(3)?,
                skill_md: String::new(),
                active: row.get::<_, i32>(4)? != 0,
                created: row.get(5)?,
                updated: row.get(6)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut skills = Vec::new();
    for r in rows {
        skills.push(r.map_err(io::Error::other)?);
    }
    Ok(skills)
}

/// List all evolved skills including the full skill_md body.
pub fn list_skills_full_conn(conn: &Connection) -> io::Result<Vec<EvolvedSkillRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT name, origin, confidence, project, skill_md, active, created, updated
             FROM evolved_skills ORDER BY name",
        )
        .map_err(io::Error::other)?;

    let rows = stmt
        .query_map([], |row| {
            Ok(EvolvedSkillRow {
                name: row.get(0)?,
                origin: row.get(1)?,
                confidence: row.get(2)?,
                project: row.get(3)?,
                skill_md: row.get(4)?,
                active: row.get::<_, i32>(5)? != 0,
                created: row.get(6)?,
                updated: row.get(7)?,
            })
        })
        .map_err(io::Error::other)?;

    let mut skills = Vec::new();
    for r in rows {
        skills.push(r.map_err(io::Error::other)?);
    }
    Ok(skills)
}

/// Read a single skill's markdown content.
pub fn read_skill_md_conn(conn: &Connection, name: &str) -> io::Result<Option<String>> {
    match conn.query_row(
        "SELECT skill_md FROM evolved_skills WHERE name = ?1",
        rusqlite::params![name],
        |row| row.get::<_, String>(0),
    ) {
        Ok(md) => Ok(Some(md)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(io::Error::other(e)),
    }
}

/// Delete an evolved skill by name.
pub fn delete_skill_conn(conn: &Connection, name: &str) -> io::Result<bool> {
    let count = conn
        .execute(
            "DELETE FROM evolved_skills WHERE name = ?1",
            rusqlite::params![name],
        )
        .map_err(io::Error::other)?;
    Ok(count > 0)
}

/// Count active evolved skills.
pub fn count_active_skills_conn(conn: &Connection) -> io::Result<usize> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolved_skills WHERE active = 1",
            [],
            |row| row.get(0),
        )
        .map_err(io::Error::other)?;
    Ok(count as usize)
}

// ── Promotion counters ───────────────────────────────

/// Load all promotion counters.
pub fn load_promotion_counters_conn(conn: &Connection) -> io::Result<HashMap<String, u64>> {
    let mut stmt = conn
        .prepare("SELECT pattern_key, count FROM promotion_counters")
        .map_err(io::Error::other)?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64)) // i64→u64 safe: always non-negative
        })
        .map_err(io::Error::other)?;

    let mut counters = HashMap::new();
    for r in rows {
        let (k, v) = r.map_err(io::Error::other)?;
        counters.insert(k, v);
    }
    Ok(counters)
}

/// Save all promotion counters (replaces entire table).
pub fn save_promotion_counters_conn(
    conn: &Connection,
    counters: &HashMap<String, u64>,
) -> io::Result<()> {
    let tx = conn.unchecked_transaction().map_err(io::Error::other)?;
    tx.execute("DELETE FROM promotion_counters", [])
        .map_err(io::Error::other)?;
    for (key, count) in counters {
        tx.execute(
            "INSERT INTO promotion_counters (pattern_key, count) VALUES (?1, ?2)",
            rusqlite::params![key, super::u64_to_i64(*count)],
        )
        .map_err(io::Error::other)?;
    }
    tx.commit().map_err(io::Error::other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::schema::init_schema(&conn).unwrap();
        conn
    }

    fn sample_skill(name: &str) -> EvolvedSkillRow {
        EvolvedSkillRow {
            name: name.into(),
            origin: "pattern".into(),
            confidence: 0.8,
            project: "test-project".into(),
            skill_md: "---\nname: test\n---\n## Process\nDo things.".into(),
            active: true,
            created: "2026-06-02T10:00:00Z".into(),
            updated: "2026-06-02T10:00:00Z".into(),
        }
    }

    #[test]
    fn upsert_list_and_read() {
        let conn = in_memory_db();
        upsert_skill_conn(&conn, &sample_skill("rust-ownership")).unwrap();
        upsert_skill_conn(&conn, &sample_skill("ts-async")).unwrap();

        let skills = list_skills_conn(&conn).unwrap();
        assert_eq!(skills.len(), 2);

        let md = read_skill_md_conn(&conn, "rust-ownership").unwrap();
        assert!(md.is_some());
        assert!(md.unwrap().contains("## Process"));
    }

    #[test]
    fn delete_skill() {
        let conn = in_memory_db();
        upsert_skill_conn(&conn, &sample_skill("to-delete")).unwrap();

        let deleted = delete_skill_conn(&conn, "to-delete").unwrap();
        assert!(deleted);

        let skills = list_skills_conn(&conn).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn promotion_counters() {
        let conn = in_memory_db();
        let mut counters = HashMap::new();
        counters.insert("pattern_a".into(), 5);
        counters.insert("pattern_b".into(), 3);

        save_promotion_counters_conn(&conn, &counters).unwrap();
        let loaded = load_promotion_counters_conn(&conn).unwrap();
        assert_eq!(loaded.get("pattern_a"), Some(&5));
        assert_eq!(loaded.len(), 2);
    }
}
