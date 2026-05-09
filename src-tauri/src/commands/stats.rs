use crate::state::AppState;
use epic_harness::mem::graph::compute_stats_conn;
use tauri::State;

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    compute_stats_conn(&conn).map_err(|e| e.to_string())
}
