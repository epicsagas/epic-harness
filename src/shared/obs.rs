use serde::{Deserialize, Serialize};

use super::scoring::ScoreDimensions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsRecord {
    pub timestamp: String,
    pub tool: String,
    pub tool_category: String,
    pub action: Option<String>,
    pub result: Option<String>,
    pub score: Option<f64>,
    pub dimensions: Option<ScoreDimensions>,
    pub failure_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_ext: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<String>,
}
