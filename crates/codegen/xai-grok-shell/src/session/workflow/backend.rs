//! Pluggable agent spawn backend for workflow host.
//!
//! Grok default: `SubagentEvent` coordinator.
//! grok-pi: implement this trait to route `SpawnAgent` to Pi child sessions.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use xai_grok_tools::implementations::grok_build::task::types::{
    ModelOverrideProvenance, SubagentCancelRequest, SubagentCancelTarget, SubagentEvent,
    SubagentOwner, SubagentRequest, SubagentResult, SubagentRuntimeOverrides, SubagentSpawnRequest,
};
use xai_tool_types::{SubagentCapabilityMode, SubagentIsolationMode};
use xai_workflow::HostError;

pub use xai_workflow::backend::{
    HostDrainOutcome, WorkflowAgentBackend, WorkflowAgentSpawnRequest, WorkflowAgentSpawnResult,
};

/// Upstream Grok path: funnel through the existing subagent coordinator channel.
pub struct GrokSubagentBackend {
    pub subagent_event_tx: mpsc::UnboundedSender<SubagentEvent>,
    pub parent_session_id: String,
}

#[async_trait]
impl WorkflowAgentBackend for GrokSubagentBackend {
    async fn spawn_and_await(
        &self,
        request: WorkflowAgentSpawnRequest,
    ) -> Result<WorkflowAgentSpawnResult, HostError> {
        let (result_tx, result_rx) = oneshot::channel();
        let subagent_request = SubagentRequest {
            id: request.id,
            prompt: request.prompt,
            description: request.description,
            subagent_type: request.subagent_type,
            parent_session_id: request.parent_session_id,
            parent_prompt_id: None,
            resume_from: request.resume_from,
            cwd: None,
            runtime_overrides: SubagentRuntimeOverrides {
                model: request.model,
                reasoning_effort: request.reasoning_effort,
                output_token_budget: None,
                model_override_provenance: ModelOverrideProvenance::Tool,
                capability_mode: request.capability_mode,
                isolation: request.isolation,
                output_schema: None,
                ..Default::default()
            },
            run_in_background: false,
            surface_completion: false,
            await_to_completion: true,
            fork_context: request.fork_context,
            owner: SubagentOwner::workflow(&request.run_id),
            cancel_token: request.cancel_token,
            spawn_root: Default::default(),
        };

        if self
            .subagent_event_tx
            .send(SubagentEvent::Spawn(SubagentSpawnRequest {
                request: Box::new(subagent_request),
                result_tx,
                registered_tx: None,
            }))
            .is_err()
        {
            return Err(HostError::Failed(
                "subagent coordinator channel closed".into(),
            ));
        }

        let result = result_rx.await.map_err(|_| {
            HostError::Failed("subagent result channel closed before completion".into())
        })?;
        Ok(WorkflowAgentSpawnResult::from(result))
    }

    async fn cancel_run_children(&self, run_id: &str) -> HostDrainOutcome {
        let (respond_to, response) = oneshot::channel();
        if self
            .subagent_event_tx
            .send(SubagentEvent::Cancel(SubagentCancelRequest {
                parent_session_id: Some(self.parent_session_id.clone()),
                target: SubagentCancelTarget::WorkflowRunId(run_id.to_owned()),
                respond_to,
            }))
            .is_err()
        {
            tracing::warn!(%run_id, "workflow child cancellation channel closed");
            return HostDrainOutcome::TimedOut;
        }
        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
        if matches!(tokio::time::timeout(TIMEOUT, response).await, Ok(Ok(_))) {
            HostDrainOutcome::Drained
        } else {
            tracing::warn!(
                %run_id,
                timeout_ms = TIMEOUT.as_millis() as u64,
                "workflow child cancel/drain timed out"
            );
            HostDrainOutcome::TimedOut
        }
    }

    fn request_cancel_run_children(&self, run_id: &str) -> bool {
        let (respond_to, _response) = oneshot::channel();
        self.subagent_event_tx
            .send(SubagentEvent::Cancel(SubagentCancelRequest {
                parent_session_id: Some(self.parent_session_id.clone()),
                target: SubagentCancelTarget::WorkflowRunId(run_id.to_owned()),
                respond_to,
            }))
            .is_ok()
    }
}

/// Test / Pi-stub backend: returns canned success without Grok subagents.
pub struct MockWorkflowAgentBackend {
    pub output: Arc<str>,
}

#[async_trait]
impl WorkflowAgentBackend for MockWorkflowAgentBackend {
    async fn spawn_and_await(
        &self,
        request: WorkflowAgentSpawnRequest,
    ) -> Result<WorkflowAgentSpawnResult, HostError> {
        if request.cancel_token.is_cancelled() {
            return Err(HostError::Cancelled);
        }
        Ok(WorkflowAgentSpawnResult {
            success: true,
            output: self.output.clone(),
            error: None,
            cancelled: false,
            child_session_id: request.id,
            total_tokens_used: 0,
            duration_ms: 1,
            backgrounded: false,
        })
    }

    async fn cancel_run_children(&self, _run_id: &str) -> HostDrainOutcome {
        HostDrainOutcome::Drained
    }

    fn request_cancel_run_children(&self, _run_id: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stock_workflow_backend_retains_child_policy_and_ownership() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let backend = GrokSubagentBackend {
            subagent_event_tx: tx,
            parent_session_id: "parent".into(),
        };
        let cancel_token = CancellationToken::new();
        let task = tokio::spawn(async move {
            backend
                .spawn_and_await(WorkflowAgentSpawnRequest {
                    id: "child".into(),
                    prompt: "work".into(),
                    description: "description".into(),
                    subagent_type: "general-purpose".into(),
                    parent_session_id: "parent".into(),
                    resume_from: Some("previous".into()),
                    model: Some("model".into()),
                    reasoning_effort: Some("high".into()),
                    capability_mode: Some(SubagentCapabilityMode::ReadOnly),
                    isolation: None,
                    fork_context: false,
                    run_id: "wf_test".into(),
                    cancel_token,
                })
                .await
                .unwrap()
        });
        let SubagentEvent::Spawn(spawn) = rx.recv().await.unwrap() else {
            panic!("expected workflow spawn");
        };
        assert!(spawn.await_to_completion);
        assert!(spawn.owner.is_workflow());
        assert!(!spawn.surface_completion);
        assert_eq!(
            spawn.runtime_overrides.model_override_provenance,
            ModelOverrideProvenance::Tool
        );
        assert_eq!(
            spawn.runtime_overrides.capability_mode,
            Some(SubagentCapabilityMode::ReadOnly)
        );
        assert_eq!(
            spawn.runtime_overrides.reasoning_effort.as_deref(),
            Some("high")
        );
        assert_eq!(spawn.runtime_overrides.output_token_budget, None);
        assert!(spawn.runtime_overrides.output_schema.is_none());
        assert_eq!(spawn.resume_from.as_deref(), Some("previous"));
        spawn
            .respond_with(|request| SubagentResult {
                success: true,
                output: Arc::from("done"),
                subagent_id: request.id.clone(),
                child_session_id: request.id.clone(),
                ..Default::default()
            })
            .unwrap();
        assert!(task.await.unwrap().success);
    }
}
