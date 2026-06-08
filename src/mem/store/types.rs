//! types.rs — Node, Edge, Index, ScoredNode, and type-level helpers
//!
//! Keeps the two-tier `Node { frontmatter, body }` as the public API type.
//! Conversion functions bridge to llm-kernel's flat `GraphNode`.

use serde::{Deserialize, Serialize};

pub use llm_kernel::graph::types::importance_for_type;

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

// ── Conversion helpers ──────────────────────────────────

/// Convert two-tier Node → flat GraphNode for llm-kernel.
pub fn node_to_graph(node: Node) -> llm_kernel::graph::types::GraphNode {
    llm_kernel::graph::types::GraphNode {
        id: node.frontmatter.id,
        node_type: node.frontmatter.node_type,
        title: node.frontmatter.title,
        body: node.body,
        tags: node.frontmatter.tags,
        projects: node.frontmatter.projects,
        agents: node.frontmatter.agents,
        created: node.frontmatter.created,
        updated: node.frontmatter.updated,
        importance: node.frontmatter.importance,
        access_count: node.frontmatter.access_count,
        accessed_at: node.frontmatter.accessed_at,
    }
}

/// Convert flat GraphNode → two-tier Node for public API.
pub fn graph_to_node(gn: llm_kernel::graph::types::GraphNode) -> Node {
    Node {
        frontmatter: NodeFrontmatter {
            id: gn.id,
            node_type: gn.node_type,
            title: gn.title,
            tags: gn.tags,
            projects: gn.projects,
            agents: gn.agents,
            created: gn.created,
            updated: gn.updated,
            importance: gn.importance,
            access_count: gn.access_count,
            accessed_at: gn.accessed_at,
        },
        body: gn.body,
    }
}

/// Convert Edge → GraphEdge.
pub fn edge_to_graph(edge: Edge) -> llm_kernel::graph::types::GraphEdge {
    llm_kernel::graph::types::GraphEdge {
        id: edge.id,
        source: edge.source,
        target: edge.target,
        relation: edge.relation,
        weight: edge.weight,
        ts: edge.ts,
    }
}

/// Convert GraphEdge → Edge.
pub fn graph_to_edge(ge: llm_kernel::graph::types::GraphEdge) -> Edge {
    Edge {
        id: ge.id,
        source: ge.source,
        target: ge.target,
        relation: ge.relation,
        weight: ge.weight,
        ts: ge.ts,
    }
}
