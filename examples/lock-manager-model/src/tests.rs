use crate::{
    Client, ClientPhase, LockAction, LockOutput, LockState, MockLockManager, format_summary,
    plan_summary, sample_handoff_plan,
};

#[test]
fn release_without_ownership_is_blocked() {
    let mut runtime = MockLockManager::default();

    assert_eq!(
        runtime.apply(LockAction::Release(Client::Alice)),
        LockOutput::Blocked
    );
    assert_eq!(
        runtime.apply(LockAction::Release(Client::Bob)),
        LockOutput::Blocked
    );
}

#[test]
fn second_client_cannot_hold_simultaneously() {
    let mut runtime = MockLockManager::default();

    assert_eq!(
        runtime.apply(LockAction::Request(Client::Alice)),
        LockOutput::Applied
    );
    assert_eq!(
        runtime.apply(LockAction::Grant(Client::Alice)),
        LockOutput::Applied
    );
    assert_eq!(
        runtime.apply(LockAction::Request(Client::Bob)),
        LockOutput::Applied
    );
    assert_eq!(
        runtime.apply(LockAction::Grant(Client::Bob)),
        LockOutput::Blocked
    );
    assert_eq!(
        runtime.state(),
        LockState {
            alice: ClientPhase::Holding,
            bob: ClientPhase::Waiting,
        }
    );
}

#[test]
fn plan_summary_finds_handoff_path() {
    let summary = plan_summary();
    let rendered = format_summary(&summary);

    assert_eq!(summary.reachable_states, 8);
    assert_eq!(summary.holding_states, 4);
    assert_eq!(summary.plan, sample_handoff_plan());
    assert_eq!(
        summary.target_state,
        LockState {
            alice: ClientPhase::Idle,
            bob: ClientPhase::Holding,
        }
    );
    assert!(rendered.contains("sample handoff plan:"));
    assert!(rendered.contains("bob granted lock"));
}
