//! evolved.rs — Evolved skills SQLite I/O
#![allow(dead_code)]

use sqlx::AnyPool;
use sqlx::Row;
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
pub async fn upsert_skill_pool(pool: &AnyPool, skill: &EvolvedSkillRow) -> io::Result<()> {
    let active_int = if skill.active { 1 } else { 0 };
    sqlx::query(
        "INSERT OR REPLACE INTO evolved_skills
         (name, origin, confidence, project, skill_md, active, created, updated)
         VALUES (?, ?, ?, ?, ?, ?,
                 COALESCE((SELECT created FROM evolved_skills WHERE name = ?), ?),
                 ?)",
    )
    .bind(&skill.name)
    .bind(&skill.origin)
    .bind(skill.confidence)
    .bind(&skill.project)
    .bind(&skill.skill_md)
    .bind(active_int)
    .bind(&skill.name)
    .bind(&skill.created)
    .bind(&skill.updated)
    .execute(pool)
    .await
    .map_err(super::sqlx_err)?;
    Ok(())
}

/// List all evolved skills (metadata only, no skill_md body).
pub async fn list_skills_pool(pool: &AnyPool) -> io::Result<Vec<EvolvedSkillRow>> {
    let rows = sqlx::query(
        "SELECT name, origin, confidence, project, active, created, updated
         FROM evolved_skills ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut skills = Vec::with_capacity(rows.len());
    for r in rows {
        let active_i32: i32 = r.try_get(4).map_err(super::sqlx_err)?;
        skills.push(EvolvedSkillRow {
            name: r.try_get(0).map_err(super::sqlx_err)?,
            origin: r.try_get(1).map_err(super::sqlx_err)?,
            confidence: r.try_get(2).map_err(super::sqlx_err)?,
            project: r.try_get(3).map_err(super::sqlx_err)?,
            skill_md: String::new(),
            active: active_i32 != 0,
            created: r.try_get(5).map_err(super::sqlx_err)?,
            updated: r.try_get(6).map_err(super::sqlx_err)?,
        });
    }
    Ok(skills)
}

/// List all evolved skills including the full skill_md body.
pub async fn list_skills_full_pool(pool: &AnyPool) -> io::Result<Vec<EvolvedSkillRow>> {
    let rows = sqlx::query(
        "SELECT name, origin, confidence, project, skill_md, active, created, updated
         FROM evolved_skills ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .map_err(super::sqlx_err)?;

    let mut skills = Vec::with_capacity(rows.len());
    for r in rows {
        let active_i32: i32 = r.try_get(5).map_err(super::sqlx_err)?;
        skills.push(EvolvedSkillRow {
            name: r.try_get(0).map_err(super::sqlx_err)?,
            origin: r.try_get(1).map_err(super::sqlx_err)?,
            confidence: r.try_get(2).map_err(super::sqlx_err)?,
            project: r.try_get(3).map_err(super::sqlx_err)?,
            skill_md: r.try_get(4).map_err(super::sqlx_err)?,
            active: active_i32 != 0,
            created: r.try_get(6).map_err(super::sqlx_err)?,
            updated: r.try_get(7).map_err(super::sqlx_err)?,
        });
    }
    Ok(skills)
}

/// Read a single skill's markdown content.
pub async fn read_skill_md_pool(pool: &AnyPool, name: &str) -> io::Result<Option<String>> {
    let row = sqlx::query("SELECT skill_md FROM evolved_skills WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(super::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(r.try_get::<String, _>(0).map_err(super::sqlx_err)?)),
        None => Ok(None),
    }
}

/// Delete an evolved skill by name.
pub async fn delete_skill_pool(pool: &AnyPool, name: &str) -> io::Result<bool> {
    let result = sqlx::query("DELETE FROM evolved_skills WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

/// Count active evolved skills.
pub async fn count_active_skills_pool(pool: &AnyPool) -> io::Result<usize> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evolved_skills WHERE active = 1")
        .fetch_one(pool)
        .await
        .map_err(super::sqlx_err)?;
    Ok(count as usize)
}

// ── Promotion counters ───────────────────────────────

/// Load all promotion counters.
pub async fn load_promotion_counters_pool(pool: &AnyPool) -> io::Result<HashMap<String, u64>> {
    let rows = sqlx::query("SELECT pattern_key, count FROM promotion_counters")
        .fetch_all(pool)
        .await
        .map_err(super::sqlx_err)?;

    let mut counters = HashMap::with_capacity(rows.len());
    for r in rows {
        let key: String = r.try_get(0).map_err(super::sqlx_err)?;
        let count: i64 = r.try_get(1).map_err(super::sqlx_err)?;
        counters.insert(key, count as u64);
    }
    Ok(counters)
}

/// Save all promotion counters (replaces entire table).
pub async fn save_promotion_counters_pool(
    pool: &AnyPool,
    counters: &HashMap<String, u64>,
) -> io::Result<()> {
    let mut tx = pool.begin().await.map_err(super::sqlx_err)?;

    sqlx::query("DELETE FROM promotion_counters")
        .execute(&mut *tx)
        .await
        .map_err(super::sqlx_err)?;

    for (key, count) in counters {
        sqlx::query("INSERT INTO promotion_counters (pattern_key, count) VALUES (?, ?)")
            .bind(key)
            .bind(super::u64_to_i64(*count))
            .execute(&mut *tx)
            .await
            .map_err(super::sqlx_err)?;
    }

    tx.commit().await.map_err(super::sqlx_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> AnyPool {
        let pool = super::super::pool::test_memory_pool().await;
        super::super::schema::init_schema_pool(&pool).await.unwrap();
        pool
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

    #[tokio::test]
    async fn upsert_list_and_read() {
        let pool = test_pool().await;
        upsert_skill_pool(&pool, &sample_skill("rust-ownership")).await.unwrap();
        upsert_skill_pool(&pool, &sample_skill("ts-async")).await.unwrap();

        let skills = list_skills_pool(&pool).await.unwrap();
        assert_eq!(skills.len(), 2);

        let md = read_skill_md_pool(&pool, "rust-ownership").await.unwrap();
        assert!(md.is_some());
        assert!(md.unwrap().contains("## Process"));
    }

    #[tokio::test]
    async fn delete_skill() {
        let pool = test_pool().await;
        upsert_skill_pool(&pool, &sample_skill("to-delete")).await.unwrap();

        let deleted = delete_skill_pool(&pool, "to-delete").await.unwrap();
        assert!(deleted);

        let skills = list_skills_pool(&pool).await.unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn promotion_counters() {
        let pool = test_pool().await;
        let mut counters = HashMap::new();
        counters.insert("pattern_a".into(), 5);
        counters.insert("pattern_b".into(), 3);

        save_promotion_counters_pool(&pool, &counters).await.unwrap();
        let loaded = load_promotion_counters_pool(&pool).await.unwrap();
        assert_eq!(loaded.get("pattern_a"), Some(&5));
        assert_eq!(loaded.len(), 2);
    }
}
