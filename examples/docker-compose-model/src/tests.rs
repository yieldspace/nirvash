use crate::{ComposeOutput, MockComposeRuntime, StackAction, format_summary, plan_summary};

#[test]
fn mock_runtime_blocks_web_until_dependencies_are_healthy() {
    let mut runtime = MockComposeRuntime::default();

    assert_eq!(
        runtime.apply(StackAction::CreateWeb),
        ComposeOutput::Blocked
    );

    assert_eq!(runtime.apply(StackAction::CreateDb), ComposeOutput::Applied);
    assert_eq!(runtime.apply(StackAction::StartDb), ComposeOutput::Applied);
    assert_eq!(
        runtime.apply(StackAction::PassDbHealth),
        ComposeOutput::Applied
    );
    assert_eq!(
        runtime.apply(StackAction::CreateWeb),
        ComposeOutput::Blocked
    );

    assert_eq!(
        runtime.apply(StackAction::CreateCache),
        ComposeOutput::Applied
    );
    assert_eq!(
        runtime.apply(StackAction::StartCache),
        ComposeOutput::Applied
    );
    assert_eq!(
        runtime.apply(StackAction::PassCacheHealth),
        ComposeOutput::Applied
    );
    assert_eq!(
        runtime.apply(StackAction::CreateWeb),
        ComposeOutput::Applied
    );
}

#[test]
fn plan_summary_reaches_ready_stack() {
    let summary = plan_summary();

    assert_eq!(summary.reachable_states, 10);
    assert_eq!(summary.ready_states, 1);
    assert_eq!(
        summary.plan,
        vec![
            StackAction::CreateDb,
            StackAction::StartDb,
            StackAction::PassDbHealth,
            StackAction::CreateCache,
            StackAction::StartCache,
            StackAction::PassCacheHealth,
            StackAction::CreateWeb,
            StackAction::StartWeb,
            StackAction::PassWebHealth,
        ]
    );
    assert!(summary.ready_state.is_ready());
}

#[test]
fn summary_format_mentions_compose_plan() {
    let summary = plan_summary();
    let rendered = format_summary(&summary);

    assert!(rendered.contains("compose up plan:"));
    assert!(rendered.contains("ready state: StackState"));
}
