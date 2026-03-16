//! Minimal formal-only example that intentionally reaches a deadlock.

pub mod model;

pub use model::{
    DeadlockAction, DeadlockSpec, DeadlockState, DeadlockSummary, deadlock_summary, format_summary,
    lower_spec,
};
