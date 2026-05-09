use crate::state::AppState;
use epic_harness::mem::graph::compute_stats_conn;
use tauri::State;

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let conn = db
            .lock()
            .map_err(|e| format!("database temporarily unavailable: {e}"))?;
        compute_stats_conn(&conn).map_err(|e| format!("failed to compute stats: {e}"))
    })
    .await
    .map_err(|e| format!("operation cancelled: {e}"))?
}
