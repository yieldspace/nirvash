use nirvash_conformance::TraceSink;
#[allow(unused_imports)]
use nirvash_macros::{
    nirvash, nirvash_binding, nirvash_project, nirvash_project_output, nirvash_trace,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{ComposeOutput, DockerComposeUpSpec, ServicePhase, StackAction, StackState};

/// A mock compose runtime whose behavior is checked against the model.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MockComposeRuntime {
    state: StackState,
}

impl MockComposeRuntime {
    pub fn state(&self) -> StackState {
        self.state
    }

    pub fn apply(&mut self, action: StackAction) -> ComposeOutput {
        match action {
            StackAction::CreateDb if matches!(self.state.db, ServicePhase::Missing) => {
                self.state.db = ServicePhase::Created;
                ComposeOutput::Applied
            }
            StackAction::StartDb if matches!(self.state.db, ServicePhase::Created) => {
                self.state.db = ServicePhase::Running;
                ComposeOutput::Applied
            }
            StackAction::PassDbHealth if matches!(self.state.db, ServicePhase::Running) => {
                self.state.db = ServicePhase::Healthy;
                ComposeOutput::Applied
            }
            StackAction::CreateCache
                if matches!(self.state.db, ServicePhase::Healthy)
                    && matches!(self.state.cache, ServicePhase::Missing) =>
            {
                self.state.cache = ServicePhase::Created;
                ComposeOutput::Applied
            }
            StackAction::StartCache if matches!(self.state.cache, ServicePhase::Created) => {
                self.state.cache = ServicePhase::Running;
                ComposeOutput::Applied
            }
            StackAction::PassCacheHealth if matches!(self.state.cache, ServicePhase::Running) => {
                self.state.cache = ServicePhase::Healthy;
                ComposeOutput::Applied
            }
            StackAction::CreateWeb
                if matches!(self.state.db, ServicePhase::Healthy)
                    && matches!(self.state.cache, ServicePhase::Healthy)
                    && matches!(self.state.web, ServicePhase::Missing) =>
            {
                self.state.web = ServicePhase::Created;
                ComposeOutput::Applied
            }
            StackAction::StartWeb if matches!(self.state.web, ServicePhase::Created) => {
                self.state.web = ServicePhase::Running;
                ComposeOutput::Applied
            }
            StackAction::PassWebHealth if matches!(self.state.web, ServicePhase::Running) => {
                self.state.web = ServicePhase::Healthy;
                ComposeOutput::Applied
            }
            StackAction::Steady if self.state.is_ready() => ComposeOutput::Applied,
            _ => ComposeOutput::Blocked,
        }
    }
}

#[nirvash_binding(spec = crate::model::DockerComposeUpSpec)]
impl MockComposeRuntime {
    #[nirvash(action = StackAction::CreateDb)]
    fn create_db(&mut self) -> ComposeOutput {
        self.apply(StackAction::CreateDb)
    }

    #[nirvash(action = StackAction::StartDb)]
    fn start_db(&mut self) -> ComposeOutput {
        self.apply(StackAction::StartDb)
    }

    #[nirvash(action = StackAction::PassDbHealth)]
    fn pass_db_health(&mut self) -> ComposeOutput {
        self.apply(StackAction::PassDbHealth)
    }

    #[nirvash(action = StackAction::CreateCache)]
    fn create_cache(&mut self) -> ComposeOutput {
        self.apply(StackAction::CreateCache)
    }

    #[nirvash(action = StackAction::StartCache)]
    fn start_cache(&mut self) -> ComposeOutput {
        self.apply(StackAction::StartCache)
    }

    #[nirvash(action = StackAction::PassCacheHealth)]
    fn pass_cache_health(&mut self) -> ComposeOutput {
        self.apply(StackAction::PassCacheHealth)
    }

    #[nirvash(action = StackAction::CreateWeb)]
    fn create_web(&mut self) -> ComposeOutput {
        self.apply(StackAction::CreateWeb)
    }

    #[nirvash(action = StackAction::StartWeb)]
    fn start_web(&mut self) -> ComposeOutput {
        self.apply(StackAction::StartWeb)
    }

    #[nirvash(action = StackAction::PassWebHealth)]
    fn pass_web_health(&mut self) -> ComposeOutput {
        self.apply(StackAction::PassWebHealth)
    }

    #[nirvash(action = StackAction::Steady)]
    fn steady(&mut self) -> ComposeOutput {
        self.apply(StackAction::Steady)
    }

    #[nirvash_project]
    fn project(&self) -> StackState {
        self.state
    }

    #[nirvash_project_output]
    fn project_output(_action: &StackAction, output: &ComposeOutput) -> ComposeOutput {
        output.clone()
    }

    #[nirvash_trace]
    fn trace(&self, _output: &ComposeOutput, sink: &mut dyn TraceSink<DockerComposeUpSpec>) {
        sink.record_update("db", Value::String(format!("{:?}", self.state.db)));
        sink.record_update("cache", Value::String(format!("{:?}", self.state.cache)));
        sink.record_update("web", Value::String(format!("{:?}", self.state.web)));
    }
}
