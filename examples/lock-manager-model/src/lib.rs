//! A small example that models a lock manager, binds it to a mock runtime,
//! and imports generated `nirvash` tests.

pub mod model;
pub mod planning;
pub mod runtime;

#[cfg(test)]
mod tests;

pub use model::{
    Client, ClientPhase, LockAction, LockManagerSpec, LockOutput, LockState, PlanSummary,
    sample_handoff_plan,
};
pub use planning::{format_summary, lower_spec, plan_summary};
pub use runtime::MockLockManager;

#[allow(unused_imports)]
use crate::model::generated;

crate::model::generated::install::all_tests!(binding = crate::runtime::MockLockManager);
