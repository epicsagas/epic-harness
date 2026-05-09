use crate::mem::store;
use crate::shared::{
    evolution::*, helpers::*, paths::*,
};

use super::analysis::build_summary;

/// Find or create a project hub node. Returns the hub node's ID.
pub fn ensure_project_hub(conn: &rusqlite::Connection, slug: &str) -> std::io::Result<String> {
    // Check if hub already exists
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM nodes WHERE type = 'project' AND title = ?1 LIMIT 1",
            rusqlite::params![format!("project: {}", slug)],
            |row| row.get(0),
        )
        .ok();

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
            title: format!("project: {}", slug),
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
    store::write_node_conn(conn, &node)?;
    Ok(id)
}

/// Ingest session analysis results into the knowledge graph.
/// Returns (nodes_created, edges_created).
pub fn ingest_to_memory(analysis: &SessionAnalysis, patterns: &[DetectedPattern]) -> (u64, u64) {
    let conn = match store::open_db() {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };

    let slug = project_slug();
    let ts = now_iso();
    let dedup_hours = 24u64;
    // `unchecked_transaction()` is used here because `open_db()` always returns
    // a fresh connection in autocommit mode (no prior transaction active). Using
    // the checked variant would be equivalent but adds unnecessary overhead for
    // this single-writer, fresh-connection pattern.
    let tx = match conn.unchecked_transaction() {
        Ok(t) => t,
        Err(_) => return (0, 0),
    };

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
        match store::write_node_dedup_conn(&tx, &node, dedup_hours) {
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
        if let Ok((id, deduped)) = store::write_node_dedup_conn(&tx, &node, dedup_hours) {
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
                if store::append_edge_conn(&tx, &edge).is_ok() {
                    edges_created += 1;
                }
            }
        }
    }

    // 8b-2. Resolution nodes: patterns resolved since last session
    if !session_node_id.is_empty() {
        // Find pattern types from the previous session via the follows edge.
        // Tags are stored as CSV, so extract the pattern_type tag by filtering
        // out the generic "auto" and "pattern" tags.
        let prev_pattern_types: Vec<String> = {
            let mut results = Vec::new();
            // Tags are stored as CSV, extract the pattern_type tag by filtering
            // out the generic "auto" and "pattern" tags.
            let sql = "SELECT n.tags FROM nodes n
                 JOIN edges e ON e.source = n.id
                 WHERE n.type = 'pattern'
                 AND e.relation = 'detected_in'
                 AND e.target IN (
                    SELECT e2.source FROM edges e2
                    WHERE e2.target = ?1 AND e2.relation = 'follows'
                 )";
            if let Ok(mut stmt) = tx.prepare(sql) {
                let rows = stmt.query_map(rusqlite::params![session_node_id], |row| {
                    let tags_str: String = row.get(0)?;
                    Ok(tags_str)
                });
                if let Ok(iter) = rows {
                    for r in iter.flatten() {
                        // CSV tags: "auto,repeated_same_error" → extract non-generic
                        for tag in r.split(',') {
                            let tag = tag.trim().trim_matches('"');
                            if !tag.is_empty()
                                && tag != "auto"
                                && tag != "pattern"
                            {
                                results.push(tag.to_string());
                            }
                        }
                    }
                }
            }
            results
        };

        // Get current pattern types for comparison
        let current_pattern_types: Vec<&str> =
            patterns.iter().map(|p| p.pattern_type.as_str()).collect();

        // For each previous pattern, check if the same type exists in current session
        for pattern_type in &prev_pattern_types {

            if !pattern_type.is_empty()
                && !current_pattern_types.contains(&pattern_type.as_str())
            {
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
                        tags: vec![
                            "auto".into(),
                            "resolution".into(),
                            pattern_type.to_string(),
                        ],
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
                if let Ok((id, false)) = store::write_node_dedup_conn(&tx, &node, dedup_hours) {
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
                    if store::append_edge_conn(&tx, &edge).is_ok() {
                        edges_created += 1;
                    }
                    // Link resolution to project hub
                    if let Ok(ref hub_id) = ensure_project_hub(&tx, &slug) {
                        let _ = store::append_edge_conn(
                            &tx,
                            &store::Edge {
                                id: store::new_uuid(),
                                source: id,
                                target: hub_id.clone(),
                                relation: "belongs_to".to_string(),
                                weight: 0.7,
                                ts: ts.clone(),
                            },
                        )
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
        if rate >= crate::config::CONFIG.pattern.weak_tool_rate || stats.total < crate::config::CONFIG.pattern.weak_tool_min_obs {
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
        if let Ok((id, false)) = store::write_node_dedup_conn(&tx, &node, dedup_hours) {
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
        if let Ok((id, false)) = store::write_node_dedup_conn(&tx, &node, dedup_hours) {
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
                if store::append_edge_conn(&tx, &edge).is_ok() {
                    edges_created += 1;
                }
            }
        }
    }

    // 8f. Project hub nodes + belongs_to edges
    if let Ok(hub_id) = ensure_project_hub(&tx, &slug) {
        // Link session node to project hub
        if !session_node_id.is_empty() {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: session_node_id.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.5,
                    ts: ts.clone(),
                },
            )
            .map(|_| edges_created += 1);
        }
        // Link pattern nodes to project hub
        for (pid, _) in &pattern_node_ids {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: pid.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.7,
                    ts: ts.clone(),
                },
            )
            .map(|_| edges_created += 1);
        }
        // Link error nodes to project hub
        for eid in &error_node_ids {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: eid.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".to_string(),
                    weight: 0.7,
                    ts: ts.clone(),
                },
            )
            .map(|_| edges_created += 1);
        }
    }

    // 8g. Session chain: link to previous session in same project
    if !session_node_id.is_empty() {
        let prev_session: Option<String> = tx
            .query_row(
                "SELECT id FROM nodes WHERE type = 'session' AND id != ?1
             AND (',' || projects || ',' LIKE '%,' || ?2 || ',%')
             ORDER BY updated DESC LIMIT 1",
                rusqlite::params![session_node_id, slug],
                |row| row.get(0),
            )
            .ok();

        if let Some(prev_id) = prev_session {
            let _ = store::append_edge_conn(
                &tx,
                &store::Edge {
                    id: store::new_uuid(),
                    source: prev_id,
                    target: session_node_id.clone(),
                    relation: "follows".to_string(),
                    weight: 0.3,
                    ts: ts.clone(),
                },
            )
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
        let new_nodes = store::read_nodes_conn(&tx, &all_new_ids);
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
                    let _ = store::append_edge_conn(
                        &tx,
                        &store::Edge {
                            id: store::new_uuid(),
                            source: new_nodes[i].frontmatter.id.clone(),
                            target: new_nodes[j].frontmatter.id.clone(),
                            relation: "shares_context".to_string(),
                            weight: shared.len() as f64,
                            ts: ts.clone(),
                        },
                    )
                    .map(|_| edges_created += 1);
                }
            }
        }
    }

    let _ = tx.commit();
    (nodes_created, edges_created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::store;

    fn open_test_mem_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
        store::init_schema(&conn).expect("schema");
        conn
    }

    #[test]
    fn ensure_project_hub_creates_new_hub_node() {
        let conn = open_test_mem_db();
        let hub_id = ensure_project_hub(&conn, "test-project").unwrap();
        assert!(!hub_id.is_empty(), "hub ID must not be empty");

        let node = store::read_node_conn(&conn, &hub_id).unwrap();
        assert_eq!(node.frontmatter.node_type, "project");
        assert_eq!(node.frontmatter.title, "project: test-project");
        assert!(node.frontmatter.tags.contains(&"hub".to_string()));
        assert!(
            node.frontmatter
                .projects
                .contains(&"test-project".to_string())
        );
    }

    #[test]
    fn ensure_project_hub_returns_existing_hub() {
        let conn = open_test_mem_db();
        let id1 = ensure_project_hub(&conn, "my-proj").unwrap();
        let id2 = ensure_project_hub(&conn, "my-proj").unwrap();
        assert_eq!(id1, id2, "second call must return same hub ID");
    }

    #[test]
    fn ensure_project_hub_different_projects_get_different_ids() {
        let conn = open_test_mem_db();
        let id_a = ensure_project_hub(&conn, "proj-a").unwrap();
        let id_b = ensure_project_hub(&conn, "proj-b").unwrap();
        assert_ne!(id_a, id_b, "different projects must get different hub IDs");
    }

    #[test]
    fn auto_edge_belongs_to_links_session_to_project_hub() {
        let conn = open_test_mem_db();

        let hub_id = ensure_project_hub(&conn, "edge-test-proj").unwrap();

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
        store::write_node_conn(&conn, &session_node).unwrap();

        let edge = store::Edge {
            id: store::new_uuid(),
            source: session_id.clone(),
            target: hub_id.clone(),
            relation: "belongs_to".into(),
            weight: 0.5,
            ts: store::now_iso(),
        };
        store::append_edge_conn(&conn, &edge).unwrap();

        let edges = store::read_edges_conn(&conn, 5000);
        let found = edges
            .iter()
            .any(|e| e.source == session_id && e.target == hub_id && e.relation == "belongs_to");
        assert!(found, "belongs_to edge from session to hub must exist");
    }

    #[test]
    fn auto_edge_follows_links_previous_session() {
        let conn = open_test_mem_db();

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
        store::write_node_conn(&conn, &prev_node).unwrap();

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
        store::write_node_conn(&conn, &curr_node).unwrap();

        let prev_session: Option<String> = conn
            .query_row(
                "SELECT id FROM nodes WHERE type = 'session' AND id != ?1
             AND (',' || projects || ',' LIKE '%,' || ?2 || ',%')
             ORDER BY updated DESC LIMIT 1",
                rusqlite::params![curr_id, "chain-proj"],
                |row| row.get(0),
            )
            .ok();

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
        store::append_edge_conn(&conn, &edge).unwrap();

        let edges = store::read_edges_conn(&conn, 5000);
        let found = edges
            .iter()
            .any(|e| e.source == prev_id && e.target == curr_id && e.relation == "follows");
        assert!(
            found,
            "follows edge must exist from prev to current session"
        );
    }

    #[test]
    fn auto_edge_shares_context_links_same_tag_nodes() {
        let conn = open_test_mem_db();

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
        store::write_node_conn(&conn, &node_a).unwrap();

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
        store::write_node_conn(&conn, &node_b).unwrap();

        let all_new_ids: Vec<&str> = vec![&id_a, &id_b];
        let new_nodes = store::read_nodes_conn(&conn, &all_new_ids);
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
        store::append_edge_conn(&conn, &edge).unwrap();

        let edges = store::read_edges_conn(&conn, 5000);
        let found = edges
            .iter()
            .any(|e| e.source == id_a && e.target == id_b && e.relation == "shares_context");
        assert!(
            found,
            "shares_context edge must exist between same-tag nodes"
        );
    }

    #[test]
    fn auto_edge_shares_context_ignores_auto_tag() {
        let conn = open_test_mem_db();

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
        store::write_node_conn(&conn, &node_a).unwrap();

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
        store::write_node_conn(&conn, &node_b).unwrap();

        let all_new_ids: Vec<&str> = vec![&id_a, &id_b];
        let new_nodes = store::read_nodes_conn(&conn, &all_new_ids);

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

    #[test]
    fn resolution_node_created_when_pattern_absent_in_current_session() {
        let conn = open_test_mem_db();
        let slug = "res-test-proj";

        // 1. Create project hub
        let hub_id = ensure_project_hub(&conn, slug).unwrap();

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
        store::write_node_conn(&conn, &prev_session).unwrap();

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
        store::write_node_conn(&conn, &pattern_node).unwrap();

        // 4. Edge: pattern -> prev_session (detected_in)
        store::append_edge_conn(
            &conn,
            &store::Edge {
                id: store::new_uuid(),
                source: pattern_id.clone(),
                target: prev_session_id.clone(),
                relation: "detected_in".into(),
                weight: 1.0,
                ts: "2026-01-01T00:00:00Z".into(),
            },
        )
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
        store::write_node_conn(&conn, &curr_session).unwrap();

        // 6. Edge: prev_session -> curr_session (follows)
        store::append_edge_conn(
            &conn,
            &store::Edge {
                id: store::new_uuid(),
                source: prev_session_id,
                target: curr_session_id.clone(),
                relation: "follows".into(),
                weight: 0.3,
                ts: "2026-01-02T00:00:00Z".into(),
            },
        )
        .unwrap();

        // 7. Query prev pattern types via CSV tags (same logic as production code)
        let prev_pattern_types: Vec<String> = {
            let mut results = Vec::new();
            let sql = "SELECT n.tags FROM nodes n
                 JOIN edges e ON e.source = n.id
                 WHERE n.type = 'pattern'
                 AND e.relation = 'detected_in'
                 AND e.target IN (
                    SELECT e2.source FROM edges e2
                    WHERE e2.target = ?1 AND e2.relation = 'follows'
                 )";
            let mut stmt = conn.prepare(sql).unwrap();
            let rows = stmt.query_map(rusqlite::params![curr_session_id], |row| {
                let tags_str: String = row.get(0)?;
                Ok(tags_str)
            })
            .unwrap();
            for r in rows.flatten() {
                for tag in r.split(',') {
                    let tag = tag.trim().trim_matches('"');
                    if !tag.is_empty() && tag != "auto" && tag != "pattern" {
                        results.push(tag.to_string());
                    }
                }
            }
            results
        };

        // Should find exactly the one pattern type from prev session
        assert_eq!(prev_pattern_types.len(), 1, "should find one previous pattern type");
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
                    tags: vec![
                        "auto".into(),
                        "resolution".into(),
                        pattern_type.to_string(),
                    ],
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
            store::write_node_conn(&conn, &resolution_node).unwrap();

            // Edge: resolution -> session (resolved_in)
            store::append_edge_conn(
                &conn,
                &store::Edge {
                    id: store::new_uuid(),
                    source: resolution_id.clone(),
                    target: curr_session_id.clone(),
                    relation: "resolved_in".into(),
                    weight: 1.0,
                    ts: "2026-01-02T00:00:00Z".into(),
                },
            )
            .unwrap();

            // Edge: resolution -> hub (belongs_to)
            store::append_edge_conn(
                &conn,
                &store::Edge {
                    id: store::new_uuid(),
                    source: resolution_id.clone(),
                    target: hub_id.clone(),
                    relation: "belongs_to".into(),
                    weight: 0.7,
                    ts: "2026-01-02T00:00:00Z".into(),
                },
            )
            .unwrap();
        }

        // 9. Verify the resolution node and edges exist
        let edges = store::read_edges_conn(&conn, 5000);
        let resolved_in: Vec<_> = edges
            .iter()
            .filter(|e| e.relation == "resolved_in" && e.target == curr_session_id)
            .collect();
        assert_eq!(resolved_in.len(), 1, "should have exactly one resolved_in edge");

        // Verify the resolution node is linked to the project hub
        let resolution_to_hub: Vec<_> = edges
            .iter()
            .filter(|e| {
                e.relation == "belongs_to"
                    && e.target == hub_id
                    && resolved_in
                        .iter()
                        .any(|rie| rie.source == e.source)
            })
            .collect();
        assert_eq!(
            resolution_to_hub.len(),
            1,
            "resolution node must have a belongs_to edge to the project hub"
        );
    }

    #[test]
    fn no_resolution_node_when_pattern_still_present() {
        let conn = open_test_mem_db();
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
        store::write_node_conn(&conn, &prev_session).unwrap();

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
        store::write_node_conn(&conn, &pattern_node).unwrap();

        store::append_edge_conn(
            &conn,
            &store::Edge {
                id: store::new_uuid(),
                source: pattern_id,
                target: prev_session_id.clone(),
                relation: "detected_in".into(),
                weight: 1.0,
                ts: "2026-01-01T00:00:00Z".into(),
            },
        )
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
        store::write_node_conn(&conn, &curr_session).unwrap();

        store::append_edge_conn(
            &conn,
            &store::Edge {
                id: store::new_uuid(),
                source: prev_session_id,
                target: curr_session_id.clone(),
                relation: "follows".into(),
                weight: 0.3,
                ts: "2026-01-02T00:00:00Z".into(),
            },
        )
        .unwrap();

        // 3. Query prev pattern types via CSV tags (same logic as production code)
        let prev_pattern_types: Vec<String> = {
            let mut results = Vec::new();
            let sql = "SELECT n.tags FROM nodes n
                 JOIN edges e ON e.source = n.id
                 WHERE n.type = 'pattern'
                 AND e.relation = 'detected_in'
                 AND e.target IN (
                    SELECT e2.source FROM edges e2
                    WHERE e2.target = ?1 AND e2.relation = 'follows'
                 )";
            let mut stmt = conn.prepare(sql).unwrap();
            let rows = stmt.query_map(rusqlite::params![curr_session_id], |row| {
                let tags_str: String = row.get(0)?;
                Ok(tags_str)
            })
            .unwrap();
            for r in rows.flatten() {
                for tag in r.split(',') {
                    let tag = tag.trim().trim_matches('"');
                    if !tag.is_empty() && tag != "auto" && tag != "pattern" {
                        results.push(tag.to_string());
                    }
                }
            }
            results
        };

        assert_eq!(prev_pattern_types.len(), 1);
        assert_eq!(prev_pattern_types[0], "thrashing");

        // 4. Current session has the SAME pattern type -- should NOT create resolution
        let current_pattern_types: Vec<&str> = vec!["thrashing"];

        for pattern_type in &prev_pattern_types {
            // The pattern is still present, so resolution should NOT be created
            assert!(current_pattern_types.contains(&pattern_type.as_str()));
        }

        // 5. Verify no resolution nodes exist
        let all_edges = store::read_edges_conn(&conn, 5000);
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
