//! Backend-neutral workflow child execution contract.
use crate::HostError;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use xai_tool_types::{SubagentCapabilityMode, SubagentIsolationMode};

/// Outcome of draining workflow child agents after cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostDrainOutcome {
    Drained,
    TimedOut,
}

/// One host-level agent execution request (after host validation / contract wrap).
#[derive(Debug, Clone)]
pub struct WorkflowAgentSpawnRequest {
    pub id: String,
    pub prompt: String,
    pub description: String,
    pub subagent_type: String,
    pub parent_session_id: String,
    pub resume_from: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub capability_mode: Option<SubagentCapabilityMode>,
    pub isolation: Option<SubagentIsolationMode>,
    pub fork_context: bool,
    pub run_id: String,
    pub cancel_token: CancellationToken,
}

pub use xai_tool_types::workflow::WorkflowAgentSpawnResult;

/// Spawn / cancel children for a workflow run.
#[async_trait]
pub trait WorkflowAgentBackend: Send + Sync {
    async fn spawn_and_await(
        &self,
        request: WorkflowAgentSpawnRequest,
    ) -> Result<WorkflowAgentSpawnResult, HostError>;

    async fn cancel_run_children(&self, run_id: &str) -> HostDrainOutcome;

    /// Best-effort fire-and-forget cancel (pause/stop paths).
    fn request_cancel_run_children(&self, _run_id: &str) -> bool {
        false
    }
}
