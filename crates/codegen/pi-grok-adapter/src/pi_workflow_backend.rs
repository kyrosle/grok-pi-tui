//! Pi `WorkflowAgentBackend` for upstream `xai-workflow` host.
//!
//! Spawn protocol: write request JSON → hidden `/__pi_workflow_spawn` → read response JSON.
//! Uses a channel so the backend is `Send` while command execution stays on the Pi LocalSet.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use xai_workflow::HostError;
use xai_workflow::backend::{
    HostDrainOutcome, WorkflowAgentBackend, WorkflowAgentSpawnRequest, WorkflowAgentSpawnResult,
};

pub const WORKFLOW_SPAWN_COMMAND: &str = "__pi_workflow_spawn";
pub const WORKFLOW_CANCEL_COMMAND: &str = "__pi_workflow_cancel";
pub const WORKFLOW_TRUST_COMMAND: &str = "__pi_workflow_trust";

/// Request executed by the Pi LocalSet owner (`PiAgent`).
pub struct BridgeCommandRequest {
    pub command: String,
    pub args: String,
    pub reply: oneshot::Sender<Result<(), String>>,
}

pub type BridgeCommandTx = mpsc::UnboundedSender<BridgeCommandRequest>;

#[derive(Debug, Serialize)]
struct SpawnRequestFile {
    id: String,
    prompt: String,
    description: String,
    subagent_type: String,
    parent_session_id: String,
    resume_from: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
    capability_mode: Option<String>,
    isolation_worktree: bool,
    fork_context: bool,
    run_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpawnResponseFile {
    success: bool,
    output: String,
    error: Option<String>,
    cancelled: bool,
    child_session_id: String,
    total_tokens_used: u64,
    duration_ms: u64,
    backgrounded: bool,
}

pub struct PiWorkflowAgentBackend {
    bridge_tx: BridgeCommandTx,
    scratch_dir: PathBuf,
}

impl PiWorkflowAgentBackend {
    pub fn new(bridge_tx: BridgeCommandTx, scratch_dir: PathBuf) -> Self {
        Self {
            bridge_tx,
            scratch_dir,
        }
    }

    async fn run_bridge(&self, command: &str, args: String) -> Result<(), HostError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.bridge_tx
            .send(BridgeCommandRequest {
                command: command.to_string(),
                args,
                reply: reply_tx,
            })
            .map_err(|_| HostError::Failed("workflow bridge command channel closed".into()))?;
        reply_rx
            .await
            .map_err(|_| HostError::Failed("workflow bridge command dropped".into()))?
            .map_err(HostError::Failed)
    }
}

#[async_trait]
impl WorkflowAgentBackend for PiWorkflowAgentBackend {
    async fn spawn_and_await(
        &self,
        request: WorkflowAgentSpawnRequest,
    ) -> Result<WorkflowAgentSpawnResult, HostError> {
        if request.cancel_token.is_cancelled() {
            return Err(HostError::Cancelled);
        }
        std::fs::create_dir_all(&self.scratch_dir)
            .map_err(|e| HostError::Failed(format!("workflow spawn scratch dir: {e}")))?;
        let id = uuid::Uuid::now_v7().simple().to_string();
        let req_path = self.scratch_dir.join(format!("spawn-{id}.req.json"));
        let resp_path = self.scratch_dir.join(format!("spawn-{id}.resp.json"));

        let capability_mode = request.capability_mode.map(|mode| mode.as_str().to_owned());

        let body = SpawnRequestFile {
            id: request.id.clone(),
            prompt: request.prompt,
            description: request.description,
            subagent_type: request.subagent_type,
            parent_session_id: request.parent_session_id,
            resume_from: request.resume_from,
            model: request.model,
            reasoning_effort: request.reasoning_effort,
            capability_mode,
            isolation_worktree: request.isolation.is_some(),
            fork_context: request.fork_context,
            run_id: request.run_id.clone(),
        };
        std::fs::write(
            &req_path,
            serde_json::to_vec(&body).map_err(|e| HostError::Failed(e.to_string()))?,
        )
        .map_err(|e| HostError::Failed(format!("write spawn request: {e}")))?;

        let args = serde_json::json!({ "request": req_path, "response": resp_path }).to_string();
        let cancel = request.cancel_token.clone();
        let bridge_fut = self.run_bridge(WORKFLOW_SPAWN_COMMAND, args);
        tokio::pin!(bridge_fut);
        tokio::select! {
            result = &mut bridge_fut => {
                result?;
            }
            _ = cancel.cancelled() => {
                let _ = self
                    .run_bridge(
                        WORKFLOW_CANCEL_COMMAND,
                        format!("--run-id {}", request.run_id),
                    )
                    .await;
                return Err(HostError::Cancelled);
            }
        }

        let raw = std::fs::read_to_string(&resp_path).map_err(|e| {
            HostError::Failed(format!("read spawn response {}: {e}", resp_path.display()))
        })?;
        let resp: SpawnResponseFile = serde_json::from_str(&raw)
            .map_err(|e| HostError::Failed(format!("parse spawn response: {e}; body={raw}")))?;
        let _ = std::fs::remove_file(&req_path);
        let _ = std::fs::remove_file(&resp_path);

        Ok(WorkflowAgentSpawnResult {
            success: resp.success,
            output: Arc::from(resp.output.as_str()),
            error: resp.error,
            cancelled: resp.cancelled,
            child_session_id: resp.child_session_id,
            total_tokens_used: resp.total_tokens_used,
            duration_ms: resp.duration_ms,
            backgrounded: resp.backgrounded,
        })
    }

    async fn cancel_run_children(&self, run_id: &str) -> HostDrainOutcome {
        let result = async {
            let directory =
                tempfile::tempdir().map_err(|error| HostError::Failed(error.to_string()))?;
            let response = directory.path().join("drain.json");
            self.run_bridge(
                WORKFLOW_CANCEL_COMMAND,
                serde_json::json!({ "run-id": run_id, "response": response }).to_string(),
            )
            .await?;
            let value: serde_json::Value = serde_json::from_slice(
                &std::fs::read(response).map_err(|error| HostError::Failed(error.to_string()))?,
            )
            .map_err(|error| HostError::Failed(error.to_string()))?;
            Ok::<bool, HostError>(
                value.get("drained").and_then(serde_json::Value::as_bool) == Some(true),
            )
        };
        if matches!(
            tokio::time::timeout(std::time::Duration::from_secs(20), result).await,
            Ok(Ok(true))
        ) {
            HostDrainOutcome::Drained
        } else {
            HostDrainOutcome::TimedOut
        }
    }

    fn request_cancel_run_children(&self, run_id: &str) -> bool {
        let (reply_tx, _reply_rx) = oneshot::channel();
        self.bridge_tx
            .send(BridgeCommandRequest {
                command: WORKFLOW_CANCEL_COMMAND.to_string(),
                args: format!("--run-id {run_id}"),
                reply: reply_tx,
            })
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn cancel_requires_a_positive_drain_acknowledgement() {
        for drained in [Some(true), Some(false), None] {
            let (tx, mut rx) = mpsc::unbounded_channel::<BridgeCommandRequest>();
            tokio::spawn(async move {
                let command = rx.recv().await.unwrap();
                assert_eq!(command.command, WORKFLOW_CANCEL_COMMAND);
                let args: serde_json::Value = serde_json::from_str(&command.args).unwrap();
                if let Some(drained) = drained {
                    std::fs::write(
                        args["response"].as_str().unwrap(),
                        serde_json::json!({ "drained": drained }).to_string(),
                    )
                    .unwrap();
                }
                let _ = command.reply.send(Ok(()));
            });
            let backend = PiWorkflowAgentBackend::new(tx, std::env::temp_dir());
            assert_eq!(
                backend.cancel_run_children("wf_ack").await,
                if drained == Some(true) {
                    HostDrainOutcome::Drained
                } else {
                    HostDrainOutcome::TimedOut
                }
            );
        }
    }

    #[tokio::test]
    async fn pi_backend_reads_spawn_response_file() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();
        let (tx, mut rx) = mpsc::unbounded_channel::<BridgeCommandRequest>();
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let args: serde_json::Value = serde_json::from_str(&req.args).unwrap();
                let resp = args["response"].as_str();
                let request_path = args["request"].as_str().unwrap();
                let request: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(request_path).unwrap()).unwrap();
                assert_eq!(request["capability_mode"], "read-only");
                assert_eq!(request["reasoning_effort"], "high");
                if let Some(resp) = resp {
                    let body = SpawnResponseFile {
                        success: true,
                        output: "pi-child-ok".into(),
                        error: None,
                        cancelled: false,
                        child_session_id: "child-1".into(),
                        total_tokens_used: 3,
                        duration_ms: 5,
                        backgrounded: false,
                    };
                    let _ = std::fs::write(resp, serde_json::to_vec(&body).unwrap());
                }
                let _ = req.reply.send(Ok(()));
            }
        });
        let backend = PiWorkflowAgentBackend::new(tx, dir_path);
        let result = backend
            .spawn_and_await(WorkflowAgentSpawnRequest {
                id: "a1".into(),
                prompt: "hi".into(),
                description: "d".into(),
                subagent_type: "general-purpose".into(),
                parent_session_id: "p".into(),
                resume_from: None,
                model: None,
                reasoning_effort: Some("high".into()),
                capability_mode: Some(serde_json::from_str("\"read-only\"").unwrap()),
                isolation: None,
                fork_context: false,
                run_id: "wf_1".into(),
                cancel_token: CancellationToken::new(),
            })
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(&*result.output, "pi-child-ok");
        assert_eq!(result.child_session_id, "child-1");
    }
}
