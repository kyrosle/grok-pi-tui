//! Session-scoped upstream workflow host for grok-pi (External ACP).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// Host is shared via Arc so PiAgent can await launch/pause without RefCell borrow.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use xai_workflow::WorkflowOutcome;
use xai_workflow::{
    external::{ExternalWorkflowRuntime, ExternalWorkflowRuntimeConfig},
    notify::{WorkflowNotifySender, workflow_session_notification_json},
    registry::WorkflowRegistryConfig,
    store::WorkflowRunStore,
};

use crate::pi_workflow_backend::{BridgeCommandTx, PiWorkflowAgentBackend};

pub struct WorkflowHost {
    runtime: ExternalWorkflowRuntime,
    session_id: String,
}

// `Arc` for await-friendly sharing from PiAgent without holding RefCell borrows.

impl WorkflowHost {
    pub fn new(
        session_id: String,
        cwd: PathBuf,
        session_dir: Option<PathBuf>,
        bridge_tx: BridgeCommandTx,
    ) -> Self {
        let store = WorkflowRunStore::standalone(session_dir.clone());
        let notify = WorkflowNotifySender::new(store.clone(), Arc::new(|_, _| {}));
        let scratch = session_dir
            .clone()
            .unwrap_or_else(std::env::temp_dir)
            .join("pi-workflow-spawn");
        let backend = Arc::new(PiWorkflowAgentBackend::new(bridge_tx, scratch));
        let registry = pi_registry_config(&cwd, false);
        let runtime = ExternalWorkflowRuntime::new(ExternalWorkflowRuntimeConfig {
            session_id: session_id.clone(),
            session_dir,
            cwd,
            backend,
            notify,
            store,
            hooks: Default::default(),
            registry,
            templates: HashMap::new(),
        });
        Self {
            runtime,
            session_id,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.runtime
            .shutdown(Duration::from_secs(30))
            .await
            .map_err(|runs| {
                anyhow::anyhow!(
                    "workflow scope did not drain: {}; restart the Pi host",
                    runs.join(", ")
                )
            })
    }

    pub fn set_project_trust(&self, trusted: bool) {
        self.runtime
            .set_registry_config(pi_registry_config(self.runtime.cwd(), trusted));
    }
    pub async fn launch_named(
        &self,
        name: &str,
        objective: String,
        args: Value,
    ) -> Result<(String, tokio::sync::oneshot::Receiver<WorkflowOutcome>)> {
        self.runtime
            .launch_named(name, objective, args, None)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    #[cfg(test)]
    pub async fn launch_inline(
        &self,
        script: String,
        objective: String,
        args: Value,
    ) -> Result<(String, tokio::sync::oneshot::Receiver<WorkflowOutcome>)> {
        self.runtime
            .launch_inline(script, objective, args, None)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    pub async fn pause(&self, run_id: &str) -> bool {
        self.runtime.pause(run_id).await
    }

    pub async fn cancel(&self, run_id: &str) -> bool {
        self.runtime.cancel(run_id).await
    }

    pub fn notification_payloads(&self) -> Vec<Value> {
        self.runtime
            .list_runs()
            .into_iter()
            .map(|state| {
                let elapsed = self.runtime.elapsed_ms(&state.run_id);
                workflow_session_notification_json(&self.session_id, &state, elapsed)
            })
            .collect()
    }

    pub async fn drive_until_outcome(
        &self,
        mut outcome_rx: tokio::sync::oneshot::Receiver<WorkflowOutcome>,
        mut emit: impl FnMut(Value),
    ) -> Result<WorkflowOutcome> {
        let mut interval = tokio::time::interval(Duration::from_millis(150));
        loop {
            tokio::select! {
                outcome = &mut outcome_rx => {
                    for payload in self.notification_payloads() {
                        emit(payload);
                    }
                    return outcome.context("workflow outcome channel closed");
                }
                _ = interval.tick() => {
                    for payload in self.notification_payloads() {
                        emit(payload);
                    }
                }
            }
        }
    }
}

/// Pi decides project trust. The neutral registry only receives its current verdict.
pub fn pi_registry_config(cwd: &std::path::Path, project_allowed: bool) -> WorkflowRegistryConfig {
    let root = cwd
        .ancestors()
        .find(|path| path.join(".git").exists())
        .unwrap_or(cwd)
        .to_path_buf();
    WorkflowRegistryConfig {
        project_workflow_dir: Some(xai_grok_config::project_config_dir(&root).join("workflows")),
        project_root: Some(root),
        project_allowed,
        user_workflow_dir: Some(xai_grok_config::grok_home().join("workflows")),
        ..Default::default()
    }
}

pub fn outcome_to_json(outcome: &WorkflowOutcome) -> Value {
    match outcome {
        WorkflowOutcome::Completed { result } => json!({
            "status": "completed",
            "result": result,
        }),
        WorkflowOutcome::Paused { kind, message } => json!({
            "status": "paused",
            "kind": format!("{kind:?}"),
            "message": message,
        }),
        WorkflowOutcome::BudgetExceeded { message } => json!({
            "status": "budget_exceeded",
            "message": message,
        }),
        WorkflowOutcome::Cancelled => json!({ "status": "cancelled" }),
        WorkflowOutcome::Failed { error } => json!({
            "status": "failed",
            "error": error,
        }),
    }
}

pub fn format_outcome_for_tool(run_id: &str, outcome: &WorkflowOutcome) -> String {
    match outcome {
        WorkflowOutcome::Completed { result } => {
            let body = match result {
                Value::String(s) => s.clone(),
                other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
            };
            format!("Workflow run `{run_id}` completed.\n\n{body}")
        }
        WorkflowOutcome::Paused { kind, message } => {
            format!("Workflow run `{run_id}` paused ({kind:?}): {message}")
        }
        WorkflowOutcome::BudgetExceeded { message } => {
            format!("Workflow run `{run_id}` hit agent budget: {message}")
        }
        WorkflowOutcome::Cancelled => format!("Workflow run `{run_id}` was cancelled."),
        WorkflowOutcome::Failed { error } => {
            format!("Workflow run `{run_id}` failed: {error}")
        }
    }
}

pub fn parse_workflow_request(name: &str, args: &str) -> Result<WorkflowRequest> {
    let name = name.trim();
    let args = args.trim();
    if name.is_empty() {
        bail!("workflow name is required");
    }
    match name {
        "pause" | "stop" | "resume" | "save" => Ok(WorkflowRequest::Manage {
            op: name.to_string(),
            target: args.to_string(),
        }),
        _ if matches!(args, "pause" | "stop" | "resume" | "save") => Ok(WorkflowRequest::Manage {
            op: args.to_string(),
            target: name.to_string(),
        }),
        _ => {
            let json_args = if args.is_empty() {
                json!({})
            } else if let Ok(v) = serde_json::from_str::<Value>(args) {
                v
            } else {
                json!({ "objective": args })
            };
            let objective = json_args
                .get("objective")
                .and_then(Value::as_str)
                .unwrap_or(args)
                .to_string();
            Ok(WorkflowRequest::Launch {
                name: name.to_string(),
                objective: if objective.is_empty() {
                    name.to_string()
                } else {
                    objective
                },
                args: json_args,
            })
        }
    }
}

#[derive(Debug, Clone)]
pub enum WorkflowRequest {
    Launch {
        name: String,
        objective: String,
        args: Value,
    },
    Manage {
        op: String,
        target: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn neutral_workflow_notification_preserves_stock_wire_shape() {
        let state = xai_workflow::tracker::WorkflowTracker::default().start_run(
            "wf_wire".into(),
            "demo".into(),
            "objective".into(),
            Vec::new(),
            None,
            None,
        );
        let neutral = xai_workflow::notify::build_workflow_updated(&state, 25, 0);
        assert_eq!(
            serde_json::to_value(neutral).unwrap()["sessionUpdate"],
            "workflow_updated"
        );
        let envelope =
            xai_workflow::notify::workflow_session_notification_json("session", &state, 25);
        assert_eq!(envelope["sessionId"], "session");
        assert_eq!(envelope["update"]["sessionUpdate"], "workflow_updated");
    }

    #[tokio::test]
    async fn completed_workflow_host_persists_terminal_state() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, mut rx) =
            mpsc::unbounded_channel::<crate::pi_workflow_backend::BridgeCommandRequest>();
        let bridge = tokio::spawn(async move {
            while let Some(request) = rx.recv().await {
                assert_eq!(
                    request.command,
                    crate::pi_workflow_backend::WORKFLOW_CANCEL_COMMAND
                );
                let args: Value = serde_json::from_str(&request.args).unwrap();
                if let Some(response) = args.get("response").and_then(Value::as_str) {
                    std::fs::write(response, json!({ "drained": true }).to_string()).unwrap();
                }
                let _ = request.reply.send(Ok(()));
            }
        });
        let host = WorkflowHost::new(
            "test".into(),
            dir.path().into(),
            Some(dir.path().into()),
            tx,
        );
        let (run_id, outcome) = host
            .launch_inline(
                "let meta = #{ name: \"storage-check\", description: \"d\" }; complete(\"saved\");"
                    .into(),
                "storage check".into(),
                json!({}),
            )
            .await
            .unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(10), outcome)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, WorkflowOutcome::Completed { .. }));
        let manifest: xai_workflow::store::WorkflowRunManifest = serde_json::from_slice(
            &std::fs::read(dir.path().join("workflows").join(run_id).join("state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            manifest.state.status,
            xai_workflow::tracker::WorkflowRunStatus::Complete
        );
        assert_eq!(manifest.state.result_summary.as_deref(), Some("saved"));
        bridge.abort();
    }

    #[test]
    fn parses_launch_and_manage() {
        match parse_workflow_request("deep-research", "compare postgres").unwrap() {
            WorkflowRequest::Launch {
                name, objective, ..
            } => {
                assert_eq!(name, "deep-research");
                assert!(objective.contains("postgres"));
            }
            other => panic!("{other:?}"),
        }
        match parse_workflow_request("pause", "deep-research").unwrap() {
            WorkflowRequest::Manage { op, target } => {
                assert_eq!(op, "pause");
                assert_eq!(target, "deep-research");
            }
            other => panic!("{other:?}"),
        }
    }
}
