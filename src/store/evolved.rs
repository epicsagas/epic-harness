//! evolved.rs — Evolved skills SQLite I/O (async pool)

#[cfg(test)]
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

// ── Async pool functions ─────────────────────────────

use sqlx::{AnyPool, Row};

#[cfg(test)]
pub async fn upsert_skill_pool(pool: &AnyPool, skill: &EvolvedSkillRow) -> io::Result<()> {
    let active_int = if skill.active { 1 } else { 0 };
    sqlx::query(
        "INSERT INTO evolved_skills (name, origin, confidence, project, skill_md, active, created, updated) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (name) DO UPDATE SET origin=excluded.origin, confidence=excluded.confidence, project=excluded.project, skill_md=excluded.skill_md, active=excluded.active, created=evolved_skills.created, updated=excluded.updated"
    )
    .bind(&skill.name)
    .bind(&skill.origin)
    .bind(skill.confidence)
    .bind(&skill.project)
    .bind(&skill.skill_md)
    .bind(active_int)
    .bind(&skill.created)
    .bind(&skill.updated)
    .execute(pool)
    .await
    .map_err(crate::store::sqlx_err)?;
    Ok(())
}

#[cfg(test)]
pub async fn list_skills_pool(pool: &AnyPool) -> io::Result<Vec<EvolvedSkillRow>> {
    list_skills_pool_inner(pool, false).await
}

pub async fn list_skills_full_pool(pool: &AnyPool) -> io::Result<Vec<EvolvedSkillRow>> {
    list_skills_pool_inner(pool, true).await
}

async fn list_skills_pool_inner(
    pool: &AnyPool,
    include_body: bool,
) -> io::Result<Vec<EvolvedSkillRow>> {
    let rows = if include_body {
        sqlx::query(
            "SELECT name, origin, confidence, project, skill_md, active, created, updated FROM evolved_skills ORDER BY name"
        )
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT name, origin, confidence, project, active, created, updated FROM evolved_skills ORDER BY name"
        )
        .fetch_all(pool)
        .await
    }
    .map_err(crate::store::sqlx_err)?;

    let skills: Vec<EvolvedSkillRow> = rows
        .iter()
        .map(|r| {
            let (skill_md, active_col, created_col, updated_col) = if include_body {
                (
                    r.try_get::<String, _>(4).unwrap_or_default(),
                    5usize,
                    6usize,
                    7usize,
                )
            } else {
                (String::new(), 4usize, 5usize, 6usize)
            };
            EvolvedSkillRow {
                name: r.try_get(0).unwrap_or_default(),
                origin: r.try_get(1).unwrap_or_default(),
                confidence: r.try_get(2).unwrap_or(0.0),
                project: r.try_get(3).unwrap_or_default(),
                skill_md,
                active: r.try_get::<i32, _>(active_col).unwrap_or(0) != 0,
                created: r.try_get(created_col).unwrap_or_default(),
                updated: r.try_get(updated_col).unwrap_or_default(),
            }
        })
        .collect();
    Ok(skills)
}

#[cfg(test)]
pub async fn read_skill_md_pool(pool: &AnyPool, name: &str) -> io::Result<Option<String>> {
    let row = sqlx::query("SELECT skill_md FROM evolved_skills WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(crate::store::sqlx_err)?;

    match row {
        Some(r) => Ok(Some(r.try_get(0).map_err(crate::store::sqlx_err)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
pub async fn delete_skill_pool(pool: &AnyPool, name: &str) -> io::Result<bool> {
    let result = sqlx::query("DELETE FROM evolved_skills WHERE name = $1")
        .bind(name)
        .execute(pool)
        .await
        .map_err(crate::store::sqlx_err)?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
pub async fn load_promotion_counters_pool(
    pool: &AnyPool,
    project: &str,
) -> io::Result<HashMap<String, u64>> {
    let rows = sqlx::query("SELECT pattern_key, count FROM promotion_counters WHERE project = $1")
        .bind(project)
        .fetch_all(pool)
        .await
        .map_err(crate::store::sqlx_err)?;

    let mut counters = HashMap::new();
    for r in &rows {
        let key: String = r.try_get(0).map_err(crate::store::sqlx_err)?;
        let count: i64 = r.try_get(1).map_err(crate::store::sqlx_err)?;
        counters.insert(key, crate::store::i64_to_u64(count));
    }
    Ok(counters)
}

#[cfg(test)]
pub async fn save_promotion_counters_pool(
    pool: &AnyPool,
    project: &str,
    counters: &HashMap<String, u64>,
) -> io::Result<()> {
    let mut tx = pool.begin().await.map_err(crate::store::sqlx_err)?;
    sqlx::query("DELETE FROM promotion_counters WHERE project = $1")
        .bind(project)
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    for (key, count) in counters {
        sqlx::query(
            "INSERT INTO promotion_counters (pattern_key, project, count) VALUES ($1, $2, $3) ON CONFLICT (pattern_key, project) DO UPDATE SET count=excluded.count",
        )
        .bind(key)
        .bind(project)
        .bind(crate::store::u64_to_i64(*count))
        .execute(&mut *tx)
        .await
        .map_err(crate::store::sqlx_err)?;
    }
    tx.commit().await.map_err(crate::store::sqlx_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn in_memory_pool() -> sqlx::AnyPool {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::store::schema::init_schema_pool(&pool).await.unwrap();
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
        let pool = in_memory_pool().await;
        upsert_skill_pool(&pool, &sample_skill("rust-ownership"))
            .await
            .unwrap();
        upsert_skill_pool(&pool, &sample_skill("ts-async"))
            .await
            .unwrap();

        let skills = list_skills_pool(&pool).await.unwrap();
        assert_eq!(skills.len(), 2);

        let md = read_skill_md_pool(&pool, "rust-ownership").await.unwrap();
        assert!(md.is_some());
        assert!(md.unwrap().contains("## Process"));
    }

    #[tokio::test]
    async fn delete_skill() {
        let pool = in_memory_pool().await;
        upsert_skill_pool(&pool, &sample_skill("to-delete"))
            .await
            .unwrap();

        let deleted = delete_skill_pool(&pool, "to-delete").await.unwrap();
        assert!(deleted);

        let skills = list_skills_pool(&pool).await.unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn promotion_counters() {
        let pool = in_memory_pool().await;
        let mut counters = HashMap::new();
        counters.insert("pattern_a".into(), 5);
        counters.insert("pattern_b".into(), 3);

        save_promotion_counters_pool(&pool, "test-project", &counters)
            .await
            .unwrap();
        let loaded = load_promotion_counters_pool(&pool, "test-project")
            .await
            .unwrap();
        assert_eq!(loaded.get("pattern_a"), Some(&5));
        assert_eq!(loaded.len(), 2);
    }
}
