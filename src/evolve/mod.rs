pub mod analysis;
pub mod cli;
pub mod critic;
pub mod digester;
pub mod edits;
pub mod ingest;
pub mod metrics;
pub mod planner;
pub mod seesaw;
pub mod skills;
pub mod snapshot;
pub mod synthesis;
pub mod variants;

pub use analysis::{analyze_session, build_summary, detect_patterns};
pub use digester::digest_session;
pub use ingest::ingest_to_memory;
pub use metrics::{
    check_stagnation, classify_epoch, compute_trend, detect_reward_hacking, partition_holdout,
    safe_avg_score, update_skill_attribution,
};
pub use planner::{build_landscape, recommends_exploration};
pub use seesaw::{
    check as seesaw_check, load_registry, save_registry, scores_from_pipeline_digests,
};
pub use skills::{
    export_to_global, gate_skills, prune_rejected_buffer, seed_smart_skills, update_meta_field,
    write_workspace_manifest,
};
