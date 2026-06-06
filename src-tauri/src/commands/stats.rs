use epic_harness::mem::graph::compute_stats_pool;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let pool = state.db.clone();
    compute_stats_pool(&pool)
        .await
        .map_err(|e| format!("failed to compute stats: {e}"))
}
