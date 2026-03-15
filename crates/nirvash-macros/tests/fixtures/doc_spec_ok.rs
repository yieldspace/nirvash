use nirvash_lower::FrontendSpec;
use nirvash_macros::{
    FiniteModelDomain as FormalFiniteModelDomain, doc_spec, nirvash_transition_program,
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

fn spec_cases() -> Vec<Spec> {
    vec![Spec]
}

#[doc_spec(cases(spec_cases), reachability(auto_if_finite))]
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

fn main() {
    let bundle = nirvash::collect_transition_doc_bundles()
        .into_iter()
        .find(|bundle| bundle.spec_name == "Spec")
        .expect("transition doc bundle");
    assert_eq!(bundle.structure_cases.len(), 1);
}
