use nirvash::{TransitionDocReachabilityMode, collect_transition_doc_bundles};
use nirvash_lower::{FrontendSpec, ModelInstance};
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
struct DocSpec;

#[doc_spec]
#[subsystem_spec]
impl FrontendSpec for DocSpec {
    type State = State;
    type Action = Action;

    fn frontend_name(&self) -> &'static str {
        "doc_spec_registration"
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

#[doc_case(spec = DocSpec)]
fn focused_doc_case() -> ModelInstance<State, Action> {
    ModelInstance::new("focused").with_doc_surface("docs")
}

#[test]
fn doc_spec_registers_transition_doc_provider_without_formal_tests() {
    let bundle = collect_transition_doc_bundles()
        .into_iter()
        .find(|bundle| bundle.spec_name == "DocSpec")
        .expect("doc spec should register a transition doc provider");
    assert_eq!(
        bundle.metadata.reachability_mode,
        TransitionDocReachabilityMode::AutoIfFinite
    );
    assert_eq!(bundle.structure_cases.len(), 1);
    assert!(!bundle.reachability_cases.is_empty());
    assert_eq!(bundle.reachability_cases[0].label, "focused");
    assert_eq!(bundle.reachability_cases[0].surface.as_deref(), Some("docs"));
}
