use crate::state::AppState;
use epic_harness::mem::store::{
    delete_node_file_conn, importance_for_type, new_uuid, now_iso, read_all_nodes_conn,
    read_node_conn, remove_edges_for_node_conn, validate_uuid, write_node_conn, Node,
    NodeFrontmatter,
};

const VALID_NODE_TYPES: &[&str] = &[
    "decision", "resolution", "psychographic", "instinct", "concept",
    "project", "pattern", "error", "session",
];
const MAX_TITLE_LEN: usize = 512;
const MAX_BODY_LEN: usize = 1_000_000;
const MAX_TAGS: usize = 50;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize)]
pub struct NodeResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    pub tags: Vec<String>,
    pub projects: Vec<String>,
    pub updated: String,
}

#[derive(Serialize)]
pub struct NodeDetailResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub title: String,
    pub tags: Vec<String>,
    pub projects: Vec<String>,
    pub agents: Vec<String>,
    pub created: String,
    pub updated: String,
    pub importance: f64,
    pub access_count: i64,
    pub body: String,
}

#[derive(Deserialize)]
pub struct CreateNodeInput {
    pub title: String,
    #[serde(rename = "type", default)]
    pub node_type: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub projects: Option<Vec<String>>,
    pub importance: Option<f64>,
}

#[derive(Deserialize)]
pub struct UpdateNodeInput {
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub node_type: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
    pub importance: Option<f64>,
}

impl From<&Node> for NodeResponse {
    fn from(n: &Node) -> Self {
        Self {
            id: n.frontmatter.id.clone(),
            node_type: n.frontmatter.node_type.clone(),
            title: n.frontmatter.title.clone(),
            tags: n.frontmatter.tags.clone(),
            projects: n.frontmatter.projects.clone(),
            updated: n.frontmatter.updated.clone(),
        }
    }
}

impl From<Node> for NodeDetailResponse {
    fn from(n: Node) -> Self {
        Self {
            id: n.frontmatter.id.clone(),
            node_type: n.frontmatter.node_type.clone(),
            title: n.frontmatter.title.clone(),
            tags: n.frontmatter.tags.clone(),
            projects: n.frontmatter.projects.clone(),
            agents: n.frontmatter.agents.clone(),
            created: n.frontmatter.created.clone(),
            updated: n.frontmatter.updated.clone(),
            importance: n.frontmatter.importance,
            access_count: n.frontmatter.access_count,
            body: n.body.clone(),
        }
    }
}

#[tauri::command]
pub fn get_nodes(state: State<'_, AppState>) -> Result<Vec<NodeResponse>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let nodes = read_all_nodes_conn(&conn);
    Ok(nodes.iter().map(NodeResponse::from).collect())
}

#[tauri::command]
pub fn get_node(id: String, state: State<'_, AppState>) -> Result<NodeDetailResponse, String> {
    if !validate_uuid(&id) {
        return Err("invalid node id".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let node = read_node_conn(&conn, &id).map_err(|e| e.to_string())?;
    Ok(NodeDetailResponse::from(node))
}

#[tauri::command]
pub fn create_node(input: CreateNodeInput, state: State<'_, AppState>) -> Result<String, String> {
    let node_type = input.node_type.unwrap_or_else(|| "concept".into());

    // Fix 1: node_type allowlist validation
    if !VALID_NODE_TYPES.contains(&node_type.as_str()) {
        return Err(format!(
            "invalid node_type '{node_type}': must be one of {}",
            VALID_NODE_TYPES.join(", ")
        ));
    }

    // Fix 2: input length limits
    if input.title.len() > MAX_TITLE_LEN {
        return Err(format!("title exceeds max length of {MAX_TITLE_LEN} characters"));
    }
    if let Some(ref body) = input.body {
        if body.len() > MAX_BODY_LEN {
            return Err(format!("body exceeds max length of {MAX_BODY_LEN} characters"));
        }
    }
    if let Some(ref tags) = input.tags {
        if tags.len() > MAX_TAGS {
            return Err(format!("tags array exceeds max of {MAX_TAGS} entries"));
        }
    }

    let importance = input
        .importance
        .unwrap_or_else(|| importance_for_type(&node_type))
        .clamp(0.0, 1.0);
    let node = Node {
        frontmatter: NodeFrontmatter {
            id: new_uuid(),
            node_type: node_type.clone(),
            title: if input.title.trim().is_empty() {
                "Untitled".into()
            } else {
                input.title.trim().to_string()
            },
            tags: input.tags.unwrap_or_default(),
            projects: input.projects.unwrap_or_default(),
            agents: vec![],
            created: now_iso(),
            updated: now_iso(),
            importance,
            access_count: 0,
            accessed_at: String::new(),
        },
        body: input.body.unwrap_or_default(),
    };
    let id = node.frontmatter.id.clone();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    write_node_conn(&conn, &node).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn update_node(id: String, input: UpdateNodeInput, state: State<'_, AppState>) -> Result<String, String> {
    if !validate_uuid(&id) {
        return Err("invalid node id".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut node = read_node_conn(&conn, &id).map_err(|e| e.to_string())?;
    if let Some(t) = input.title {
        if t.len() > MAX_TITLE_LEN {
            return Err(format!("title exceeds max length of {MAX_TITLE_LEN} characters"));
        }
        node.frontmatter.title = t;
    }
    if let Some(t) = input.node_type {
        if !VALID_NODE_TYPES.contains(&t.as_str()) {
            return Err(format!(
                "invalid node_type: must be one of {}",
                VALID_NODE_TYPES.join(", ")
            ));
        }
        node.frontmatter.node_type = t;
    }
    if let Some(b) = input.body {
        if b.len() > MAX_BODY_LEN {
            return Err(format!("body exceeds max length of {MAX_BODY_LEN} characters"));
        }
        node.body = b;
    }
    if let Some(tags) = input.tags {
        if tags.len() > MAX_TAGS {
            return Err(format!("tags array exceeds max of {MAX_TAGS} entries"));
        }
        node.frontmatter.tags = tags;
    }
    if let Some(imp) = input.importance {
        node.frontmatter.importance = imp.clamp(0.0, 1.0);
    }
    node.frontmatter.updated = now_iso();
    write_node_conn(&conn, &node).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn delete_node(id: String, state: State<'_, AppState>) -> Result<String, String> {
    if !validate_uuid(&id) {
        return Err("invalid node id".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    remove_edges_for_node_conn(&conn, &id).map_err(|e| e.to_string())?;
    delete_node_file_conn(&conn, &id).map_err(|e| e.to_string())?;
    Ok(id)
}
