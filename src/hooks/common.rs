// Re-export all shared types and functions.
// This preserves backward compatibility with `use super::common::*` in hook modules
// and `crate::hooks::common::*` references across the crate.
pub use crate::shared::*;
