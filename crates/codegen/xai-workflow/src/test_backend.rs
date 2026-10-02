//! Channel backend used by the shared lifecycle tests.
use crate::{
    HostError,
    backend::{
        HostDrainOutcome, WorkflowAgentBackend, WorkflowAgentSpawnRequest, WorkflowAgentSpawnResult,
    },
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
pub enum TestEvent {
    Spawn(TestSpawn),
    #[allow(dead_code)]
    Cancel,
}
pub struct TestSpawn {
    pub request: WorkflowAgentSpawnRequest,
    pub result_tx: oneshot::Sender<WorkflowAgentSpawnResult>,
}
impl std::ops::Deref for TestSpawn {
    type Target = WorkflowAgentSpawnRequest;
    fn deref(&self) -> &Self::Target {
        &self.request
    }
}
impl TestSpawn {
    pub fn respond_with(
        self,
        f: impl FnOnce(&WorkflowAgentSpawnRequest) -> WorkflowAgentSpawnResult,
    ) -> Result<(), WorkflowAgentSpawnResult> {
        self.result_tx.send(f(&self.request))
    }
}
pub struct ChannelBackend {
    pub tx: mpsc::UnboundedSender<TestEvent>,
    pub cancels: Arc<parking_lot::Mutex<Vec<String>>>,
}
#[async_trait::async_trait]
impl WorkflowAgentBackend for ChannelBackend {
    async fn spawn_and_await(
        &self,
        request: WorkflowAgentSpawnRequest,
    ) -> Result<WorkflowAgentSpawnResult, HostError> {
        let (result_tx, result_rx) = oneshot::channel();
        self.tx
            .send(TestEvent::Spawn(TestSpawn { request, result_tx }))
            .map_err(|_| HostError::Failed("test channel closed".into()))?;
        result_rx
            .await
            .map_err(|_| HostError::Failed("test result channel closed".into()))
    }
    async fn cancel_run_children(&self, run_id: &str) -> HostDrainOutcome {
        self.cancels.lock().push(run_id.to_owned());
        HostDrainOutcome::Drained
    }
    fn request_cancel_run_children(&self, run_id: &str) -> bool {
        self.cancels.lock().push(run_id.to_owned());
        true
    }
}
pub struct MockWorkflowAgentBackend {
    pub output: Arc<str>,
}
#[async_trait::async_trait]
impl WorkflowAgentBackend for MockWorkflowAgentBackend {
    async fn spawn_and_await(
        &self,
        request: WorkflowAgentSpawnRequest,
    ) -> Result<WorkflowAgentSpawnResult, HostError> {
        Ok(WorkflowAgentSpawnResult {
            success: true,
            output: self.output.clone(),
            child_session_id: request.id,
            ..empty_result()
        })
    }
    async fn cancel_run_children(&self, _: &str) -> HostDrainOutcome {
        HostDrainOutcome::Drained
    }
}
pub fn empty_result() -> WorkflowAgentSpawnResult {
    WorkflowAgentSpawnResult {
        success: false,
        output: Arc::from(""),
        error: None,
        cancelled: false,
        child_session_id: String::new(),
        total_tokens_used: 0,
        duration_ms: 0,
        backgrounded: false,
    }
}
