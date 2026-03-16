use nirvash_conformance::TraceSink;
#[allow(unused_imports)]
use nirvash_macros::{
    nirvash, nirvash_binding, nirvash_project, nirvash_project_output, nirvash_trace,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Client, ClientPhase, LockAction, LockManagerSpec, LockOutput, LockState};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MockLockManager {
    state: LockState,
}

impl MockLockManager {
    pub fn state(&self) -> LockState {
        self.state
    }

    pub fn apply(&mut self, action: LockAction) -> LockOutput {
        match action {
            LockAction::Request(Client::Alice) if matches!(self.state.alice, ClientPhase::Idle) => {
                self.state.alice = ClientPhase::Waiting;
                LockOutput::Applied
            }
            LockAction::Grant(Client::Alice)
                if matches!(self.state.alice, ClientPhase::Waiting)
                    && !matches!(self.state.bob, ClientPhase::Holding) =>
            {
                self.state.alice = ClientPhase::Holding;
                LockOutput::Applied
            }
            LockAction::Release(Client::Alice)
                if matches!(self.state.alice, ClientPhase::Holding) =>
            {
                self.state.alice = ClientPhase::Idle;
                LockOutput::Applied
            }
            LockAction::Request(Client::Bob) if matches!(self.state.bob, ClientPhase::Idle) => {
                self.state.bob = ClientPhase::Waiting;
                LockOutput::Applied
            }
            LockAction::Grant(Client::Bob)
                if matches!(self.state.bob, ClientPhase::Waiting)
                    && !matches!(self.state.alice, ClientPhase::Holding) =>
            {
                self.state.bob = ClientPhase::Holding;
                LockOutput::Applied
            }
            LockAction::Release(Client::Bob) if matches!(self.state.bob, ClientPhase::Holding) => {
                self.state.bob = ClientPhase::Idle;
                LockOutput::Applied
            }
            _ => LockOutput::Blocked,
        }
    }
}

#[nirvash_binding(spec = crate::model::LockManagerSpec, concurrent)]
impl MockLockManager {
    #[nirvash(action = LockAction::Request)]
    fn request(&mut self, client: Client) -> LockOutput {
        self.apply(LockAction::Request(client))
    }

    #[nirvash(action = LockAction::Grant)]
    fn grant(&mut self, client: Client) -> LockOutput {
        self.apply(LockAction::Grant(client))
    }

    #[nirvash(action = LockAction::Release)]
    fn release(&mut self, client: Client) -> LockOutput {
        self.apply(LockAction::Release(client))
    }

    #[nirvash_project]
    fn project(&self) -> LockState {
        self.state
    }

    #[nirvash_project_output]
    fn project_output(_action: &LockAction, output: &LockOutput) -> LockOutput {
        output.clone()
    }

    #[nirvash_trace]
    fn trace(&self, _output: &LockOutput, sink: &mut dyn TraceSink<LockManagerSpec>) {
        sink.record_update("alice", Value::String(format!("{:?}", self.state.alice)));
        sink.record_update("bob", Value::String(format!("{:?}", self.state.bob)));
    }
}
