//! Stock compatibility constructor for the shared External runtime.
use super::{backend::WorkflowAgentBackend, notify::WorkflowNotifySender, store::WorkflowRunStore};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::mpsc;
pub struct ExternalWorkflowRuntime(xai_workflow::external::ExternalWorkflowRuntime);
pub struct ExternalWorkflowRuntimeConfig {
    pub session_id: String,
    pub session_dir: Option<PathBuf>,
    pub cwd: PathBuf,
    pub backend: Arc<dyn WorkflowAgentBackend>,
    pub notify: WorkflowNotifySender,
    pub store: WorkflowRunStore,
    pub session_cmd_tx: mpsc::UnboundedSender<crate::session::commands::SessionCommand>,
    pub templates: HashMap<String, String>,
}
impl std::ops::Deref for ExternalWorkflowRuntime {
    type Target = xai_workflow::external::ExternalWorkflowRuntime;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl ExternalWorkflowRuntime {
    pub fn new(config: ExternalWorkflowRuntimeConfig) -> Self {
        let registry = super::registry::stock_config(Some(&config.cwd));
        Self(xai_workflow::external::ExternalWorkflowRuntime::new(
            xai_workflow::external::ExternalWorkflowRuntimeConfig {
                session_id: config.session_id,
                session_dir: config.session_dir,
                cwd: config.cwd,
                backend: config.backend,
                notify: config.notify.into(),
                store: config.store,
                hooks: super::manager::stock_hooks(config.session_cmd_tx),
                registry,
                templates: config.templates,
            },
        ))
    }
}
/// Build a test-only runtime with mock agent backend (no Grok subagent channel).
#[cfg(test)]
pub fn test_runtime(session_dir: Option<PathBuf>) -> ExternalWorkflowRuntime {
    use super::backend::MockWorkflowAgentBackend;
    use super::notify::WorkflowNotifySender;

    let (persist_tx, mut persist_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(message) = persist_rx.recv().await {
            if let crate::session::persistence::PersistenceMsg::WorkflowRunStateAndAck {
                respond_to,
                ..
            } = message
            {
                let _ = respond_to.send(Ok(()));
            }
        }
    });
    let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
    let store = WorkflowRunStore::new(session_dir.clone(), persist_tx.clone());
    let notify = WorkflowNotifySender::new(
        agent_client_protocol::SessionId::new("external-test"),
        xai_acp_lib::AcpAgentGatewaySender::new(gateway_tx),
        persist_tx,
        store.clone(),
    );
    ExternalWorkflowRuntime::new(ExternalWorkflowRuntimeConfig {
        session_id: "external-test".into(),
        session_dir,
        cwd: std::env::temp_dir(),
        backend: Arc::new(MockWorkflowAgentBackend {
            output: Arc::from("external-mock"),
        }),
        notify,
        store,
        session_cmd_tx: mpsc::unbounded_channel().0,
        templates: HashMap::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_workflow::WorkflowOutcome;

    #[tokio::test]
    async fn external_runtime_launch_inline_with_mock_backend() {
        let dir = tempfile::tempdir().unwrap();
        let rt = test_runtime(Some(dir.path().to_path_buf()));
        let (run_id, outcome_rx) = rt
            .launch_inline(
                r#"
let meta = #{ name: "ext-inline", description: "d" };
let r = agent("hi");
complete(r.output);
"#
                .into(),
                "obj".into(),
                serde_json::json!({}),
                None,
            )
            .await
            .unwrap();
        let outcome = outcome_rx.await.unwrap();
        match outcome {
            WorkflowOutcome::Completed { result } => {
                assert_eq!(result.as_str().unwrap_or_default(), "external-mock");
            }
            other => panic!("expected completed, got {other:?}"),
        }
        assert!(rt.list_runs().iter().any(|r| r.run_id == run_id));
    }
}
