//! types.rs — Node, Edge, Index, ScoredNode, and type-level helpers

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeFrontmatter {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub agents: Vec<String>,
    pub created: String,
    pub updated: String,
    /// Importance score (0.0–1.0). Higher = more valuable for recall.
    #[serde(default = "default_importance")]
    pub importance: f64,
    /// How many times this node has been retrieved via recall/search.
    #[serde(default)]
    pub access_count: i64,
    /// Last time this node was accessed (not just updated).
    #[serde(default)]
    pub accessed_at: String,
}

pub fn default_importance() -> f64 {
    0.5
}

/// Default importance by node type.
pub fn importance_for_type(node_type: &str) -> f64 {
    match node_type {
        "decision" => 0.9,
        "resolution" => 0.8,
        "psychographic" => 0.8,
        "instinct" => 0.7,
        "concept" => 0.7,
        "project" => 0.7,
        "pattern" => 0.5,
        "error" => 0.4,
        "session" => 0.05,
        _ => 0.5,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub frontmatter: NodeFrontmatter,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub weight: f64,
    pub ts: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(dead_code)]
pub struct Index {
    pub nodes: Vec<IndexNode>,
    pub by_tag: std::collections::HashMap<String, Vec<String>>,
    pub by_type: std::collections::HashMap<String, Vec<String>>,
    pub by_project: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexNode {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub tags: Vec<String>,
    pub projects: Vec<String>,
    pub updated: String,
}

/// Scored node: a node with a computed relevance score.
#[derive(Debug, Clone)]
pub struct ScoredNode {
    pub node: Node,
    pub score: f64,
}
