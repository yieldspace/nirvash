use nirvash_lower::FrontendSpec;
use nirvash_macros::{
    FiniteModelDomain as FormalFiniteModelDomain, doc_case, doc_spec, nirvash_transition_program,
    subsystem_spec,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FormalFiniteModelDomain)]
struct State {
    busy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FormalFiniteModelDomain)]
enum Action {
    Start,
}

#[derive(Default)]
struct Spec;

#[doc_spec]
#[subsystem_spec]
impl FrontendSpec for Spec {
    type State = State;
    type Action = Action;

    fn frontend_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn initial_states(&self) -> Vec<Self::State> {
        vec![State { busy: false }]
    }

    fn actions(&self) -> Vec<Self::Action> {
        vec![Action::Start]
    }

    fn transition_program(&self) -> Option<::nirvash::TransitionProgram<Self::State, Self::Action>> {
        Some(nirvash_transition_program! {
            rule start when matches!(action, Action::Start) && !prev.busy => {
                set busy <= true;
            }
        })
    }
}

#[doc_case(spec = Spec)]
fn invalid_case(_unexpected: u8) -> nirvash_lower::ModelInstance<State, Action> {
    nirvash_lower::ModelInstance::default()
}

fn main() {}
