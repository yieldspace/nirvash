use nirvash_macros::{
    Signature as FormalSignature, legacy_formal_tests, legacy_invariant, legacy_subsystem_spec,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FormalSignature)]
enum State {
    Idle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, FormalSignature)]
enum Action {
    Tick,
}

struct Spec;

#[legacy_subsystem_spec]
impl nirvash::__private::TransitionSystem for Spec {
    type State = State;
    type Action = Action;

    fn initial_states(&self) -> Vec<Self::State> {
        vec![State::Idle]
    }

    fn actions(&self) -> Vec<Self::Action> {
        vec![Action::Tick]
    }

    fn transition(&self, _: &Self::State, _: &Self::Action) -> Option<Self::State> {
        None
    }
}

#[legacy_invariant(Spec)]
fn old_style_invariant() -> nirvash::BoolExpr<State> {
    nirvash::BoolExpr::new("old_style_invariant", |_| true)
}

#[legacy_formal_tests(spec = Spec)]
const _: () = ();

fn main() {}
