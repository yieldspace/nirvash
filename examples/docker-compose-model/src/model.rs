use nirvash::{BoolExpr, TransitionProgram};
use nirvash_conformance::SpecOracle;
use nirvash_lower::{FrontendSpec, ModelInstance};
use nirvash_macros::{
    FiniteModelDomain as FormalFiniteModelDomain, code_tests, doc_case, doc_spec, invariant,
    nirvash_expr, nirvash_transition_program, system_spec,
};
use serde::{Deserialize, Serialize};

/// Lifecycle state for a service in the compose stack.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, FormalFiniteModelDomain,
)]
pub enum ServicePhase {
    #[default]
    Missing,
    Created,
    Running,
    Healthy,
}

/// State of the three-service compose stack.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, FormalFiniteModelDomain,
)]
pub struct StackState {
    pub(crate) db: ServicePhase,
    pub(crate) cache: ServicePhase,
    pub(crate) web: ServicePhase,
}

impl StackState {
    pub fn is_ready(self) -> bool {
        matches!(self.db, ServicePhase::Healthy)
            && matches!(self.cache, ServicePhase::Healthy)
            && matches!(self.web, ServicePhase::Healthy)
    }
}

/// Actions supported by the model and mock runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FormalFiniteModelDomain)]
pub enum StackAction {
    CreateDb,
    StartDb,
    PassDbHealth,
    CreateCache,
    StartCache,
    PassCacheHealth,
    CreateWeb,
    StartWeb,
    PassWebHealth,
    Steady,
}

impl StackAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::CreateDb => "create db",
            Self::StartDb => "start db",
            Self::PassDbHealth => "db becomes healthy",
            Self::CreateCache => "create cache",
            Self::StartCache => "start cache",
            Self::PassCacheHealth => "cache becomes healthy",
            Self::CreateWeb => "create web",
            Self::StartWeb => "start web",
            Self::PassWebHealth => "web becomes healthy",
            Self::Steady => "steady reconcile",
        }
    }
}

/// Output returned by the mock compose runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComposeOutput {
    Applied,
    Blocked,
}

/// Summary returned by the example executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSummary {
    pub spec_name: &'static str,
    pub reachable_states: usize,
    pub ready_states: usize,
    pub ready_state: StackState,
    pub plan: Vec<StackAction>,
}

/// Model of `docker compose up` with dependency and health gates.
#[derive(Debug, Clone, Copy, Default)]
#[code_tests]
#[cfg_attr(
    doc,
    doc = include_str!(env!("NIRVASH_DOC_FRAGMENT_DOCKER_COMPOSE_UP_SPEC"))
)]
pub struct DockerComposeUpSpec;

#[doc_spec]
#[system_spec]
impl FrontendSpec for DockerComposeUpSpec {
    type State = StackState;
    type Action = StackAction;

    fn frontend_name(&self) -> &'static str {
        "docker_compose_up"
    }

    fn initial_states(&self) -> Vec<Self::State> {
        vec![StackState::default()]
    }

    fn actions(&self) -> Vec<Self::Action> {
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
            StackAction::Steady,
        ]
    }

    fn transition_program(&self) -> Option<TransitionProgram<Self::State, Self::Action>> {
        Some(nirvash_transition_program! {
            rule create_db
                when matches!(action, StackAction::CreateDb)
                    && matches!(prev.db, ServicePhase::Missing) => {
                set db <= ServicePhase::Created;
            }

            rule start_db
                when matches!(action, StackAction::StartDb)
                    && matches!(prev.db, ServicePhase::Created) => {
                set db <= ServicePhase::Running;
            }

            rule db_healthy
                when matches!(action, StackAction::PassDbHealth)
                    && matches!(prev.db, ServicePhase::Running) => {
                set db <= ServicePhase::Healthy;
            }

            rule create_cache
                when matches!(action, StackAction::CreateCache)
                    && matches!(prev.db, ServicePhase::Healthy)
                    && matches!(prev.cache, ServicePhase::Missing) => {
                set cache <= ServicePhase::Created;
            }

            rule start_cache
                when matches!(action, StackAction::StartCache)
                    && matches!(prev.cache, ServicePhase::Created) => {
                set cache <= ServicePhase::Running;
            }

            rule cache_healthy
                when matches!(action, StackAction::PassCacheHealth)
                    && matches!(prev.cache, ServicePhase::Running) => {
                set cache <= ServicePhase::Healthy;
            }

            rule create_web
                when matches!(action, StackAction::CreateWeb)
                    && matches!(prev.db, ServicePhase::Healthy)
                    && matches!(prev.cache, ServicePhase::Healthy)
                    && matches!(prev.web, ServicePhase::Missing) => {
                set web <= ServicePhase::Created;
            }

            rule start_web
                when matches!(action, StackAction::StartWeb)
                    && matches!(prev.web, ServicePhase::Created) => {
                set web <= ServicePhase::Running;
            }

            rule web_healthy
                when matches!(action, StackAction::PassWebHealth)
                    && matches!(prev.web, ServicePhase::Running) => {
                set web <= ServicePhase::Healthy;
            }

            rule steady
                when matches!(action, StackAction::Steady)
                    && matches!(prev.db, ServicePhase::Healthy)
                    && matches!(prev.cache, ServicePhase::Healthy)
                    && matches!(prev.web, ServicePhase::Healthy) => {
                set db <= prev.db;
                set cache <= prev.cache;
                set web <= prev.web;
            }
        })
    }
}

#[doc_case(spec = crate::model::DockerComposeUpSpec)]
fn docker_compose_doc_case() -> ModelInstance<StackState, StackAction> {
    ModelInstance::new("reachable_stack").with_doc_surface("compose_stack")
}

impl SpecOracle for DockerComposeUpSpec {
    type ExpectedOutput = ComposeOutput;

    fn expected_output(
        &self,
        _prev: &Self::State,
        _action: &Self::Action,
        next: Option<&Self::State>,
    ) -> Self::ExpectedOutput {
        if next.is_some() {
            ComposeOutput::Applied
        } else {
            ComposeOutput::Blocked
        }
    }
}

#[invariant(crate::model::DockerComposeUpSpec)]
fn cache_waits_for_db() -> BoolExpr<StackState> {
    nirvash_expr!(cache_waits_for_db(state) =>
        matches!(state.cache, ServicePhase::Missing)
            || matches!(state.db, ServicePhase::Healthy)
    )
}

#[invariant(crate::model::DockerComposeUpSpec)]
fn web_waits_for_dependencies() -> BoolExpr<StackState> {
    nirvash_expr!(web_waits_for_dependencies(state) =>
        matches!(state.web, ServicePhase::Missing)
            || (matches!(state.db, ServicePhase::Healthy)
                && matches!(state.cache, ServicePhase::Healthy))
    )
}
