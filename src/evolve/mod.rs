pub mod analysis;
pub mod ingest;
pub mod instincts;
pub mod metrics;
pub mod skills;

pub use analysis::{analyze_session, build_summary, detect_patterns};
pub use ingest::ingest_to_memory;
pub use instincts::{extract_instincts, promote_instincts_to_global};
pub use metrics::{check_stagnation, compute_trend, safe_avg_score, update_skill_attribution};
pub use skills::{export_to_global, gate_skills, seed_smart_skills, write_workspace_manifest};
