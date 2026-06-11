//! types.rs — Node, Edge, Index, ScoredNode, and type-level helpers
//!
//! Keeps the two-tier `Node { frontmatter, body }` as the public API type.
//! Local flat `GraphNode` and `GraphEdge` types replace the removed llm_kernel types.

use serde::{Deserialize, Serialize};

// ── Importance defaults by node type ──────────────────────

/// Default importance score for a node type. Used when no explicit importance is set.
pub fn importance_for_type(node_type: &str) -> f64 {
    match node_type {
        "decision" => 0.9,
        "resolution" => 0.8,
        "concept" | "project" => 0.7,
        "pattern" => 0.5,
        "error" => 0.4,
        "session" => 0.05,
        _ => 0.5,
    }
}

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

// ── Flat graph types (replace llm_kernel graph types) ──────

/// Flat node as stored in the `nodes` table.
/// Fields use CSV strings for tags/projects/agents (SQL storage format).
#[derive(Debug, Clone)]
pub(crate) struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub title: String,
    pub body: String,
    pub tags: String,     // CSV
    pub projects: String, // CSV
    pub agents: String,   // CSV
    pub created: String,
    pub updated: String,
    pub importance: f64,
    pub access_count: i64,
    pub accessed_at: String,
}

/// Flat edge as stored in the `edges` table.
#[derive(Debug, Clone)]
pub(crate) struct GraphEdge {
    pub source: String,
    pub target: String,
    pub label: String,
    pub created: String,
}

// ── Conversion helpers ──────────────────────────────────────

/// Convert two-tier Node → flat GraphNode for SQL storage.
pub(crate) fn node_to_graph(node: Node) -> GraphNode {
    GraphNode {
        id: node.frontmatter.id,
        node_type: node.frontmatter.node_type,
        title: node.frontmatter.title,
        body: node.body,
        tags: node.frontmatter.tags.join(","),
        projects: node.frontmatter.projects.join(","),
        agents: node.frontmatter.agents.join(","),
        created: node.frontmatter.created,
        updated: node.frontmatter.updated,
        importance: node.frontmatter.importance,
        access_count: node.frontmatter.access_count,
        accessed_at: node.frontmatter.accessed_at,
    }
}

/// Convert flat GraphNode → two-tier Node for public API.
pub(crate) fn graph_to_node(gn: GraphNode) -> Node {
    Node {
        frontmatter: NodeFrontmatter {
            id: gn.id,
            node_type: gn.node_type,
            title: gn.title,
            tags: super::util::split_csv(&gn.tags),
            projects: super::util::split_csv(&gn.projects),
            agents: super::util::split_csv(&gn.agents),
            created: gn.created,
            updated: gn.updated,
            importance: gn.importance,
            access_count: gn.access_count,
            accessed_at: gn.accessed_at,
        },
        body: gn.body,
    }
}

/// Convert Edge → GraphEdge for SQL storage.
pub(crate) fn edge_to_graph(edge: &Edge) -> GraphEdge {
    GraphEdge {
        source: edge.source.clone(),
        target: edge.target.clone(),
        label: edge.relation.clone(),
        created: edge.ts.clone(),
    }
}
