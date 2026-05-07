use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScoreDimensions {
    pub tool_success: f64,
    pub output_quality: f64,
    pub execution_cost: f64,
}

impl Default for ScoreDimensions {
    fn default() -> Self {
        Self {
            tool_success: 0.0,
            output_quality: 0.0,
            execution_cost: 0.0,
        }
    }
}

pub fn compute_score(dims: &ScoreDimensions) -> f64 {
    let w = &crate::config::CONFIG.scoring.weights;
    let raw = w[0] * dims.tool_success + w[1] * dims.output_quality + w[2] * dims.execution_cost;
    (raw * 1000.0).round() / 1000.0
}
