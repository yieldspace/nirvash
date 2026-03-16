use std::fmt::Write as _;

use nirvash::{ModelCheckConfig, TransitionProgram};
use nirvash_check::ExplicitModelChecker;
use nirvash_lower::{FrontendSpec, LoweringCx};
use nirvash_macros::{
    FiniteModelDomain as FormalFiniteModelDomain, formal_tests, nirvash_transition_program,
    subsystem_spec,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, FormalFiniteModelDomain)]
pub enum DeadlockState {
    Start,
    Stuck,
}

impl DeadlockState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stuck => "stuck",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FormalFiniteModelDomain)]
pub enum DeadlockAction {
    Advance,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DeadlockSpec;

#[subsystem_spec]
impl FrontendSpec for DeadlockSpec {
    type State = DeadlockState;
    type Action = DeadlockAction;

    fn frontend_name(&self) -> &'static str {
        "deadlock_formal"
    }

    fn initial_states(&self) -> Vec<Self::State> {
        vec![DeadlockState::Start]
    }

    fn actions(&self) -> Vec<Self::Action> {
        vec![DeadlockAction::Advance]
    }

    fn transition_program(&self) -> Option<TransitionProgram<Self::State, Self::Action>> {
        Some(nirvash_transition_program! {
            rule advance
                when matches!(action, DeadlockAction::Advance)
                    && matches!(prev, DeadlockState::Start) => {
                set self <= DeadlockState::Stuck;
            }
        })
    }
}

#[formal_tests(spec = DeadlockSpec)]
const _: () = ();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlockSummary {
    pub spec_name: &'static str,
    pub reachable_states: usize,
    pub deadlock_states: Vec<DeadlockState>,
}

pub fn lower_spec(
    spec: &DeadlockSpec,
) -> nirvash_lower::LoweredSpec<'_, DeadlockState, DeadlockAction> {
    let mut lowering_cx = LoweringCx;
    spec.lower(&mut lowering_cx)
        .expect("deadlock formal example should lower")
}

pub fn deadlock_summary() -> DeadlockSummary {
    let spec = DeadlockSpec;
    let lowered = lower_spec(&spec);
    let snapshot = ExplicitModelChecker::with_config(&lowered, ModelCheckConfig::reachable_graph())
        .full_reachable_graph_snapshot()
        .expect("reachable graph should build");

    DeadlockSummary {
        spec_name: spec.frontend_name(),
        reachable_states: snapshot.states.len(),
        deadlock_states: snapshot
            .deadlocks
            .iter()
            .map(|&index| snapshot.states[index])
            .collect(),
    }
}

pub fn format_summary(summary: &DeadlockSummary) -> String {
    let mut output = String::new();
    writeln!(&mut output, "spec: {}", summary.spec_name).expect("write to string");
    writeln!(
        &mut output,
        "reachable states: {}",
        summary.reachable_states
    )
    .expect("write to string");
    writeln!(
        &mut output,
        "deadlock states: {}",
        summary.deadlock_states.len()
    )
    .expect("write to string");
    writeln!(&mut output, "deadlock targets:").expect("write to string");
    for (index, state) in summary.deadlock_states.iter().enumerate() {
        writeln!(&mut output, "  {}. {}", index + 1, state.label()).expect("write to string");
    }
    output
}
