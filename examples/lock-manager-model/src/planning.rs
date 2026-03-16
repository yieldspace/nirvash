use std::fmt::Write as _;

use nirvash::{ModelCheckConfig, ReachableGraphSnapshot};
use nirvash_check::ExplicitModelChecker;
use nirvash_lower::{FrontendSpec, LoweringCx};

use crate::model::{LockAction, LockManagerSpec, LockState, PlanSummary, sample_handoff_plan};

pub fn lower_spec(spec: &LockManagerSpec) -> nirvash_lower::LoweredSpec<'_, LockState, LockAction> {
    let mut lowering_cx = LoweringCx;
    spec.lower(&mut lowering_cx)
        .expect("lock manager example should lower")
}

fn follow_plan(
    snapshot: &ReachableGraphSnapshot<LockState, LockAction>,
    plan: &[LockAction],
) -> Option<LockState> {
    let mut current = *snapshot.initial_indices.first()?;

    for action in plan {
        let edge = snapshot.edges[current]
            .iter()
            .find(|edge| edge.action == *action)?;
        current = edge.target;
    }

    snapshot.states.get(current).copied()
}

pub fn plan_summary() -> PlanSummary {
    let spec = LockManagerSpec;
    let lowered = lower_spec(&spec);
    let checker = ExplicitModelChecker::with_config(&lowered, ModelCheckConfig::reachable_graph());

    let snapshot = checker
        .full_reachable_graph_snapshot()
        .expect("reachable graph should build");
    let invariant_result = checker
        .check_invariants()
        .expect("invariants should be checkable");

    assert!(
        invariant_result.is_ok(),
        "mutual exclusion must hold: {:?}",
        invariant_result.violations()
    );
    assert!(
        !snapshot.truncated,
        "reachable graph should be complete for this small example"
    );
    assert!(
        snapshot.deadlocks.is_empty(),
        "request, grant, or release should always keep the system moving"
    );

    let plan = sample_handoff_plan();
    let target_state = follow_plan(&snapshot, &plan).expect("handoff path must be reachable");
    assert!(
        target_state.is_handoff_target(),
        "sample plan should end with bob holding the lock"
    );

    PlanSummary {
        spec_name: spec.frontend_name(),
        reachable_states: snapshot.states.len(),
        holding_states: snapshot
            .states
            .iter()
            .filter(|state| state.holder_count() > 0)
            .count(),
        plan,
        target_state,
    }
}

pub fn format_summary(summary: &PlanSummary) -> String {
    let mut output = String::new();
    writeln!(&mut output, "spec: {}", summary.spec_name).expect("write to string");
    writeln!(
        &mut output,
        "reachable states: {}",
        summary.reachable_states
    )
    .expect("write to string");
    writeln!(&mut output, "holding states: {}", summary.holding_states).expect("write to string");
    writeln!(&mut output, "sample handoff plan:").expect("write to string");
    for (index, action) in summary.plan.iter().enumerate() {
        writeln!(&mut output, "  {}. {}", index + 1, action.label()).expect("write to string");
    }
    writeln!(&mut output, "target state: {:?}", summary.target_state).expect("write to string");
    output
}
