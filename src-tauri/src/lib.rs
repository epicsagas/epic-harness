mod commands;
mod state;

use state::AppState;

/// Application entry point.
///
/// # Error handling
///
/// Fatal startup errors (e.g. corrupted DB, missing harness dir) are reported
/// to the user via an osascript dialog on macOS, then the process exits with
/// code 1. This is intentional: the Tauri event loop cannot start without valid
/// DB pools, so graceful shutdown is not possible.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = match AppState::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[epic-harness] FATAL: {e}");
            #[cfg(target_os = "macos")]
            {
                let safe_msg = e.to_string()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', " ")
                    .replace('\r', "");
                let _ = std::process::Command::new("osascript")
                    .args(["-e", &format!("display dialog \"Epic Harness failed to start:\\n\\n{safe_msg}\\n\\nCheck ~/.harness/ permissions.\" with title \"Epic Harness\" buttons {{\"OK\"}} default button \"OK\" with icon stop")])
                    .status();
            }
            std::process::exit(1);
        }
    };

    if let Err(e) = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Nodes
            commands::nodes::get_nodes,
            commands::nodes::get_node,
            commands::nodes::create_node,
            commands::nodes::update_node,
            commands::nodes::delete_node,
            // Edges
            commands::edges::get_edges,
            commands::edges::create_edge,
            commands::edges::delete_edge,
            // Graph
            commands::graph::get_graph,
            commands::graph::get_neighbors,
            // Search
            commands::search::search_nodes,
            commands::search::query_nodes,
            commands::search::recall_nodes,
            // Stats
            commands::stats::get_stats,
            // Harness live data
            commands::harness::get_harness_metrics,
            commands::harness::get_orbit_pipelines,
            commands::harness::get_evolved_skills,
            commands::harness::get_obs_summary,
            commands::harness::get_integration_status,
            commands::harness::get_session_snapshots,
            commands::harness::get_global_patterns,
            commands::harness::list_projects,
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("[epic-harness] FATAL: {e}");
        std::process::exit(1);
    }
}
