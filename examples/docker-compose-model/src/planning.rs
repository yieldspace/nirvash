use std::collections::VecDeque;
use std::fmt::Write as _;

use nirvash::{ModelCheckConfig, ReachableGraphSnapshot};
use nirvash_check::ExplicitModelChecker;
use nirvash_lower::{FrontendSpec, LoweringCx};

use crate::model::{DockerComposeUpSpec, PlanSummary, StackAction, StackState};

pub fn lower_spec(
    spec: &DockerComposeUpSpec,
) -> nirvash_lower::LoweredSpec<'_, StackState, StackAction> {
    let mut lowering_cx = LoweringCx;
    spec.lower(&mut lowering_cx)
        .expect("docker compose example should lower")
}

pub fn find_ready_plan(
    snapshot: &ReachableGraphSnapshot<StackState, StackAction>,
) -> Option<Vec<StackAction>> {
    let target = snapshot.states.iter().position(|state| state.is_ready())?;
    let mut queue = VecDeque::new();
    let mut parents: Vec<Option<(usize, StackAction)>> = vec![None; snapshot.states.len()];
    let mut visited = vec![false; snapshot.states.len()];

    for &initial in &snapshot.initial_indices {
        visited[initial] = true;
        queue.push_back(initial);
    }

    while let Some(current) = queue.pop_front() {
        if current == target {
            break;
        }

        for edge in &snapshot.edges[current] {
            if visited[edge.target] {
                continue;
            }
            visited[edge.target] = true;
            parents[edge.target] = Some((current, edge.action));
            queue.push_back(edge.target);
        }
    }

    if !visited[target] {
        return None;
    }

    let mut actions = Vec::new();
    let mut cursor = target;
    while let Some((prev, action)) = parents[cursor] {
        actions.push(action);
        cursor = prev;
    }
    actions.reverse();
    Some(actions)
}

pub fn plan_summary() -> PlanSummary {
    let spec = DockerComposeUpSpec;
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
        "dependency invariants must hold: {:?}",
        invariant_result.violations()
    );
    assert!(
        !snapshot.truncated,
        "reachable graph should be complete for this small example"
    );
    assert!(
        snapshot.deadlocks.is_empty(),
        "steady action keeps the ready state from deadlocking"
    );

    let plan = find_ready_plan(&snapshot).expect("ready state must be reachable");
    let ready_state = snapshot
        .states
        .iter()
        .copied()
        .find(|state| state.is_ready())
        .expect("ready state must exist");

    PlanSummary {
        spec_name: spec.frontend_name(),
        reachable_states: snapshot.states.len(),
        ready_states: snapshot
            .states
            .iter()
            .filter(|state| state.is_ready())
            .count(),
        ready_state,
        plan,
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
    writeln!(&mut output, "ready states: {}", summary.ready_states).expect("write to string");
    writeln!(&mut output, "compose up plan:").expect("write to string");
    for (index, action) in summary.plan.iter().enumerate() {
        writeln!(&mut output, "  {}. {}", index + 1, action.label()).expect("write to string");
    }
    writeln!(&mut output, "ready state: {:?}", summary.ready_state).expect("write to string");
    output
}
