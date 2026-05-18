mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new().expect("Failed to initialize app state"))
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
