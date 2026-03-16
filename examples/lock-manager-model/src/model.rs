use nirvash::{BoolExpr, TransitionProgram};
use nirvash_conformance::SpecOracle;
use nirvash_lower::{FrontendSpec, TemporalSpec};
use nirvash_macros::{
    FiniteModelDomain as FormalFiniteModelDomain, code_tests, nirvash_expr,
    nirvash_transition_program,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FormalFiniteModelDomain)]
pub enum Client {
    Alice,
    Bob,
}

impl Client {
    pub fn label(self) -> &'static str {
        match self {
            Self::Alice => "alice",
            Self::Bob => "bob",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, FormalFiniteModelDomain,
)]
pub enum ClientPhase {
    #[default]
    Idle,
    Waiting,
    Holding,
}

impl ClientPhase {
    pub fn is_holding(self) -> bool {
        matches!(self, Self::Holding)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, FormalFiniteModelDomain,
)]
pub struct LockState {
    pub alice: ClientPhase,
    pub bob: ClientPhase,
}

impl LockState {
    pub fn phase(self, client: Client) -> ClientPhase {
        match client {
            Client::Alice => self.alice,
            Client::Bob => self.bob,
        }
    }

    pub fn holder_count(self) -> usize {
        usize::from(self.alice.is_holding()) + usize::from(self.bob.is_holding())
    }

    pub fn is_handoff_target(self) -> bool {
        matches!(self.alice, ClientPhase::Idle) && matches!(self.bob, ClientPhase::Holding)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FormalFiniteModelDomain)]
pub enum LockAction {
    Request(Client),
    Grant(Client),
    Release(Client),
}

impl LockAction {
    pub fn label(self) -> String {
        match self {
            Self::Request(client) => format!("{} requests lock", client.label()),
            Self::Grant(client) => format!("{} granted lock", client.label()),
            Self::Release(client) => format!("{} releases lock", client.label()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockOutput {
    Applied,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSummary {
    pub spec_name: &'static str,
    pub reachable_states: usize,
    pub holding_states: usize,
    pub plan: Vec<LockAction>,
    pub target_state: LockState,
}

#[derive(Debug, Clone, Copy, Default)]
#[code_tests(
    profiles = [
        smoke_default = {
            coverage = [transitions],
            engines = [explicit_suite],
        },
        boundary_default = {
            coverage = [guard_boundaries],
            engines = [explicit_suite, proptest_online(cases = 64, steps = 4)],
        },
        unit_default = {
            coverage = [transitions, transition_pairs(2), guard_boundaries],
            engines = [explicit_suite, proptest_online(cases = 128, steps = 8)],
        },
        e2e_default = {
            coverage = [transitions],
            engines = [trace_validation],
        },
        concurrency_default = {
            coverage = [transitions, transition_pairs(2)],
            engines = [
                loom_small(threads = 2, max_permutations = 8),
                shuttle_pct(depth = 2, runs = 64),
            ],
        },
    ],
)]
pub struct LockManagerSpec;

impl FrontendSpec for LockManagerSpec {
    type State = LockState;
    type Action = LockAction;

    fn frontend_name(&self) -> &'static str {
        "lock_manager"
    }

    fn initial_states(&self) -> Vec<Self::State> {
        vec![LockState::default()]
    }

    fn actions(&self) -> Vec<Self::Action> {
        vec![
            LockAction::Request(Client::Alice),
            LockAction::Grant(Client::Alice),
            LockAction::Release(Client::Alice),
            LockAction::Request(Client::Bob),
            LockAction::Grant(Client::Bob),
            LockAction::Release(Client::Bob),
        ]
    }

    fn transition_program(&self) -> Option<TransitionProgram<Self::State, Self::Action>> {
        Some(nirvash_transition_program! {
            rule request_alice
                when matches!(action, LockAction::Request(Client::Alice))
                    && matches!(prev.alice, ClientPhase::Idle) => {
                set alice <= ClientPhase::Waiting;
            }

            rule grant_alice
                when matches!(action, LockAction::Grant(Client::Alice))
                    && matches!(prev.alice, ClientPhase::Waiting)
                    && !matches!(prev.bob, ClientPhase::Holding) => {
                set alice <= ClientPhase::Holding;
            }

            rule release_alice
                when matches!(action, LockAction::Release(Client::Alice))
                    && matches!(prev.alice, ClientPhase::Holding) => {
                set alice <= ClientPhase::Idle;
            }

            rule request_bob
                when matches!(action, LockAction::Request(Client::Bob))
                    && matches!(prev.bob, ClientPhase::Idle) => {
                set bob <= ClientPhase::Waiting;
            }

            rule grant_bob
                when matches!(action, LockAction::Grant(Client::Bob))
                    && matches!(prev.bob, ClientPhase::Waiting)
                    && !matches!(prev.alice, ClientPhase::Holding) => {
                set bob <= ClientPhase::Holding;
            }

            rule release_bob
                when matches!(action, LockAction::Release(Client::Bob))
                    && matches!(prev.bob, ClientPhase::Holding) => {
                set bob <= ClientPhase::Idle;
            }
        })
    }
}

impl TemporalSpec for LockManagerSpec {
    fn invariants(&self) -> Vec<BoolExpr<Self::State>> {
        vec![nirvash_expr!(mutual_exclusion(state) =>
            !(matches!(state.alice, ClientPhase::Holding)
                && matches!(state.bob, ClientPhase::Holding))
        )]
    }
}

impl SpecOracle for LockManagerSpec {
    type ExpectedOutput = LockOutput;

    fn expected_output(
        &self,
        _prev: &Self::State,
        _action: &Self::Action,
        next: Option<&Self::State>,
    ) -> Self::ExpectedOutput {
        if next.is_some() {
            LockOutput::Applied
        } else {
            LockOutput::Blocked
        }
    }
}

pub fn sample_handoff_plan() -> Vec<LockAction> {
    vec![
        LockAction::Request(Client::Alice),
        LockAction::Grant(Client::Alice),
        LockAction::Request(Client::Bob),
        LockAction::Release(Client::Alice),
        LockAction::Grant(Client::Bob),
    ]
}
