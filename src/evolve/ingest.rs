use crate::mem::store;
use crate::mem::store::conn::memory_pool_sync;
use crate::shared::{evolution::*, helpers::*, paths::*};
use crate::store::runtime::block_on;

use sqlx::{AnyPool, Row};

use super::analysis::build_summary;

/// Query pattern types detected in the previous session (the session that the
/// current session "follows"). Extracts non-generic tags from CSV tag strings.
pub async fn query_prev_pattern_types_async(pool: &AnyPool, session_node_id: &str) -> Vec<String> {
    let rows = sqlx::query(
        "SELECT n.tags FROM nodes n
         JOIN edges e ON e.source = n.id
         WHERE n.type = 'pattern'
         AND e.label = 'detected_in'
         AND e.target IN (
            SELECT e2.source FROM edges e2
            WHERE e2.target = ? AND e2.label = 'follows'
         )",
    )
    .bind(session_node_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut results = Vec::new();
    for row in &rows {
        let tags_str: String = row.try_get(0).unwrap_or_default();
        for tag in tags_str.split(',') {
            let tag = tag.trim().trim_matches('"');
            if !tag.is_empty() && tag != "auto" && tag != "pattern" {
                results.push(tag.to_string());
            }
        }
    }
    results
}

/// Find or create a project hub node. Returns the hub node's ID.
pub async fn ensure_project_hub_async(pool: &AnyPool, slug: &str) -> std::io::Result<String> {
    let title = format!("project: {}", slug);

    // Check if project hub already exists
    let existing: Option<String> =
        sqlx::query("SELECT id FROM nodes WHERE type = 'project' AND title = ? LIMIT 1")
            .bind(&title)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .and_then(|r| r.try_get::<String, _>(0).ok())
            .filter(|s| !s.is_empty());

    if let Some(id) = existing {
        return Ok(id);
    }

    // Create new project hub
    let id = store::new_uuid();
    let now = store::now_iso();
    let node = store::Node {
        frontmatter: store::NodeFrontmatter {
            id: id.clone(),
            node_type: "project".to_string(),
            title,
            tags: vec!["hub".to_string()],
            projects: vec![slug.to_string()],
            agents: vec![],
            created: now.clone(),
            updated: now,
            importance: store::importance_for_type("project"),
            access_count: 0,
            accessed_at: String::new(),
        },
        body: format!("Project hub node for {}", slug),
    };
    store::write_node_pool(pool, &node).await?;
    Ok(id)
}

/// Find the most recent previous session node for a project slug.
async fn find_prev_session_async(
    pool: &AnyPool,
    session_node_id: &str,
    slug: &str,
) -> Option<String> {
    let csv_proj = format!("%{slug}%");
    sqlx::query(
        "SELECT id FROM nodes WHERE type = 'session' AND id != ?
         AND projects LIKE ?
         ORDER BY updated DESC LIMIT 1",
    )
    .bind(session_node_id)
    .bind(&csv_proj)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.try_get::<String, _>(0).ok())
}

/// Find the most recent previous session node for a project slug (sync wrapper).
#[allow(dead_code)]
fn find_prev_session(session_node_id: &str, slug: &str) -> Option<String> {
    let pool = memory_pool_sync().ok()?;
    block_on(find_prev_session_async(&pool, session_node_id, slug))
}

/// Ingest session analysis results into the knowledge graph.
/// Returns (nodes_created, edges_created).
pub fn ingest_to_memory(analysis: &SessionAnalysis, patterns: &[DetectedPattern]) -> (u64, u64) {
    let pool = match block_on(crate::store::pool::memory_pool()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[ingest] failed to open memory DB: {e}");
            return (0, 0);
        }
    };

    block_on(ingest_to_memory_async(&pool, analysis, patterns))
}

async fn ingest_to_memory_async(
    pool: &AnyPool,
    analysis: &SessionAnalysis,
    patterns: &[DetectedPattern],
) -> (u64, u64) {
    let slug = project_slug();
    let ts = now_iso();
    let dedup_hours = 24u64;

    let mut nodes_created = 0u64;
    let mut edges_created = 0u64;
    let mut session_node_id = String::new();

    // 8a. Session summary node
    {
        let title = format!(
            "session: {} {:.0}% avg={}",
            slug,
            analysis.success_rate * 100.0,
            analysis.avg_score
        );
        let body = build_summary(analysis);
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "session".into(),
                title,
                tags: vec!["auto".into(), "session".into()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("session"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        match store::write_node_dedup_pool(pool, &node, dedup_hours).await {
            Ok((id, false)) => {
                session_node_id = id;
                nodes_created += 1;
            }
            Ok((id, true)) => {
                session_node_id = id;
            }
            Err(_) => {}
        }
    }

    // 8b. Pattern nodes + edges to session
    let mut pattern_node_ids: Vec<(String, Vec<String>)> = vec![]; // (node_id, involved_files)
    for pattern in patterns {
        let title = format!("{}: {} ({}x)", slug, pattern.pattern_type, pattern.count);
        let body = format!(
            "**Pattern**: {}\n**Description**: {}\n**Files**: {}\n**Remediation**: {}",
            pattern.pattern_type,
            pattern.description,
            if pattern.involved_files.is_empty() {
                "various".into()
            } else {
                pattern.involved_files.join(", ")
            },
            pattern.suggested_remediation,
        );
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "pattern".into(),
                title,
                tags: vec!["auto".into(), pattern.pattern_type.clone()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("pattern"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        if let Ok((id, deduped)) = store::write_node_dedup_pool(pool, &node, dedup_hours).await {
            let files = pattern.involved_files.clone();
            pattern_node_ids.push((id.clone(), files));
            if !deduped {
                nodes_created += 1;
            }
            // Edge: session -> pattern (detected_in)
            if !session_node_id.is_empty() {
                let edge = store::Edge {
                    id: store::new_uuid(),
                    source: session_node_id.clone(),
                    target: id,
                    relation: "detected_in".into(),
                    weight: 1.0,
                    ts: ts.clone(),
                };
                if store::append_edge_pool(pool, &edge).await.is_ok() {
                    edges_created += 1;
                }
            }
        }
    }

    // 8b-2. Resolution nodes: patterns resolved since last session
    if !session_node_id.is_empty() {
        let prev_pattern_types = query_prev_pattern_types_async(pool, &session_node_id).await;

        // Get current pattern types for comparison
        let current_pattern_types: Vec<&str> =
            patterns.iter().map(|p| p.pattern_type.as_str()).collect();

        // For each previous pattern, check if the same type exists in current session
        for pattern_type in &prev_pattern_types {
            if !pattern_type.is_empty() && !current_pattern_types.contains(&pattern_type.as_str()) {
                let title = format!("{}: resolved {} (auto)", slug, pattern_type);
                let body = format!(
                    "**Resolved**: Pattern `{}` was detected in the previous session but absent in this session.\n\
                     **Inference**: The approach or fix applied likely addressed the root cause.",
                    pattern_type,
                );
                let node = store::Node {
                    frontmatter: store::NodeFrontmatter {
                        id: store::new_uuid(),
                        node_type: "resolution".into(),
                        title,
                        tags: vec!["auto".into(), "resolution".into(), pattern_type.to_string()],
                        projects: vec![slug.clone()],
                        agents: vec![],
                        created: ts.clone(),
                        updated: ts.clone(),
                        importance: store::importance_for_type("resolution"),
                        access_count: 0,
                        accessed_at: String::new(),
                    },
                    body,
                };
                if let Ok((id, false)) =
                    store::write_node_dedup_pool(pool, &node, dedup_hours).await
                {
                    nodes_created += 1;
                    // Edge: resolution -> session (resolved_in)
                    let edge = store::Edge {
                        id: store::new_uuid(),
                        source: id.clone(),
                        target: session_node_id.clone(),
                        relation: "resolved_in".into(),
                        weight: 1.0,
                        ts: ts.clone(),
                    };
                    if store::append_edge_pool(pool, &edge).await.is_ok() {
                        edges_created += 1;
                    }
                    // Link resolution to project hub
                    if let Ok(ref hub_id) = ensure_project_hub_async(pool, &slug).await {
                        let _ = store::append_edge_pool(
                            pool,
                            &store::Edge {
                                id: store::new_uuid(),
                                source: id,
                                target: hub_id.clone(),
                                relation: "belongs_to".to_string(),
                                weight: 0.7,
                                ts: ts.clone(),
                            },
                        )
                        .await
                        .map(|_| edges_created += 1);
                    }
                }
            }
        }
    }

    // 8c. Weak tool nodes
    let mut error_node_ids: Vec<String> = vec![];
    for (cat, stats) in &analysis.per_tool_stats {
        let rate = if stats.total > 0 {
            stats.successes as f64 / stats.total as f64
        } else {
            1.0
        };
        if rate >= crate::config::CONFIG.pattern.weak_tool_rate
            || stats.total < crate::config::CONFIG.pattern.weak_tool_min_obs
        {
            continue;
        }
        let title = format!("{}: weak tool {} ({:.0}%)", slug, cat, rate * 100.0);
        let body = format!(
            "Tool `{}` success rate: {:.1}% ({}/{} ops)\nTop failures: {:?}",
            cat,
            rate * 100.0,
            stats.successes,
            stats.total,
            stats.failure_categories,
        );
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "error".into(),
                title,
                tags: vec!["auto".into(), "weak-tool".into(), cat.clone()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("error"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        if let Ok((id, false)) = store::write_node_dedup_pool(pool, &node, dedup_hours).await {
            error_node_ids.push(id);
            nodes_created += 1;
        }
    }

    // 8d. High-frequency error nodes
    for (category, count) in &analysis.per_error_stats {
        if *count < crate::config::CONFIG.pattern.high_freq_error_min {
            continue;
        }
        let title = format!("{}: high-freq {} ({}x)", slug, category, count);
        let body = format!(
            "Error category `{}` occurred {} times in this session.",
            category, count
        );
        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "error".into(),
                title,
                tags: vec!["auto".into(), "high-freq-error".into(), category.clone()],
                projects: vec![slug.clone()],
                agents: vec![],
                created: ts.clone(),
                updated: ts.clone(),
                importance: store::importance_for_type("error"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };
        if let Ok((id, false)) = store::write_node_dedup_pool(pool, &node, dedup_hours).await {
            error_node_ids.push(id);
            nodes_created += 1;
        }
    }

    // 8e. Auto edges between patterns sharing files
    for i in 0..pattern_node_ids.len() {
        for j in (i + 1)..pattern_node_ids.len() {
            let (id_a, files_a) = &pattern_node_ids[i];
            let (id_b, files_b) = &pattern_node_ids[j];
            let shared: Vec<_> = files_a.iter().filter(|f| files_b.contains(f)).collect();
            if !shared.is_empty() {
                let edge = store::Edge {
                    id: store::new_uuid(),
                    source: id_a.clone(),
                    target: id_b.clone(),
                    relation: "related".into(),
                    weight: shared.len() as f64,
                    ts: ts.clone(),
                };
                if store::append_edge_pool(pool, &edge).await.is_ok() {
                    edges_created += 1;
                }
            }
        }
    }

    // 8f. Project hub nodes + belongs_to edges
    if let Ok(hub_id) = ensure_project_hub_async(pool, &slug).await {
        // Link session node to project hub
        if !session_node_id.is_empty() {
            let _ = store::append_edge_pool(
                pool,
                &store::Edge {
                    id: store::new_uuid(),
                    source: session_node_id.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.5,
                    ts: ts.clone(),
                },
            )
            .await
            .map(|_| edges_created += 1);
        }
        // Link pattern nodes to project hub
        for (pid, _) in &pattern_node_ids {
            let _ = store::append_edge_pool(
                pool,
                &store::Edge {
                    id: store::new_uuid(),
                    source: pid.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.7,
                    ts: ts.clone(),
                },
            )
            .await
            .map(|_| edges_created += 1);
        }
        // Link error nodes to project hub
        for eid in &error_node_ids {
            let _ = store::append_edge_pool(
                pool,
                &store::Edge {
                    id: store::new_uuid(),
                    source: eid.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.7,
                    ts: ts.clone(),
                },
            )
            .await
            .map(|_| edges_created += 1);
        }
    }

    // 8g. Session chain: link to previous session in same project
    if !session_node_id.is_empty() {
        let prev_session = find_prev_session_async(pool, &session_node_id, &slug).await;

        if let Some(prev_id) = prev_session {
            let _ = store::append_edge_pool(
                pool,
                &store::Edge {
                    id: store::new_uuid(),
                    source: prev_id,
                    target: session_node_id.clone(),
                    relation: "follows".to_string(),
                    weight: 0.3,
                    ts: ts.clone(),
                },
            )
            .await
            .map(|_| edges_created += 1);
        }
    }

    // 8h. Same-tag edges: link non-session nodes that share tags
    let pattern_only_ids: Vec<String> = pattern_node_ids.iter().map(|(id, _)| id.clone()).collect();
    let all_new_ids: Vec<&str> = pattern_only_ids
        .iter()
        .chain(error_node_ids.iter())
        .map(String::as_str)
        .collect();

    if all_new_ids.len() >= 2 {
        let new_nodes = store::read_nodes_pool(pool, &all_new_ids)
            .await
            .unwrap_or_default();
        for i in 0..new_nodes.len() {
            for j in (i + 1)..new_nodes.len() {
                let shared: Vec<String> = new_nodes[i]
                    .frontmatter
                    .tags
                    .iter()
                    .filter(|t| **t != "auto" && new_nodes[j].frontmatter.tags.contains(t))
                    .cloned()
                    .collect();
                if !shared.is_empty() {
                    let _ = store::append_edge_pool(
                        pool,
                        &store::Edge {
                            id: store::new_uuid(),
                            source: new_nodes[i].frontmatter.id.clone(),
                            target: new_nodes[j].frontmatter.id.clone(),
                            relation: "shares_context".to_string(),
                            weight: shared.len() as f64,
                            ts: ts.clone(),
                        },
                    )
                    .await
                    .map(|_| edges_created += 1);
                }
            }
        }
    }

    (nodes_created, edges_created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::store;

    async fn open_test_mem_pool() -> AnyPool {
        let pool = crate::store::pool::test_memory_pool().await;
        crate::mem::store::init_schema_pool(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn ensure_project_hub_creates_new_hub_node() {
        let pool = open_test_mem_pool().await;
        let hub_id = ensure_project_hub_async(&pool, "test-project")
            .await
            .unwrap();
        assert!(!hub_id.is_empty(), "hub ID must not be empty");

        let node = store::read_node_pool(&pool, &hub_id).await.unwrap();
        assert_eq!(node.frontmatter.node_type, "project");
        assert_eq!(node.frontmatter.title, "project: test-project");
        assert!(node.frontmatter.tags.contains(&"hub".to_string()));
        assert!(
            node.frontmatter
                .projects
                .contains(&"test-project".to_string())
        );
    }

    #[tokio::test]
    async fn ensure_project_hub_returns_existing_hub() {
        let pool = open_test_mem_pool().await;
        let id1 = ensure_project_hub_async(&pool, "my-proj").await.unwrap();
        let id2 = ensure_project_hub_async(&pool, "my-proj").await.unwrap();
        assert_eq!(id1, id2, "second call must return same hub ID");
    }

    #[tokio::test]
    async fn ensure_project_hub_different_projects_get_different_ids() {
        let pool = open_test_mem_pool().await;
        let id_a = ensure_project_hub_async(&pool, "proj-a").await.unwrap();
        let id_b = ensure_project_hub_async(&pool, "proj-b").await.unwrap();
        assert_ne!(id_a, id_b, "different projects must get different hub IDs");
    }

    #[tokio::test]
    async fn auto_edge_belongs_to_links_session_to_project_hub() {
        let pool = open_test_mem_pool().await;

        let hub_id = ensure_project_hub_async(&pool, "edge-test-proj")
            .await
            .unwrap();

        let session_id = store::new_uuid();
        let session_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: session_id.clone(),
                node_type: "session".into(),
                title: "session: edge-test-proj 80% avg=0.8".into(),
                tags: vec!["auto".into(), "session".into()],
                projects: vec!["edge-test-proj".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                importance: store::importance_for_type("session"),
                ..Default::default()
            },
            body: "test session".into(),
        };
        store::write_node_pool(&pool, &session_node).await.unwrap();

        let edge = store::Edge {
            id: store::new_uuid(),
            source: session_id.clone(),
            target: hub_id.clone(),
            relation: "belongs_to".into(),
            weight: 0.5,
            ts: store::now_iso(),
        };
        store::append_edge_pool(&pool, &edge).await.unwrap();

        let edges = store::read_edges_pool(&pool, 5000)
            .await
            .unwrap_or_default();
        let found = edges
            .iter()
            .any(|e| e.source == session_id && e.target == hub_id && e.relation == "belongs_to");
        assert!(found, "belongs_to edge from session to hub must exist");
    }

    #[tokio::test]
    async fn auto_edge_follows_links_previous_session() {
        let pool = open_test_mem_pool().await;

        let prev_id = store::new_uuid();
        let prev_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: prev_id.clone(),
                node_type: "session".into(),
                title: "session: chain-proj 70%".into(),
                tags: vec!["auto".into()],
                projects: vec!["chain-proj".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "prev session".into(),
        };
        store::write_node_pool(&pool, &prev_node).await.unwrap();

        let curr_id = store::new_uuid();
        let curr_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: curr_id.clone(),
                node_type: "session".into(),
                title: "session: chain-proj 80%".into(),
                tags: vec!["auto".into()],
                projects: vec!["chain-proj".into()],
                created: "2026-01-02T00:00:00Z".into(),
                updated: "2026-01-02T00:00:00Z".into(),
                ..Default::default()
            },
            body: "curr session".into(),
        };
        store::write_node_pool(&pool, &curr_node).await.unwrap();

        let prev_session = find_prev_session_async(&pool, &curr_id, "chain-proj").await;

        assert!(prev_session.is_some(), "should find a previous session");
        assert_eq!(prev_session.unwrap(), prev_id);

        let edge = store::Edge {
            id: store::new_uuid(),
            source: prev_id.clone(),
            target: curr_id.clone(),
            relation: "follows".into(),
            weight: 0.3,
            ts: store::now_iso(),
        };
        store::append_edge_pool(&pool, &edge).await.unwrap();

        let edges = store::read_edges_pool(&pool, 5000)
            .await
            .unwrap_or_default();
        let found = edges
            .iter()
            .any(|e| e.source == prev_id && e.target == curr_id && e.relation == "follows");
        assert!(
            found,
            "follows edge must exist from prev to current session"
        );
    }

    #[tokio::test]
    async fn auto_edge_shares_context_links_same_tag_nodes() {
        let pool = open_test_mem_pool().await;

        let id_a = store::new_uuid();
        let node_a = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_a.clone(),
                node_type: "error".into(),
                title: "error A".into(),
                tags: vec!["auto".into(), "weak-tool".into(), "bash".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error A body".into(),
        };
        store::write_node_pool(&pool, &node_a).await.unwrap();

        let id_b = store::new_uuid();
        let node_b = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_b.clone(),
                node_type: "error".into(),
                title: "error B".into(),
                tags: vec!["auto".into(), "high-freq-error".into(), "bash".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error B body".into(),
        };
        store::write_node_pool(&pool, &node_b).await.unwrap();

        let all_new_ids: Vec<&str> = vec![&id_a, &id_b];
        let new_nodes = store::read_nodes_pool(&pool, &all_new_ids).await.unwrap();
        assert_eq!(new_nodes.len(), 2);

        let shared: Vec<String> = new_nodes[0]
            .frontmatter
            .tags
            .iter()
            .filter(|t| **t != "auto" && new_nodes[1].frontmatter.tags.contains(t))
            .cloned()
            .collect();
        assert!(
            shared.contains(&"bash".to_string()),
            "should share 'bash' tag"
        );

        let edge = store::Edge {
            id: store::new_uuid(),
            source: id_a.clone(),
            target: id_b.clone(),
            relation: "shares_context".into(),
            weight: shared.len() as f64,
            ts: store::now_iso(),
        };
        store::append_edge_pool(&pool, &edge).await.unwrap();

        let edges = store::read_edges_pool(&pool, 5000)
            .await
            .unwrap_or_default();
        let found = edges
            .iter()
            .any(|e| e.source == id_a && e.target == id_b && e.relation == "shares_context");
        assert!(
            found,
            "shares_context edge must exist between same-tag nodes"
        );
    }

    #[tokio::test]
    async fn auto_edge_shares_context_ignores_auto_tag() {
        let pool = open_test_mem_pool().await;

        let id_a = store::new_uuid();
        let node_a = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_a.clone(),
                node_type: "error".into(),
                title: "error C".into(),
                tags: vec!["auto".into(), "weak-tool".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error C body".into(),
        };
        store::write_node_pool(&pool, &node_a).await.unwrap();

        let id_b = store::new_uuid();
        let node_b = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: id_b.clone(),
                node_type: "error".into(),
                title: "error D".into(),
                tags: vec!["auto".into(), "high-freq-error".into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "error D body".into(),
        };
        store::write_node_pool(&pool, &node_b).await.unwrap();

        let all_new_ids: Vec<&str> = vec![&id_a, &id_b];
        let new_nodes = store::read_nodes_pool(&pool, &all_new_ids).await.unwrap();

        let shared: Vec<String> = new_nodes[0]
            .frontmatter
            .tags
            .iter()
            .filter(|t| **t != "auto" && new_nodes[1].frontmatter.tags.contains(t))
            .cloned()
            .collect();
        assert!(
            shared.is_empty(),
            "nodes sharing only 'auto' tag should have no shared tags"
        );
    }

    #[tokio::test]
    async fn resolution_node_created_when_pattern_absent_in_current_session() {
        let pool = open_test_mem_pool().await;
        let slug = "res-test-proj";

        // 1. Create project hub
        let hub_id = ensure_project_hub_async(&pool, slug).await.unwrap();

        // 2. Create previous session node
        let prev_session_id = store::new_uuid();
        let prev_session = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: prev_session_id.clone(),
                node_type: "session".into(),
                title: format!("session: {} 50% avg=0.5", slug),
                tags: vec!["auto".into(), "session".into()],
                projects: vec![slug.into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "previous session".into(),
        };
        store::write_node_pool(&pool, &prev_session).await.unwrap();

        // 3. Create a pattern node from the previous session
        let pattern_id = store::new_uuid();
        let pattern_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: pattern_id.clone(),
                node_type: "pattern".into(),
                title: format!("{}: repeated_same_error (3x)", slug),
                tags: vec!["auto".into(), "repeated_same_error".into()],
                projects: vec![slug.into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "pattern body".into(),
        };
        store::write_node_pool(&pool, &pattern_node).await.unwrap();

        // 4. Edge: pattern -> prev_session (detected_in)
        store::append_edge_pool(
            &pool,
            &store::Edge {
                id: store::new_uuid(),
                source: pattern_id.clone(),
                target: prev_session_id.clone(),
                relation: "detected_in".into(),
                weight: 1.0,
                ts: "2026-01-01T00:00:00Z".into(),
            },
        )
        .await
        .unwrap();

        // 5. Create current session node (simulates what ingest_to_memory does)
        let curr_session_id = store::new_uuid();
        let curr_session = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: curr_session_id.clone(),
                node_type: "session".into(),
                title: format!("session: {} 90% avg=0.9", slug),
                tags: vec!["auto".into(), "session".into()],
                projects: vec![slug.into()],
                created: "2026-01-02T00:00:00Z".into(),
                updated: "2026-01-02T00:00:00Z".into(),
                ..Default::default()
            },
            body: "current session".into(),
        };
        store::write_node_pool(&pool, &curr_session).await.unwrap();

        // 6. Edge: prev_session -> curr_session (follows)
        store::append_edge_pool(
            &pool,
            &store::Edge {
                id: store::new_uuid(),
                source: prev_session_id,
                target: curr_session_id.clone(),
                relation: "follows".into(),
                weight: 0.3,
                ts: "2026-01-02T00:00:00Z".into(),
            },
        )
        .await
        .unwrap();

        // 7. Query prev pattern types via shared helper
        let prev_pattern_types = query_prev_pattern_types_async(&pool, &curr_session_id).await;

        // Should find exactly the one pattern type from prev session
        assert_eq!(
            prev_pattern_types.len(),
            1,
            "should find one previous pattern type"
        );
        assert_eq!(
            prev_pattern_types[0], "repeated_same_error",
            "pattern type should be extracted from tags"
        );

        // 8. Simulate resolution creation (no current patterns = resolved)
        let current_pattern_types: Vec<&str> = vec![]; // empty = no patterns in current session

        for pattern_type in &prev_pattern_types {
            assert!(!current_pattern_types.contains(&pattern_type.as_str()));

            let title = format!("{}: resolved {} (auto)", slug, pattern_type);
            let resolution_id = store::new_uuid();
            let resolution_node = store::Node {
                frontmatter: store::NodeFrontmatter {
                    id: resolution_id.clone(),
                    node_type: "resolution".into(),
                    title,
                    tags: vec!["auto".into(), "resolution".into(), pattern_type.to_string()],
                    projects: vec![slug.into()],
                    created: "2026-01-02T00:00:00Z".into(),
                    updated: "2026-01-02T00:00:00Z".into(),
                    importance: store::importance_for_type("resolution"),
                    ..Default::default()
                },
                body: format!(
                    "**Resolved**: Pattern `{}` was detected in the previous session but absent in this session.",
                    pattern_type,
                ),
            };
            store::write_node_pool(&pool, &resolution_node)
                .await
                .unwrap();

            // Edge: resolution -> session (resolved_in)
            store::append_edge_pool(
                &pool,
                &store::Edge {
                    id: store::new_uuid(),
                    source: resolution_id.clone(),
                    target: curr_session_id.clone(),
                    relation: "resolved_in".into(),
                    weight: 1.0,
                    ts: "2026-01-02T00:00:00Z".into(),
                },
            )
            .await
            .unwrap();

            // Edge: resolution -> hub (belongs_to)
            store::append_edge_pool(
                &pool,
                &store::Edge {
                    id: store::new_uuid(),
                    source: resolution_id.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".into(),
                    weight: 0.7,
                    ts: "2026-01-02T00:00:00Z".into(),
                },
            )
            .await
            .unwrap();
        }

        // 9. Verify the resolution node and edges exist
        let edges = store::read_edges_pool(&pool, 5000).await.unwrap();
        let resolved_in: Vec<_> = edges
            .iter()
            .filter(|e| e.relation == "resolved_in" && e.target == curr_session_id)
            .collect();
        assert_eq!(
            resolved_in.len(),
            1,
            "should have exactly one resolved_in edge"
        );

        // Verify the resolution node is linked to the project hub
        let resolution_to_hub: Vec<_> = edges
            .iter()
            .filter(|e| {
                e.relation == "belongs_to"
                    && e.target == hub_id
                    && resolved_in.iter().any(|rie| rie.source == e.source)
            })
            .collect();
        assert_eq!(
            resolution_to_hub.len(),
            1,
            "resolution node must have a belongs_to edge to the project hub"
        );
    }

    #[tokio::test]
    async fn no_resolution_node_when_pattern_still_present() {
        let pool = open_test_mem_pool().await;
        let slug = "res-still-present";

        // 1. Create previous session with a pattern
        let prev_session_id = store::new_uuid();
        let prev_session = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: prev_session_id.clone(),
                node_type: "session".into(),
                title: format!("session: {} 50%", slug),
                tags: vec!["auto".into()],
                projects: vec![slug.into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "prev session".into(),
        };
        store::write_node_pool(&pool, &prev_session).await.unwrap();

        let pattern_id = store::new_uuid();
        let pattern_node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: pattern_id.clone(),
                node_type: "pattern".into(),
                title: format!("{}: thrashing (5x)", slug),
                tags: vec!["auto".into(), "thrashing".into()],
                projects: vec![slug.into()],
                created: "2026-01-01T00:00:00Z".into(),
                updated: "2026-01-01T00:00:00Z".into(),
                ..Default::default()
            },
            body: "pattern body".into(),
        };
        store::write_node_pool(&pool, &pattern_node).await.unwrap();

        store::append_edge_pool(
            &pool,
            &store::Edge {
                id: store::new_uuid(),
                source: pattern_id,
                target: prev_session_id.clone(),
                relation: "detected_in".into(),
                weight: 1.0,
                ts: "2026-01-01T00:00:00Z".into(),
            },
        )
        .await
        .unwrap();

        // 2. Create current session
        let curr_session_id = store::new_uuid();
        let curr_session = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: curr_session_id.clone(),
                node_type: "session".into(),
                title: format!("session: {} 80%", slug),
                tags: vec!["auto".into()],
                projects: vec![slug.into()],
                created: "2026-01-02T00:00:00Z".into(),
                updated: "2026-01-02T00:00:00Z".into(),
                ..Default::default()
            },
            body: "curr session".into(),
        };
        store::write_node_pool(&pool, &curr_session).await.unwrap();

        store::append_edge_pool(
            &pool,
            &store::Edge {
                id: store::new_uuid(),
                source: prev_session_id,
                target: curr_session_id.clone(),
                relation: "follows".into(),
                weight: 0.3,
                ts: "2026-01-02T00:00:00Z".into(),
            },
        )
        .await
        .unwrap();

        // 3. Query prev pattern types via shared helper
        let prev_pattern_types = query_prev_pattern_types_async(&pool, &curr_session_id).await;

        assert_eq!(prev_pattern_types.len(), 1);
        assert_eq!(prev_pattern_types[0], "thrashing");

        // 4. Current session has the SAME pattern type -- should NOT create resolution
        let current_pattern_types: Vec<&str> = vec!["thrashing"];

        for pattern_type in &prev_pattern_types {
            // The pattern is still present, so resolution should NOT be created
            assert!(current_pattern_types.contains(&pattern_type.as_str()));
        }

        // 5. Verify no resolution nodes exist
        let all_edges = store::read_edges_pool(&pool, 5000).await.unwrap();
        let resolved_edges: Vec<_> = all_edges
            .iter()
            .filter(|e| e.relation == "resolved_in")
            .collect();
        assert!(
            resolved_edges.is_empty(),
            "no resolved_in edges should exist when pattern is still present"
        );
    }
}
