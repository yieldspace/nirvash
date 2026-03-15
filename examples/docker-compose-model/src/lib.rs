#![allow(rustdoc::invalid_html_tags)]

//! A compact example that combines a formal model, a mock runtime, generated
//! `nirvash` tests, and rustdoc docgen output in one crate.

pub mod model;
pub mod planning;
pub mod runtime;

#[cfg(test)]
mod tests;

pub use model::{
    ComposeOutput, DockerComposeUpSpec, PlanSummary, ServicePhase, StackAction, StackState,
};
pub use planning::{find_ready_plan, format_summary, lower_spec, plan_summary};
pub use runtime::MockComposeRuntime;

#[cfg(test)]
use crate::model::generated;

nirvash::import_generated_tests!(
    spec = crate::model::DockerComposeUpSpec,
    binding = crate::runtime::MockComposeRuntime,
    profiles = [crate::model::generated::profiles::smoke_default()],
);

crate::model::generated::install::trace_tests!(binding = crate::runtime::MockComposeRuntime);
