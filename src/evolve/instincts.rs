use serde::{Deserialize, Serialize};

use crate::config::CONFIG;
use crate::mem::store;
use crate::shared::{evolution::*, helpers::*, paths::*};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Instinct {
    pub trigger: String,
    pub confidence: f64,
    pub domain: String,
    pub scope: String,
    pub observation_count: u64,
    pub success_count: u64,
    pub projects: Vec<String>,
}

pub fn extract_instincts(
    observations: &[crate::shared::obs::ObsRecord],
    analysis: &SessionAnalysis,
) -> Vec<Instinct> {
    if observations.len() < CONFIG.instinct.min_observations
        || analysis.avg_score < CONFIG.instinct.min_avg_score
    {
        return vec![];
    }

    // Extract per-tool-category instincts
    let mut instincts = Vec::new();
    for (tool_cat, stats) in &analysis.per_tool_stats {
        if stats.total < 3 {
            continue;
        }
        let rate = stats.successes as f64 / stats.total as f64;
        if rate >= CONFIG.instinct.confidence_threshold {
            instincts.push(Instinct {
                trigger: format!("high-success-{}", tool_cat),
                confidence: rate,
                domain: "tool-usage".into(),
                scope: "local".into(),
                observation_count: stats.total,
                success_count: stats.successes,
                projects: vec![project_slug()],
            });
        }
    }

    // Extract per-file-extension instincts
    for (ext, stats) in &analysis.per_ext_stats {
        if stats.total < 3 {
            continue;
        }
        let rate = stats.success_rate;
        if rate >= CONFIG.instinct.confidence_threshold {
            instincts.push(Instinct {
                trigger: format!("high-success-{}", ext.trim_start_matches('.')),
                confidence: rate,
                domain: ext.trim_start_matches('.').into(),
                scope: "local".into(),
                observation_count: stats.total,
                success_count: stats.total - stats.errors,
                projects: vec![project_slug()],
            });
        }
    }

    instincts.truncate(CONFIG.instinct.max_instincts);
    instincts
}

pub fn promote_instincts_to_global(instincts: &[Instinct]) -> u64 {
    let pool = match crate::store::runtime::block_on(crate::store::pool::memory_pool()) {
        Ok(p) => p,
        Err(_) => return 0,
    };

    let mut promoted = 0u64;
    for instinct in instincts {
        if instinct.confidence < CONFIG.instinct.confidence_threshold {
            continue;
        }

        // Only promote instincts seen across multiple projects (or single-project
        // when the threshold is set to 1 for early bootstrapping).
        if instinct.projects.len() < CONFIG.instinct.promotion_min_projects {
            continue;
        }

        let title = format!("instinct: {} ({})", instinct.trigger, instinct.domain);
        let body = format!(
            "**Trigger**: {}\n**Confidence**: {:.2}\n**Domain**: {}\n**Scope**: {}\n**Observations**: {} ({} successful)\n**Projects**: {}",
            instinct.trigger,
            instinct.confidence,
            instinct.domain,
            instinct.scope,
            instinct.observation_count,
            instinct.success_count,
            instinct.projects.join(", "),
        );

        let node = store::Node {
            frontmatter: store::NodeFrontmatter {
                id: store::new_uuid(),
                node_type: "instinct".into(),
                title,
                tags: vec!["auto".into(), "instinct".into(), instinct.domain.clone()],
                projects: instinct.projects.clone(),
                agents: vec![],
                created: now_iso(),
                updated: now_iso(),
                importance: store::importance_for_type("instinct"),
                access_count: 0,
                accessed_at: String::new(),
            },
            body,
        };

        // Use dedup to avoid duplicate instincts (7-day window)
        // write_node_dedup_pool returns (id, is_new): true = newly written, false = duplicate
        if let Ok((_, is_new)) =
            crate::store::runtime::block_on(store::write_node_dedup_pool(&pool, &node, 168))
            && is_new
        {
            promoted += 1;
        }
    }

    promoted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG;
    use crate::shared::obs::ObsRecord;
    use crate::shared::scoring::ScoreDimensions;
    use std::collections::HashMap;

    fn make_obs(
        tool: &str,
        cat: &str,
        result: &str,
        score: f64,
        action: Option<&str>,
    ) -> ObsRecord {
        ObsRecord {
            timestamp: "2026-04-09T12:00:00Z".into(),
            tool: tool.into(),
            tool_category: cat.into(),
            action: action.map(String::from),
            result: Some(result.into()),
            score: Some(score),
            dimensions: Some(ScoreDimensions {
                tool_success: if result == "success" { 1.0 } else { 0.0 },
                output_quality: score,
                execution_cost: 1.0,
            }),
            failure_category: if result == "error" {
                Some("type_error".into())
            } else {
                None
            },
            error_snippet: if result == "error" {
                Some("TypeError: x is not a function".into())
            } else {
                None
            },
            file_ext: Some(".ts".into()),
            sequence_id: Some(1),
            pipeline_id: None,
        }
    }

    #[test]
    fn extract_instincts_returns_empty_for_few_observations() {
        let obs: Vec<ObsRecord> = (0..5)
            .map(|_| make_obs("Bash", "bash", "success", 0.9, Some("ls")))
            .collect();
        let analysis = SessionAnalysis {
            total_observations: 5,
            avg_score: 0.9,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.is_empty(),
            "fewer than 10 observations should yield no instincts"
        );
    }

    #[test]
    fn extract_instincts_returns_empty_for_low_score() {
        let obs: Vec<ObsRecord> = (0..15)
            .map(|_| make_obs("Bash", "bash", "success", 0.3, Some("ls")))
            .collect();
        let analysis = SessionAnalysis {
            total_observations: 15,
            avg_score: 0.3,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.is_empty(),
            "avg_score < 0.5 should yield no instincts"
        );
    }

    #[test]
    fn extract_instincts_finds_high_success_tools() {
        let obs: Vec<ObsRecord> = (0..15)
            .map(|_| make_obs("Bash", "bash", "success", 0.9, Some("ls")))
            .collect();
        let mut per_tool = HashMap::new();
        per_tool.insert(
            "bash".into(),
            ToolStats {
                tool_category: "bash".into(),
                total: 15,
                successes: 15,
                errors: 0,
                avg_score: 0.9,
                failure_categories: HashMap::new(),
            },
        );
        let analysis = SessionAnalysis {
            total_observations: 15,
            avg_score: 0.9,
            per_tool_stats: per_tool,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.iter().any(|i| i.trigger == "high-success-bash"),
            "should find high-success-bash instinct"
        );
    }

    #[test]
    fn extract_instincts_respects_max_limit() {
        let obs: Vec<ObsRecord> = (0..15)
            .map(|_| make_obs("Bash", "bash", "success", 0.9, Some("ls")))
            .collect();
        let mut per_tool = HashMap::new();
        for i in 0..30 {
            let cat = format!("tool-{}", i);
            per_tool.insert(
                cat.clone(),
                ToolStats {
                    tool_category: cat,
                    total: 5,
                    successes: 5,
                    errors: 0,
                    avg_score: 0.9,
                    failure_categories: HashMap::new(),
                },
            );
        }
        let analysis = SessionAnalysis {
            total_observations: 150,
            avg_score: 0.9,
            per_tool_stats: per_tool,
            ..Default::default()
        };
        let instincts = extract_instincts(&obs, &analysis);
        assert!(
            instincts.len() <= CONFIG.instinct.max_instincts,
            "should not exceed max_instincts ({})",
            CONFIG.instinct.max_instincts
        );
    }

    #[test]
    fn instinct_promotion_requires_multi_project_by_config() {
        let instinct_one_project = Instinct {
            trigger: "high-success-bash".into(),
            confidence: 0.9,
            domain: "tool-usage".into(),
            scope: "local".into(),
            observation_count: 20,
            success_count: 18,
            projects: vec!["project-a".into()],
        };
        assert!(
            instinct_one_project.projects.len() < CONFIG.instinct.promotion_min_projects,
            "single-project instinct must fail the promotion gate with default config (min=2)"
        );

        let instinct_two_projects = Instinct {
            trigger: "high-success-bash".into(),
            confidence: 0.9,
            domain: "tool-usage".into(),
            scope: "local".into(),
            observation_count: 20,
            success_count: 18,
            projects: vec!["project-a".into(), "project-b".into()],
        };
        assert!(
            instinct_two_projects.projects.len() >= CONFIG.instinct.promotion_min_projects,
            "two-project instinct must pass the promotion gate with default config (min=2)"
        );
    }
}
