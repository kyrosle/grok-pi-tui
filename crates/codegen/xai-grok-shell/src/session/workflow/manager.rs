//! Stock-session compatibility for the shared workflow manager.
use super::{
    backend::WorkflowAgentBackend,
    host_service::TelemetryHook,
    notify::WorkflowNotifySender,
    registry::{ResolvedWorkflow, WorkflowSource},
    store::WorkflowRunStore,
    tracker::{WorkflowRunState, WorkflowRunStatus, WorkflowTracker},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tokio::sync::{mpsc, oneshot};
use xai_grok_sampling_types::ReasoningEffort;
pub use xai_workflow::manager::LaunchError;
pub(crate) use xai_workflow::manager::{
    ControlError, WORKFLOW_DEFAULT_AGENT_BUDGET, WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION,
};
use xai_workflow::manager::{WorkflowEpisodeStatus, WorkflowRuntimeEvent, WorkflowRuntimeHooks};

pub(crate) struct LaunchSpec {
    pub objective: String,
    pub args: serde_json::Value,
    pub agent_budget: Option<u64>,
    pub effort: Option<ReasoningEffort>,
    pub resume_run_id: Option<String>,
}
pub(crate) struct WorkflowManager(xai_workflow::manager::WorkflowManager);
impl std::ops::Deref for WorkflowManager {
    type Target = xai_workflow::manager::WorkflowManager;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for WorkflowManager {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
static WORKFLOW_RUNS_ACTIVE: xai_grok_telemetry::activity::ActivityGauge =
    xai_grok_telemetry::activity::ActivityGauge::work(
        xai_grok_telemetry::activity::WORKFLOW_RUNS_ACTIVE_KEY,
    );
pub(crate) fn stock_hooks(
    session_cmd_tx: mpsc::UnboundedSender<crate::session::commands::SessionCommand>,
) -> WorkflowRuntimeHooks {
    WorkflowRuntimeHooks {
        enter_activity: Arc::new(|| Box::new(WORKFLOW_RUNS_ACTIVE.enter())),
        completion: Arc::new(move |run_id, revision| {
            let _ = session_cmd_tx.send(
                crate::session::commands::SessionCommand::WorkflowCompletionTurn {
                    run_id: run_id.to_owned(),
                    revision,
                },
            );
        }),
        normalize_effort: Arc::new(|token| {
            token
                .parse::<ReasoningEffort>()
                .map(|effort| effort.to_string())
        }),
        lifecycle: Arc::new(|event| {
            use xai_grok_telemetry::events::{
                SubagentLimitHit, WorkflowRunEnded, WorkflowRunStarted, WorkflowSourceKind,
            };
            match event {
                WorkflowRuntimeEvent::RunStarted {
                    run_id,
                    parent_session_id,
                    source,
                    state,
                    max_concurrent_agents,
                    resumed,
                } => {
                    debug_assert!(
                        WORKFLOW_RUNS_ACTIVE.get() >= 1,
                        "WorkflowRunStarted must stamp a self-inclusive count"
                    );
                    xai_grok_telemetry::session_ctx::log_event(WorkflowRunStarted {
                        run_id: run_id.to_owned(),
                        parent_session_id: parent_session_id.to_owned(),
                        source: match source {
                            WorkflowSource::Builtin => WorkflowSourceKind::Builtin,
                            WorkflowSource::Inline => WorkflowSourceKind::Inline,
                            WorkflowSource::File(_) => WorkflowSourceKind::File,
                        },
                        workflow_name: (*source == WorkflowSource::Builtin)
                            .then(|| state.name.clone()),
                        agent_budget: state.agent_budget,
                        max_concurrent_agents: u32::try_from(max_concurrent_agents)
                            .unwrap_or(u32::MAX),
                        resumed,
                    });
                }
                WorkflowRuntimeEvent::RunEnded {
                    run_id,
                    parent_session_id,
                    status,
                    duration_ms,
                    agents_used,
                    agent_budget,
                    stats,
                } => {
                    xai_grok_telemetry::session_ctx::log_event(WorkflowRunEnded {
                        run_id: run_id.to_owned(),
                        parent_session_id: parent_session_id.to_owned(),
                        status: run_ended_status(status),
                        duration_ms,
                        agents_used,
                        agent_budget,
                        agents_failed: stats.agents_failed.load(Ordering::Relaxed),
                        peak_concurrent_agents: stats.peak_concurrent.load(Ordering::Relaxed),
                        slot_waits: stats.slot_waits.load(Ordering::Relaxed),
                        slot_wait_ms_total: stats.slot_wait_ms_total.load(Ordering::Relaxed),
                        slot_wait_ms_max: stats.slot_wait_ms_max.load(Ordering::Relaxed),
                    });
                }
                WorkflowRuntimeEvent::AgentLimitHit {
                    parent_session_id,
                    run_id,
                    limit,
                    running,
                } => {
                    xai_grok_telemetry::session_ctx::log_event(
                        SubagentLimitHit::workflow_run_concurrent(
                            parent_session_id.to_owned(),
                            run_id.to_owned(),
                            limit,
                            running,
                        ),
                    );
                }
            }
        }),
    }
}
fn run_ended_status(
    status: WorkflowEpisodeStatus,
) -> xai_grok_telemetry::events::WorkflowRunEndStatus {
    use xai_grok_telemetry::events::WorkflowRunEndStatus as E;
    match status {
        WorkflowEpisodeStatus::Superseded => E::Superseded,
        WorkflowEpisodeStatus::Run(status) => match status {
            WorkflowRunStatus::Active => E::Active,
            WorkflowRunStatus::UserPaused => E::UserPaused,
            WorkflowRunStatus::BackOffPaused => E::BackOffPaused,
            WorkflowRunStatus::NoProgressPaused => E::NoProgressPaused,
            WorkflowRunStatus::InfraPaused => E::InfraPaused,
            WorkflowRunStatus::Blocked => E::Blocked,
            WorkflowRunStatus::BudgetLimited => E::BudgetLimited,
            WorkflowRunStatus::Interrupted => E::Interrupted,
            WorkflowRunStatus::Complete => E::Complete,
            WorkflowRunStatus::Failed => E::Failed,
            WorkflowRunStatus::Cancelled => E::Cancelled,
        },
    }
}
impl WorkflowManager {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        session_id: String,
        session_dir: Option<PathBuf>,
        cwd: PathBuf,
        tracker: Arc<parking_lot::Mutex<WorkflowTracker>>,
        active_work: Arc<std::sync::atomic::AtomicUsize>,
        store: WorkflowRunStore,
        notify: WorkflowNotifySender,
        backend: Arc<dyn WorkflowAgentBackend>,
        telemetry: TelemetryHook,
        session_cmd_tx: mpsc::UnboundedSender<crate::session::commands::SessionCommand>,
        templates: HashMap<String, String>,
        max_concurrent_agents: usize,
    ) -> Self {
        Self(xai_workflow::manager::WorkflowManager::new(
            session_id,
            session_dir,
            cwd,
            tracker,
            active_work,
            store,
            notify.into(),
            backend,
            telemetry,
            stock_hooks(session_cmd_tx),
            templates,
            max_concurrent_agents,
        ))
    }
    pub(crate) fn launch(
        &mut self,
        resolved: ResolvedWorkflow,
        spec: LaunchSpec,
    ) -> Result<(String, oneshot::Receiver<xai_workflow::WorkflowOutcome>), LaunchError> {
        self.0.launch(
            resolved,
            xai_workflow::manager::LaunchSpec {
                objective: spec.objective,
                args: spec.args,
                agent_budget: spec.agent_budget,
                effort: spec.effort.map(|effort| effort.to_string()),
                resume_run_id: spec.resume_run_id,
            },
        )
    }
    #[cfg(test)]
    pub(crate) fn test_bundle() -> (
        Arc<tokio::sync::Mutex<WorkflowManager>>,
        Arc<parking_lot::Mutex<WorkflowTracker>>,
    ) {
        Self::test_bundle_with_session_dir(None)
    }

    #[cfg(test)]
    pub(crate) fn test_bundle_with_session_dir(
        session_dir: Option<PathBuf>,
    ) -> (
        Arc<tokio::sync::Mutex<WorkflowManager>>,
        Arc<parking_lot::Mutex<WorkflowTracker>>,
    ) {
        let tracker = Arc::new(parking_lot::Mutex::new(WorkflowTracker::default()));
        let (gateway_tx, _gateway_rx) = mpsc::unbounded_channel();
        let (persist_tx, _persist_rx) = mpsc::unbounded_channel();
        let store = WorkflowRunStore::new(session_dir.clone(), persist_tx.clone());
        let notify = super::notify::WorkflowNotifySender::new(
            agent_client_protocol::SessionId::new("test-session"),
            xai_acp_lib::AcpAgentGatewaySender::new(gateway_tx),
            persist_tx,
            store.clone(),
        );
        let manager = Arc::new(tokio::sync::Mutex::new(WorkflowManager::new(
            "test-session".into(),
            session_dir,
            std::env::temp_dir(),
            tracker.clone(),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            store,
            notify,
            Arc::new(super::backend::MockWorkflowAgentBackend {
                output: Arc::from("mock"),
            }),
            Arc::new(|_, _, _| {}),
            mpsc::unbounded_channel().0,
            std::collections::HashMap::new(),
            super::host_service::DEFAULT_WORKFLOW_MAX_CONCURRENT_AGENTS,
        )));
        (manager, tracker)
    }
}
