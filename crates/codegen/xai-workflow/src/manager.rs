use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{Journal, WorkflowOutcome, WorkflowRunParams};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;
use xai_tool_types::workflow::WorkflowControl;

use super::backend::{HostDrainOutcome, WorkflowAgentBackend};
use super::host_service::{TelemetryHook, WorkflowHostParams, spawn_workflow_host_service};
use super::notify::WorkflowNotifySender;
use super::registry::{ResolvedWorkflow, WorkflowSource};
use super::store::WorkflowRunStore;
use super::tracker::{WorkflowRunState, WorkflowRunStatus, WorkflowTracker};

pub const WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION: usize = 4;
pub const WORKFLOW_DEFAULT_AGENT_BUDGET: u64 = crate::DEFAULT_AGENT_BUDGET;

/// Product side effects supplied by the owning host; execution stays shared.
#[derive(Clone)]
pub struct WorkflowRuntimeHooks {
    pub lifecycle: Arc<dyn Fn(WorkflowRuntimeEvent<'_>) + Send + Sync>,
    pub enter_activity: Arc<dyn Fn() -> Box<dyn Send> + Send + Sync>,
    pub completion: Arc<dyn Fn(&str, u64) + Send + Sync>,
    pub normalize_effort: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
}
impl Default for WorkflowRuntimeHooks {
    fn default() -> Self {
        Self {
            lifecycle: Arc::new(|_| {}),
            enter_activity: Arc::new(|| Box::new(())),
            completion: Arc::new(|_, _| {}),
            normalize_effort: Arc::new(|effort| {
                let token = effort.to_lowercase();
                match token.as_str() {
                    "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max" => Ok(token),
                    _ => Err(format!(
                        "invalid reasoning effort: {effort:?} (expected one of: none, minimal, low, medium, high, xhigh, max)"
                    )),
                }
            }),
        }
    }
}
#[derive(Clone, Copy)]
pub enum WorkflowEpisodeStatus {
    Run(WorkflowRunStatus),
    Superseded,
}
pub enum WorkflowRuntimeEvent<'a> {
    RunStarted {
        run_id: &'a str,
        parent_session_id: &'a str,
        source: &'a WorkflowSource,
        state: &'a WorkflowRunState,
        max_concurrent_agents: usize,
        resumed: bool,
    },
    RunEnded {
        run_id: &'a str,
        parent_session_id: &'a str,
        status: WorkflowEpisodeStatus,
        duration_ms: u64,
        agents_used: u64,
        agent_budget: Option<u64>,
        stats: &'a super::host_service::WorkflowAgentStats,
    },
    AgentLimitHit {
        parent_session_id: &'a str,
        run_id: &'a str,
        limit: u64,
        running: u32,
    },
}

struct ActiveRun {
    cancel: CancellationToken,
    pause_intent: Arc<AtomicBool>,
    done: oneshot::Receiver<()>,
}

pub struct LaunchSpec {
    pub objective: String,
    pub args: serde_json::Value,
    pub agent_budget: Option<u64>,
    pub effort: Option<String>,
    pub resume_run_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LaunchError {
    #[error("workflow session scope is closed")]
    SessionClosed,
    #[error("workflow run not found: {0}")]
    UnknownRun(String),
    #[error("journal error: {0}")]
    Journal(String),
    #[error("workflow store error: {0}")]
    Store(String),
    #[error("run is not resumable (status: {0})")]
    NotResumable(String),
    #[error(
        "run is budget-limited at {used} of {limit} agents; resume it \
         with an agent_budget above {used}"
    )]
    BudgetNotRaised { used: u64, limit: u64 },
    #[error(
        "session already has the maximum of {WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION} active workflow runs"
    )]
    TooManyActiveRuns,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ControlError {
    #[error("no workflow run in this session matches '{0}'")]
    UnknownRun(String),
    #[error("run '{name}' is {} and cannot be {}", status.as_ref(), match control {
        WorkflowControl::Pause => "paused",
        WorkflowControl::Stop => "stopped",
    })]
    NotApplicable {
        name: String,
        status: WorkflowRunStatus,
        control: WorkflowControl,
    },
}

pub struct WorkflowManager {
    session_id: String,
    session_dir: Option<PathBuf>,
    cwd: PathBuf,
    tracker: Arc<parking_lot::Mutex<WorkflowTracker>>,
    active_work: Arc<std::sync::atomic::AtomicUsize>,
    store: WorkflowRunStore,
    notify: WorkflowNotifySender,
    backend: Arc<dyn WorkflowAgentBackend>,
    telemetry: TelemetryHook,
    hooks: WorkflowRuntimeHooks,
    templates: HashMap<String, String>,
    active: HashMap<String, ActiveRun>,
    retiring: Vec<(String, oneshot::Receiver<()>)>,
    max_concurrent_agents: usize,
    closed: bool,
    shutdown_error: Option<Vec<String>>,
}

impl WorkflowManager {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: String,
        session_dir: Option<PathBuf>,
        cwd: PathBuf,
        tracker: Arc<parking_lot::Mutex<WorkflowTracker>>,
        active_work: Arc<std::sync::atomic::AtomicUsize>,
        store: WorkflowRunStore,
        notify: WorkflowNotifySender,
        backend: Arc<dyn WorkflowAgentBackend>,
        telemetry: TelemetryHook,
        hooks: WorkflowRuntimeHooks,
        templates: HashMap<String, String>,
        max_concurrent_agents: usize,
    ) -> Self {
        Self {
            session_id,
            session_dir,
            cwd,
            tracker,
            active_work,
            store,
            notify,
            backend,
            telemetry,
            hooks,
            templates,
            active: HashMap::new(),
            retiring: Vec::new(),
            closed: false,
            shutdown_error: None,
            max_concurrent_agents: super::host_service::workflow_max_concurrent_agents(
                max_concurrent_agents,
            ),
        }
    }

    #[cfg(test)]
    pub fn test_set_max_concurrent_agents(&mut self, n: usize) {
        self.max_concurrent_agents = n.max(1);
    }

    pub fn tracker(&self) -> Arc<parking_lot::Mutex<WorkflowTracker>> {
        self.tracker.clone()
    }

    pub fn launch(
        &mut self,
        resolved: ResolvedWorkflow,
        mut spec: LaunchSpec,
    ) -> Result<(String, oneshot::Receiver<WorkflowOutcome>), LaunchError> {
        if self.closed {
            return Err(LaunchError::SessionClosed);
        }
        self.reap_terminal_runs();
        if self.active.len().saturating_add(self.retiring.len())
            >= WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION
        {
            return Err(LaunchError::TooManyActiveRuns);
        }

        let allow_fork_context = resolved.source == WorkflowSource::Builtin;
        let mut execution_script = resolved.script;
        let (run_id, journal, state) = match &spec.resume_run_id {
            Some(run_id) => {
                let existing = self
                    .tracker
                    .lock()
                    .get(run_id)
                    .ok_or_else(|| LaunchError::UnknownRun(run_id.clone()))?;
                if !existing.status.is_resumable() {
                    return Err(LaunchError::NotResumable(
                        existing.status.as_ref().to_string(),
                    ));
                }
                if existing.status == crate::tracker::WorkflowRunStatus::BudgetLimited
                    && existing.agents_used >= crate::MAX_AGENT_BUDGET
                {
                    return Err(LaunchError::NotResumable(
                        "maximum agent budget reached; start a new run".into(),
                    ));
                }
                let original_args = self.store.args_for(run_id).ok_or_else(|| {
                    LaunchError::Store("immutable launch args are missing".into())
                })?;
                spec.effort = self
                    .store
                    .effort_for(run_id)
                    .map(|token| (self.hooks.normalize_effort)(&token))
                    .transpose()
                    .map_err(LaunchError::Store)?;
                if original_args != spec.args {
                    return Err(LaunchError::Store(
                        "workflow launch args are immutable across resume".into(),
                    ));
                }
                execution_script = self.store.script_for(run_id).ok_or_else(|| {
                    LaunchError::Store("immutable workflow script is missing".into())
                })?;
                let mut journal = match existing
                    .journal_path
                    .as_ref()
                    .and_then(|p| self.session_dir.as_ref().map(|d| (d, p)))
                {
                    Some((session_dir, relative)) => {
                        let expected = format!("workflows/{run_id}/journal.jsonl");
                        if relative != &expected {
                            return Err(LaunchError::Journal(
                                "persisted journal path does not match its workflow run".into(),
                            ));
                        }
                        Journal::load(session_dir.join(relative))
                            .map_err(|e| LaunchError::Journal(e.to_string()))?
                    }
                    None => Journal::new(None),
                };
                if existing.status == crate::tracker::WorkflowRunStatus::Failed {
                    journal
                        .prune_trailing_host_error(existing.pause_message.as_deref().unwrap_or(""))
                        .map_err(|e| LaunchError::Journal(e.to_string()))?;
                }
                let state = {
                    let mut tracker = self.tracker.lock();
                    tracker.reconcile_agents_used(run_id, journal.agent_reservation_count());
                    tracker.resume_run(run_id, spec.agent_budget)
                }
                .ok_or_else(|| {
                    if existing.status == crate::tracker::WorkflowRunStatus::BudgetLimited {
                        LaunchError::BudgetNotRaised {
                            used: existing.agents_used,
                            limit: existing.agent_budget.unwrap_or(0),
                        }
                    } else {
                        LaunchError::NotResumable(existing.status.as_ref().into())
                    }
                })?;

                (run_id.clone(), journal, state)
            }
            None => {
                let run_id = format!("wf_{}", uuid::Uuid::now_v7().simple());
                let agent_budget = spec.agent_budget.unwrap_or(WORKFLOW_DEFAULT_AGENT_BUDGET);
                self.store
                    .register(&run_id, &execution_script, &spec.args, spec.effort.clone())
                    .map_err(|error| LaunchError::Store(error.to_string()))?;
                let journal_rel = format!("workflows/{run_id}/journal.jsonl");
                let journal_path = self.session_dir.as_ref().map(|d| d.join(&journal_rel));
                let journal = Journal::new(journal_path);
                let state = self.tracker.lock().start_run(
                    run_id.clone(),
                    resolved.meta.name,
                    spec.objective.clone(),
                    resolved.meta.phases,
                    Some(agent_budget),
                    self.session_dir.as_ref().map(|_| journal_rel),
                );
                (run_id, journal, state)
            }
        };

        if let Err(error) = self.store.persist_now(&state) {
            if spec.resume_run_id.is_some() {
                if let Some(interrupted) = self.tracker.lock().interrupt(
                    &run_id,
                    "workflow state persistence failed before resume; start a new run",
                ) && let Err(persist_error) = self.store.persist(&interrupted)
                {
                    tracing::warn!(run_id = %run_id, %persist_error, "failed to queue interrupted workflow state");
                }
            } else {
                self.tracker.lock().clear_run(&run_id);
                self.store.remove(&run_id);
            }
            return Err(LaunchError::Store(error.to_string()));
        }
        self.notify
            .emit(&state, self.tracker.lock().elapsed_ms(&run_id), 0);

        let active = (self.hooks.enter_activity)();
        let work = crate::activity::WorkGuard::new(self.active_work.clone());
        log_run_started(
            &self.hooks,
            &run_id,
            &self.session_id,
            &resolved.source,
            &state,
            self.max_concurrent_agents,
            spec.resume_run_id.is_some(),
        );

        let (host_tx, host_rx) = mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let scratch_dir = self
            .session_dir
            .clone()
            .unwrap_or_else(std::env::temp_dir)
            .join("workflows")
            .join(&run_id)
            .join("scratch");

        let agent_stats = Arc::new(super::host_service::WorkflowAgentStats::default());
        let (host_service, host_drained) = spawn_workflow_host_service(
            WorkflowHostParams {
                run_id: run_id.clone(),
                max_concurrent_agents: self.max_concurrent_agents,
                cwd: self.cwd.clone(),
                scratch_dir,
                tracker: self.tracker.clone(),
                store: self.store.clone(),
                notify: self.notify.clone(),
                backend: self.backend.clone(),
                parent_session_id: self.session_id.clone(),
                allow_fork_context,
                effort: spec.effort,
                hooks: self.hooks.clone(),
                templates: self.templates.clone(),
                telemetry: self.telemetry.clone(),
                stats: agent_stats.clone(),
                cancel: cancel.clone(),
            },
            host_rx,
        );

        let script = execution_script;
        let args = spec.args;
        let exec_cancel = cancel.clone();
        let exec = tokio::task::spawn_blocking(move || {
            crate::run_workflow(WorkflowRunParams {
                script,
                args,
                journal,
                host_tx,
                cancel: exec_cancel,
                max_ops: WorkflowRunParams::DEFAULT_MAX_OPS,
            })
        });

        let pause_intent = Arc::new(AtomicBool::new(false));
        let (done_tx, done_rx) = oneshot::channel();
        self.active.insert(
            run_id.clone(),
            ActiveRun {
                cancel: cancel.clone(),
                pause_intent: pause_intent.clone(),
                done: done_rx,
            },
        );

        let (outcome_tx, outcome_rx) = oneshot::channel();
        let tracker = self.tracker.clone();
        let store = self.store.clone();
        let notify = self.notify.clone();
        let hooks = self.hooks.clone();
        let watcher_run_id = run_id.clone();
        let watcher_cancel = cancel.clone();
        let watcher_session_id = self.session_id.clone();
        let watcher_agent_stats = agent_stats;
        let execution_epoch = self.tracker.lock().execution_epoch(&run_id).unwrap_or(0);
        tokio::spawn(async move {
            let _active = active;
            let _work = work;
            let mut outcome = exec.await.unwrap_or_else(|e| WorkflowOutcome::Failed {
                error: format!("workflow executor panicked: {e}"),
            });
            if !host_service.is_finished() {
                watcher_cancel.cancel();
            }
            let host_drain =
                tokio::time::timeout(std::time::Duration::from_secs(25), host_drained).await;
            let drain_failed = !matches!(host_drain, Ok(Ok(HostDrainOutcome::Drained)));
            if drain_failed {
                host_service.abort();
            }
            let _ = host_service.await;
            if drain_failed {
                tracing::warn!(run_id = %watcher_run_id, "workflow host/child drain did not complete before lifecycle update");
                outcome = WorkflowOutcome::Failed {
                    error:
                        "workflow cleanup did not complete; run is interrupted and cannot resume"
                            .into(),
                };
            }
            // Epoch check and lifecycle mutation stay under one lock, or a quick resume could let this stale watcher stomp the successor
            let (epoch_matches, state) = {
                let mut tracker = tracker.lock();
                if tracker.execution_epoch(&watcher_run_id) != Some(execution_epoch) {
                    (false, None)
                } else if drain_failed {
                    (
                        true,
                        tracker.interrupt(
                            &watcher_run_id,
                            "workflow cleanup timed out or could not be acknowledged; start a \
                             new run",
                        ),
                    )
                } else if pause_intent.load(Ordering::Relaxed)
                    && matches!(
                        outcome,
                        WorkflowOutcome::Cancelled | WorkflowOutcome::Paused { .. }
                    )
                {
                    (true, tracker.pause_user(&watcher_run_id, None))
                } else {
                    (true, tracker.apply_outcome(&watcher_run_id, &outcome))
                }
            };
            if !epoch_matches {
                // A quick resume took over; close this episode as superseded (cumulative fields may reflect the successor)
                let (elapsed, agents_used, agent_budget) = {
                    let tracker = tracker.lock();
                    let run = tracker.get(&watcher_run_id);
                    (
                        tracker.elapsed_ms(&watcher_run_id),
                        run.as_ref().map(|run| run.agents_used).unwrap_or_default(),
                        run.as_ref().and_then(|run| run.agent_budget),
                    )
                };
                log_run_ended(
                    &hooks,
                    RunEndMetadata {
                        run_id: &watcher_run_id,
                        parent_session_id: &watcher_session_id,
                        status: WorkflowEpisodeStatus::Superseded,
                        duration_ms: elapsed,
                        agents_used,
                        agent_budget,
                    },
                    &watcher_agent_stats,
                );
                let _ = done_tx.send(());
                let _ = outcome_tx.send(outcome);
                return;
            }
            if state.is_none() {
                // The run left the tracker; close the episode so its `workflow_run_started` is not orphaned
                log_run_ended(
                    &hooks,
                    RunEndMetadata {
                        run_id: &watcher_run_id,
                        parent_session_id: &watcher_session_id,
                        status: WorkflowEpisodeStatus::Run(WorkflowRunStatus::Interrupted),
                        duration_ms: 0,
                        agents_used: 0,
                        agent_budget: None,
                    },
                    &watcher_agent_stats,
                );
            }
            if let Some(mut state) = state {
                let mut persisted = true;
                if let Err(error) = store.persist_ack(&state).await {
                    tracing::warn!(run_id = %watcher_run_id, %error, "workflow terminal manifest was not durably written");
                    outcome = WorkflowOutcome::Failed {
                        error: format!(
                            "workflow terminal state could not be persisted: {error}; run is interrupted"
                        ),
                    };
                    state = tracker
                        .lock()
                        .interrupt(
                            &watcher_run_id,
                            format!(
                                "workflow terminal state could not be persisted: {error}; start a new run"
                            ),
                        )
                        .unwrap_or(state);
                    if let Err(interrupt_error) = store.persist_ack(&state).await {
                        persisted = false;
                        tracing::error!(run_id = %watcher_run_id, %interrupt_error, "failed to persist workflow interruption marker");
                    }
                }
                // Emit before the persist-failure return: every start event gets an end event
                let elapsed = tracker.lock().elapsed_ms(&watcher_run_id);
                log_run_ended(
                    &hooks,
                    RunEndMetadata {
                        run_id: &watcher_run_id,
                        parent_session_id: &watcher_session_id,
                        status: run_ended_status(state.status),
                        duration_ms: elapsed,
                        agents_used: state.agents_used,
                        agent_budget: state.agent_budget,
                    },
                    &watcher_agent_stats,
                );
                if !persisted {
                    let _ = done_tx.send(());
                    let _ = outcome_tx.send(outcome);
                    return;
                }
                notify.broadcast(&state, elapsed, 0, true);
                if state.status.is_completion_reportable() {
                    (hooks.completion)(&watcher_run_id, state.revision);
                }
            }
            let _ = done_tx.send(());
            let _ = outcome_tx.send(outcome);
        });

        Ok((run_id, outcome_rx))
    }

    fn reap_terminal_runs(&mut self) {
        let terminal: Vec<String> = self
            .active
            .keys()
            .filter(|run_id| {
                self.tracker
                    .lock()
                    .get(run_id)
                    .is_some_and(|state| state.status.is_terminal())
            })
            .cloned()
            .collect();
        for run_id in terminal {
            if let Some(run) = self.active.remove(&run_id) {
                self.retiring.push((run_id.to_owned(), run.done));
            }
        }

        self.retiring.retain_mut(|(_, done)| match done.try_recv() {
            Ok(()) | Err(oneshot::error::TryRecvError::Closed) => false,
            Err(oneshot::error::TryRecvError::Empty) => true,
        });
    }

    fn reap_if_terminal(&mut self, run_id: &str) -> bool {
        let terminal = self
            .tracker
            .lock()
            .get(run_id)
            .is_some_and(|s| s.status.is_terminal());
        if terminal && let Some(run) = self.active.remove(run_id) {
            self.retiring.push((run_id.to_owned(), run.done));
        }
        terminal
    }

    fn cancel_children_for_run(&self, run_id: &str) -> bool {
        self.backend.request_cancel_run_children(run_id)
    }

    pub fn pause(&mut self, run_id: &str) -> bool {
        if self.reap_if_terminal(run_id) {
            return false;
        }
        let Some(run) = self.active.remove(run_id) else {
            return false;
        };
        run.pause_intent.store(true, Ordering::Relaxed);
        run.cancel.cancel();
        let _ = self.cancel_children_for_run(run_id);
        self.retiring.push((run_id.to_owned(), run.done));
        let paused = {
            let mut tracker = self.tracker.lock();
            let active = tracker
                .get(run_id)
                .is_some_and(|state| !state.status.is_terminal());
            if active {
                let state = tracker.pause_user(run_id, None);
                state.map(|state| (state, tracker.elapsed_ms(run_id)))
            } else {
                None
            }
        };
        if let Some((state, elapsed)) = paused {
            self.notify.emit(&state, elapsed, 0);
        }
        true
    }

    pub fn cancel(&mut self, run_id: &str) -> bool {
        if self.reap_if_terminal(run_id) {
            return false;
        }
        if let Some(run) = self.active.remove(run_id) {
            run.cancel.cancel();
            let _ = self.cancel_children_for_run(run_id);
            self.retiring.push((run_id.to_owned(), run.done));
            let state = {
                let mut tracker = self.tracker.lock();
                match tracker.get(run_id) {
                    Some(state) if !state.status.is_terminal() => {
                        tracker.apply_outcome(run_id, &WorkflowOutcome::Cancelled)
                    }
                    _ => None,
                }
            };
            if state.is_some() {
                let (state, elapsed) = {
                    let tracker = self.tracker.lock();
                    (
                        tracker.get(run_id).expect("run still tracked"),
                        tracker.elapsed_ms(run_id),
                    )
                };
                self.notify.emit(&state, elapsed, 0);
            }
            return true;
        }
        let _ = self.cancel_children_for_run(run_id);
        let state = {
            let mut tracker = self.tracker.lock();
            match tracker.get(run_id) {
                Some(state) if !state.status.is_terminal() => {
                    tracker.apply_outcome(run_id, &WorkflowOutcome::Cancelled)
                }
                _ => None,
            }
        };
        match state {
            Some(_) => {
                let (state, elapsed) = {
                    let tracker = self.tracker.lock();
                    (
                        tracker.get(run_id).expect("run still tracked"),
                        tracker.elapsed_ms(run_id),
                    )
                };
                self.notify.emit(&state, elapsed, 0);
                true
            }
            None => false,
        }
    }

    /// Pause or stop the run whose id or display name is `key`, returning its
    /// state as of before the op.
    pub fn control_run(
        &mut self,
        key: &str,
        control: WorkflowControl,
    ) -> Result<WorkflowRunState, ControlError> {
        let run = {
            let tracker = self.tracker.lock();
            tracker
                .find_run_id(key)
                .and_then(|run_id| tracker.get(&run_id))
        }
        .ok_or_else(|| ControlError::UnknownRun(key.to_owned()))?;
        let not_applicable = |status: WorkflowRunStatus| ControlError::NotApplicable {
            name: run.name.clone(),
            status,
            control,
        };
        if !run.status.accepts(control) {
            return Err(not_applicable(run.status));
        }
        let applied = match control {
            WorkflowControl::Pause => self.pause(&run.run_id),
            WorkflowControl::Stop => self.cancel(&run.run_id),
        };
        if applied {
            Ok(run)
        } else {
            // The run finished between the gate and the op; report the status it reached.
            let status = self
                .tracker
                .lock()
                .get(&run.run_id)
                .map_or(run.status, |state| state.status);
            Err(not_applicable(status))
        }
    }

    /// Close an External session scope permanently, with an explicit child-drain ACK.
    pub async fn shutdown(&mut self, timeout: std::time::Duration) -> Result<(), Vec<String>> {
        if self.closed {
            return self.shutdown_error.clone().map_or(Ok(()), Err);
        }
        self.closed = true;
        let run_ids: std::collections::HashSet<String> = self
            .active
            .keys()
            .cloned()
            .chain(self.retiring.iter().map(|(id, _)| id.clone()))
            .collect();
        if let Err(error) = self.cancel_all_and_drain(timeout).await {
            self.shutdown_error = Some(error.clone());
            return Err(error);
        }
        let mut failed = Vec::new();
        for run_id in run_ids {
            if self.backend.cancel_run_children(&run_id).await != HostDrainOutcome::Drained {
                failed.push(run_id);
            }
        }
        if failed.is_empty() {
            Ok(())
        } else {
            self.shutdown_error = Some(failed.clone());
            Err(failed)
        }
    }

    pub async fn cancel_all_and_drain(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<(), Vec<String>> {
        let active: Vec<(String, ActiveRun)> = self.active.drain().collect();
        for (run_id, run) in &active {
            run.cancel.cancel();
            let _ = self.cancel_children_for_run(run_id);
        }
        let mut pending: Vec<(Option<String>, oneshot::Receiver<()>)> = active
            .into_iter()
            .map(|(run_id, run)| (Some(run_id), run.done))
            .collect();
        pending.extend(
            self.retiring
                .drain(..)
                .map(|(run_id, done)| (Some(run_id), done)),
        );

        let mut timed_out = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;
        let mut pending = pending.into_iter();
        while let Some((run_id, done)) = pending.next() {
            if tokio::time::timeout_at(deadline, done).await.is_err() {
                if let Some(run_id) = run_id {
                    timed_out.push(run_id);
                }
                timed_out.extend(pending.filter_map(|(run_id, _)| run_id));
                break;
            }
        }
        if timed_out.is_empty() {
            return Ok(());
        }

        tracing::warn!(
            run_ids = ?timed_out,
            "workflow shutdown drain timed out; marking runs interrupted"
        );
        for run_id in &timed_out {
            if self
                .tracker
                .lock()
                .get(run_id)
                .is_some_and(|s| s.status.is_terminal() || s.status.is_paused())
            {
                continue;
            }
            let state = {
                let mut tracker = self.tracker.lock();
                tracker.interrupt(
                    run_id,
                    "session shutdown timed out before workflow cleanup completed; the run cannot resume",
                )
            };
            if let Some(state) = state {
                if let Err(error) = self.store.persist_now(&state) {
                    tracing::error!(%run_id, %error, "failed to persist workflow shutdown interruption");
                }
                let elapsed = self.tracker.lock().elapsed_ms(run_id);
                self.notify.broadcast(&state, elapsed, 0, true);
            }
        }
        Err(timed_out)
    }

    #[cfg(test)]
    pub fn test_insert_active_run(&mut self, run_id: String, done: oneshot::Receiver<()>) {
        self.active.insert(
            run_id,
            ActiveRun {
                cancel: CancellationToken::new(),
                pause_intent: Arc::new(AtomicBool::new(false)),
                done,
            },
        );
    }

    pub fn script_copy_for(&self, run_id: &str) -> Option<String> {
        self.store.script_for(run_id)
    }

    pub fn script_copy_path(&self, run_id: &str) -> Option<std::path::PathBuf> {
        self.store.script_copy_path(run_id)
    }

    pub fn args_copy_for(&self, run_id: &str) -> serde_json::Value {
        self.store
            .args_for(run_id)
            .unwrap_or(serde_json::Value::Null)
    }
}

fn log_run_started(
    hooks: &WorkflowRuntimeHooks,
    run_id: &str,
    parent_session_id: &str,
    source: &WorkflowSource,
    state: &WorkflowRunState,
    max_concurrent_agents: usize,
    resumed: bool,
) {
    (hooks.lifecycle)(WorkflowRuntimeEvent::RunStarted {
        run_id,
        parent_session_id,
        source,
        state,
        max_concurrent_agents,
        resumed,
    });
}
struct RunEndMetadata<'a> {
    run_id: &'a str,
    parent_session_id: &'a str,
    status: WorkflowEpisodeStatus,
    duration_ms: u64,
    agents_used: u64,
    agent_budget: Option<u64>,
}
fn log_run_ended(
    hooks: &WorkflowRuntimeHooks,
    episode: RunEndMetadata<'_>,
    stats: &super::host_service::WorkflowAgentStats,
) {
    (hooks.lifecycle)(WorkflowRuntimeEvent::RunEnded {
        run_id: episode.run_id,
        parent_session_id: episode.parent_session_id,
        status: episode.status,
        duration_ms: episode.duration_ms,
        agents_used: episode.agents_used,
        agent_budget: episode.agent_budget,
        stats,
    });
}
fn run_ended_status(status: WorkflowRunStatus) -> WorkflowEpisodeStatus {
    WorkflowEpisodeStatus::Run(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::WorkflowAgentSpawnResult as SubagentResult;
    use crate::registry::resolve_inline;
    use crate::test_backend::{TestEvent as SubagentEvent, TestSpawn as SubagentSpawnRequest};
    type SubagentEventRx = mpsc::UnboundedReceiver<SubagentEvent>;
    type CancelLog = Arc<parking_lot::Mutex<Vec<String>>>;
    fn test_manager(session_dir: Option<PathBuf>) -> (WorkflowManager, SubagentEventRx) {
        let (manager, events, _) = test_manager_with_cancels(session_dir);
        (manager, events)
    }
    fn test_manager_with_cancels(
        session_dir: Option<PathBuf>,
    ) -> (WorkflowManager, SubagentEventRx, CancelLog) {
        let (tx, rx) = mpsc::unbounded_channel();
        let cancels = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let store = WorkflowRunStore::standalone(session_dir.clone());
        let notify = WorkflowNotifySender::new(store.clone(), Arc::new(|_, _| {}));
        let manager = WorkflowManager::new(
            "test-session".into(),
            session_dir,
            std::env::temp_dir(),
            Arc::new(parking_lot::Mutex::new(WorkflowTracker::default())),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            store,
            notify,
            Arc::new(crate::test_backend::ChannelBackend {
                tx,
                cancels: cancels.clone(),
            }),
            Arc::new(|_, _, _| {}),
            WorkflowRuntimeHooks::default(),
            HashMap::new(),
            super::super::host_service::DEFAULT_WORKFLOW_MAX_CONCURRENT_AGENTS,
        );
        (manager, rx, cancels)
    }

    fn spec() -> LaunchSpec {
        LaunchSpec {
            objective: "obj".into(),
            args: serde_json::json!({}),
            agent_budget: None,
            effort: None,
            resume_run_id: None,
        }
    }

    fn parallel_n_script(n: usize) -> String {
        format!(
            "let meta = #{{ name: \"t\", description: \"d\" }};\n\
             let jobs = [];\n\
             let i = 0;\n\
             while i < {n} {{\n\
                 jobs.push(#{{ prompt: \"work \" + i.to_string() }});\n\
                 i += 1;\n\
             }}\n\
             let results = parallel(jobs);\n\
             complete(results.len());"
        )
    }

    async fn recv_spawn(rx: &mut SubagentEventRx) -> SubagentSpawnRequest {
        match tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await {
            Ok(Some(SubagentEvent::Spawn(req))) => req,
            Ok(Some(_)) => panic!("expected spawn, got a non-spawn event"),
            Ok(None) => panic!("expected spawn, channel closed"),
            Err(_) => panic!("expected spawn, timed out"),
        }
    }

    fn complete_spawn(req: SubagentSpawnRequest) {
        let id = req.id.clone();
        let _ = req.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("ok"),

            child_session_id: id,
            ..crate::test_backend::empty_result()
        });
    }

    #[tokio::test]
    async fn shutdown_is_idempotent_and_prevents_stale_scope_launches() {
        let (mut manager, _) = test_manager(None);
        let (_, outcome) = manager
            .launch(
                resolve_inline(
                    "let meta = #{ name: \"scope\", description: \"d\" }; complete(\"done\");"
                        .into(),
                )
                .unwrap(),
                spec(),
            )
            .unwrap();
        outcome.await.unwrap();
        manager
            .shutdown(std::time::Duration::from_secs(2))
            .await
            .unwrap();
        manager
            .shutdown(std::time::Duration::from_secs(2))
            .await
            .unwrap();
        assert!(matches!(
            manager.launch(
                resolve_inline(
                    "let meta = #{ name: \"stale\", description: \"d\" }; complete(\"bad\");"
                        .into()
                )
                .unwrap(),
                spec()
            ),
            Err(LaunchError::SessionClosed)
        ));
    }

    #[tokio::test]
    async fn launch_completes_and_updates_tracker() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, _rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\ncomplete(\"done\");".into(),
        )
        .unwrap();
        let (run_id, outcome_rx) = manager.launch(resolved, spec()).unwrap();

        let outcome = outcome_rx.await.unwrap();
        assert!(matches!(outcome, WorkflowOutcome::Completed { .. }));
        let state = manager.tracker.lock().get(&run_id).unwrap();
        assert_eq!(state.status, crate::tracker::WorkflowRunStatus::Complete);
        assert_eq!(state.result_summary.as_deref(), Some("done"));
        assert!(
            dir.path()
                .join("workflows")
                .join(&run_id)
                .join("script.rhai")
                .exists()
        );
    }

    #[tokio::test]
    async fn running_workflow_keeps_session_active_work_nonzero() {
        use std::sync::atomic::Ordering;
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut rx) = test_manager(Some(dir.path().to_path_buf()));

        assert_eq!(
            manager.active_work.load(Ordering::Acquire),
            0,
            "no active work before any run launches"
        );

        let (_run_id, outcome_rx) = manager
            .launch(resolve_inline(parallel_n_script(1)).unwrap(), spec())
            .unwrap();
        let req = recv_spawn(&mut rx).await;
        assert!(
            manager.active_work.load(Ordering::Acquire) > 0,
            "a running workflow must count toward the session's active work (GBT-6282)"
        );

        complete_spawn(req);
        assert!(matches!(
            outcome_rx.await.unwrap(),
            WorkflowOutcome::Completed { .. }
        ));
        for _ in 0..200 {
            if manager.active_work.load(Ordering::Acquire) == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(
            manager.active_work.load(Ordering::Acquire),
            0,
            "active work returns to zero once the run completes"
        );
    }

    #[tokio::test]
    async fn plain_resume_uses_immutable_script_not_edited_projection() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, _rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\nawait_user(\"user\", \"pause\");\ncomplete(\"original\");";
        let (run_id, outcome_rx) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();
        assert!(matches!(
            outcome_rx.await.unwrap(),
            WorkflowOutcome::Paused { .. }
        ));
        std::fs::write(
            manager.script_copy_path(&run_id).unwrap(),
            "let meta = #{ name: \"t\", description: \"d\" };\ncomplete(\"edited\");",
        )
        .unwrap();

        let (_same_id, outcome_rx) = manager
            .launch(
                resolve_inline(
                    "let meta = #{ name: \"t\", description: \"d\" };\ncomplete(\"caller copy\");"
                        .into(),
                )
                .unwrap(),
                LaunchSpec {
                    resume_run_id: Some(run_id),
                    ..spec()
                },
            )
            .unwrap();
        match outcome_rx.await.unwrap() {
            WorkflowOutcome::Completed { result } => {
                assert_eq!(result, serde_json::json!("original"));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn resume_reuses_immutable_launch_effort() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\n\
                      await_user(\"user\", \"pause\");\n\
                      let r = agent(\"after resume\");\n\
                      complete(r.output);";
        let (run_id, first_outcome) = manager
            .launch(
                resolve_inline(script.into()).unwrap(),
                LaunchSpec {
                    effort: Some("high".into()),
                    ..spec()
                },
            )
            .unwrap();
        assert!(matches!(
            first_outcome.await.unwrap(),
            WorkflowOutcome::Paused { .. }
        ));

        let (_same_id, resumed_outcome) = manager
            .launch(
                resolve_inline(script.into()).unwrap(),
                LaunchSpec {
                    effort: None,
                    resume_run_id: Some(run_id),
                    ..spec()
                },
            )
            .unwrap();
        let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("resumed spawn") else {
            panic!("expected resumed spawn event");
        };
        assert_eq!(req.reasoning_effort.as_deref(), Some("high"));
        complete_spawn(req);
        assert!(matches!(
            resumed_outcome.await.unwrap(),
            WorkflowOutcome::Completed { .. }
        ));
    }

    #[tokio::test]
    async fn pause_eagerly_marks_user_paused() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, _rx) = test_manager(Some(dir.path().to_path_buf()));
        let run_id = "wf_pause_eager".to_string();
        manager
            .store
            .register(
                &run_id,
                "let meta = #{ name: \"t\", description: \"d\" };",
                &serde_json::json!({}),
                None,
            )
            .unwrap();
        manager.tracker.lock().start_run(
            run_id.clone(),
            "t".into(),
            "obj".into(),
            Vec::new(),
            None,
            None,
        );
        let (_done_tx, done_rx) = oneshot::channel();
        manager.test_insert_active_run(run_id.clone(), done_rx);

        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().status,
            crate::tracker::WorkflowRunStatus::Active,
        );
        assert!(manager.pause(&run_id));
        let state = manager.tracker.lock().get(&run_id).unwrap();
        assert_eq!(
            state.status,
            crate::tracker::WorkflowRunStatus::UserPaused,
            "pause() must eagerly mark UserPaused so status is not still Active"
        );
        assert!(!manager.active.contains_key(&run_id));
    }

    #[tokio::test]
    async fn pause_marks_user_paused_and_resume_replays() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\nlet r = agent(\"work\");\ncomplete(r.output);";
        let resolved = resolve_inline(script.into()).unwrap();
        let (run_id, outcome_rx) = manager.launch(resolved, spec()).unwrap();

        let spawn_req = subagent_rx.recv().await.expect("spawn request");
        let SubagentEvent::Spawn(_spawn) = spawn_req else {
            panic!("expected spawn request");
        };
        assert!(manager.pause(&run_id));
        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().status,
            crate::tracker::WorkflowRunStatus::UserPaused,
            "pause() must mark UserPaused immediately"
        );
        let outcome = outcome_rx.await.unwrap();
        assert!(matches!(outcome, WorkflowOutcome::Cancelled));
        let state = manager.tracker.lock().get(&run_id).unwrap();
        assert_eq!(
            state.status,
            crate::tracker::WorkflowRunStatus::UserPaused,
            "pause intent must map Cancelled → UserPaused"
        );

        let resolved = resolve_inline(script.into()).unwrap();
        let (_run_id2, outcome_rx) = manager
            .launch(
                resolved,
                LaunchSpec {
                    resume_run_id: Some(run_id.clone()),
                    ..spec()
                },
            )
            .unwrap();
        let spawn_req = subagent_rx.recv().await.expect("respawned agent");
        if let SubagentEvent::Spawn(req) = spawn_req {
            let _ = req.result_tx.send(SubagentResult {
                success: true,
                output: std::sync::Arc::from("resumed output"),

                ..crate::test_backend::empty_result()
            });
        } else {
            panic!("expected spawn event");
        }
        let outcome = outcome_rx.await.unwrap();
        match outcome {
            WorkflowOutcome::Completed { result } => {
                assert_eq!(result, serde_json::json!("resumed output"));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn resume_reconciles_agents_used_from_journal_no_double_charge() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\nlet r = agent(\"work\");\ncomplete(r.output);";
        let (run_id, outcome_rx) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();

        let SubagentEvent::Spawn(_first) = subagent_rx.recv().await.expect("first spawn") else {
            panic!("expected spawn event");
        };
        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().agents_used,
            1,
            "the live agent reserves one slot before it spawns"
        );

        assert!(manager.pause(&run_id));
        assert!(matches!(
            outcome_rx.await.unwrap(),
            WorkflowOutcome::Cancelled
        ));
        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().agents_used,
            1,
            "cancel tears the host down before the release lands, so the reserved slot leaks in memory"
        );

        let (_resumed_id, outcome_rx) = manager
            .launch(
                resolve_inline(script.into()).unwrap(),
                LaunchSpec {
                    resume_run_id: Some(run_id.clone()),
                    ..spec()
                },
            )
            .unwrap();
        let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("respawned agent") else {
            panic!("expected respawn event");
        };
        let _ = req.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("resumed output"),

            ..crate::test_backend::empty_result()
        });
        assert!(matches!(
            outcome_rx.await.unwrap(),
            WorkflowOutcome::Completed { .. }
        ));

        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().agents_used,
            1,
            "resume reconciles agents_used from the journal (0 journaled) then re-reserves once; \
             without the reconcile the leaked slot double-charges to 2"
        );
    }

    #[tokio::test]
    async fn failed_run_resumes_and_reexecutes_failed_host_call_live() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, _rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\n\
                      let content = read_scratch_file(\"data.txt\");\n\
                      complete(content);";
        let (run_id, outcome_rx) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();
        match outcome_rx.await.unwrap() {
            WorkflowOutcome::Failed { error } => {
                assert!(error.contains("scratch"), "{error}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().status,
            crate::tracker::WorkflowRunStatus::Failed
        );
        let journal_path = dir
            .path()
            .join("workflows")
            .join(&run_id)
            .join("journal.jsonl");
        assert!(
            std::fs::read_to_string(&journal_path)
                .unwrap()
                .contains("__xai_workflow_host_error"),
            "the uncaught host error must be journaled as a trailing sentinel"
        );

        let scratch = dir.path().join("workflows").join(&run_id).join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join("data.txt"), "hello").unwrap();

        let (_same_id, outcome_rx) = manager
            .launch(
                resolve_inline(script.into()).unwrap(),
                LaunchSpec {
                    resume_run_id: Some(run_id.clone()),
                    ..spec()
                },
            )
            .unwrap();
        match outcome_rx.await.unwrap() {
            WorkflowOutcome::Completed { result } => {
                assert_eq!(
                    result,
                    serde_json::json!("hello"),
                    "the failed host call must go live instead of replaying the sentinel"
                );
            }
            other => panic!("expected Completed, got {other:?}"),
        }
        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().status,
            crate::tracker::WorkflowRunStatus::Complete
        );
        assert!(
            !std::fs::read_to_string(&journal_path)
                .unwrap()
                .contains("__xai_workflow_host_error"),
            "the trailing sentinel must be pruned and replaced by the live result"
        );
    }

    #[tokio::test]
    async fn completed_and_interrupted_runs_are_not_resumable() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\nlet a = agent(\"step one\");\ncomplete(a.output);";
        let (run_id, outcome_rx) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();
        let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("first spawn") else {
            panic!("expected spawn event");
        };
        let _ = req.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("one"),

            ..crate::test_backend::empty_result()
        });
        assert!(matches!(
            outcome_rx.await.unwrap(),
            WorkflowOutcome::Completed { .. }
        ));

        let state = manager.tracker.lock().get(&run_id).unwrap();
        for status in [
            crate::tracker::WorkflowRunStatus::Complete,
            crate::tracker::WorkflowRunStatus::Interrupted,
        ] {
            let mut restored = state.clone();
            restored.status = status;
            let original_tracker = manager.tracker.clone();
            manager.tracker = Arc::new(parking_lot::Mutex::new(WorkflowTracker::from_snapshot(
                vec![restored],
            )));
            let err = manager
                .launch(
                    resolve_inline(script.into()).unwrap(),
                    LaunchSpec {
                        resume_run_id: Some(run_id.clone()),
                        ..spec()
                    },
                )
                .unwrap_err();
            manager.tracker = original_tracker;
            assert!(
                matches!(err, LaunchError::NotResumable(_)),
                "{status:?}: {err}"
            );
        }

        let mut cancelled = state.clone();
        cancelled.status = crate::tracker::WorkflowRunStatus::Cancelled;
        manager.tracker = Arc::new(parking_lot::Mutex::new(WorkflowTracker::from_snapshot(
            vec![cancelled],
        )));
        manager
            .launch(
                resolve_inline(script.into()).unwrap(),
                LaunchSpec {
                    resume_run_id: Some(run_id.clone()),
                    ..spec()
                },
            )
            .expect("cancelled /workflow stop runs stay resumable from the journal");
    }

    #[tokio::test]
    async fn shutdown_timeout_marks_active_run_interrupted() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, _rx) = test_manager(Some(dir.path().to_path_buf()));
        let run_id = "wf_timeout".to_string();
        manager
            .store
            .register(
                &run_id,
                "let meta = #{ name: \"t\", description: \"d\" };",
                &serde_json::json!({}),
                None,
            )
            .unwrap();
        manager.tracker.lock().start_run(
            run_id.clone(),
            "t".into(),
            "obj".into(),
            Vec::new(),
            None,
            None,
        );
        let (_done_tx, done_rx) = oneshot::channel();
        manager.test_insert_active_run(run_id.clone(), done_rx);

        let result = manager
            .cancel_all_and_drain(std::time::Duration::from_millis(1))
            .await;
        assert_eq!(result.unwrap_err(), vec![run_id.clone()]);
        let state = manager.tracker.lock().get(&run_id).unwrap();
        assert_eq!(state.status, crate::tracker::WorkflowRunStatus::Interrupted);
        assert!(!state.status.is_paused());
    }

    #[tokio::test]
    async fn workflow_spawns_await_to_completion() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let r = agent(\"work\");\n\
             complete(r.output);"
                .into(),
        )
        .unwrap();
        let (_run_id, outcome_rx) = manager.launch(resolved, spec()).unwrap();

        let spawn_req = subagent_rx.recv().await.expect("spawn event");
        let SubagentEvent::Spawn(req) = spawn_req else {
            panic!("expected spawn event");
        };
        assert_eq!(req.reasoning_effort, None);
        let _ = req.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("slow but done"),

            ..crate::test_backend::empty_result()
        });
        let outcome = outcome_rx.await.unwrap();
        assert!(matches!(outcome, WorkflowOutcome::Completed { .. }));
    }

    #[tokio::test]
    async fn launch_effort_applies_to_children_and_child_override_wins() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let results = parallel([\n\
                 #{ prompt: \"inherits\" },\n\
                 #{ prompt: \"overrides\", effort: \"LoW\" },\n\
             ]);\n\
             complete(results.len());"
                .into(),
        )
        .unwrap();
        let (_run_id, outcome_rx) = manager
            .launch(
                resolved,
                LaunchSpec {
                    effort: Some("high".into()),
                    ..spec()
                },
            )
            .unwrap();

        let mut efforts = HashMap::new();
        for _ in 0..2 {
            let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("spawn") else {
                panic!("expected spawn event");
            };
            efforts.insert(
                req.request.prompt.clone(),
                req.request.reasoning_effort.clone(),
            );
            complete_spawn(req);
        }
        assert_eq!(
            efforts.get("inherits").and_then(Option::as_deref),
            Some("high")
        );
        assert_eq!(
            efforts.get("overrides").and_then(Option::as_deref),
            Some("low")
        );
        assert!(matches!(
            outcome_rx.await.unwrap(),
            WorkflowOutcome::Completed { .. }
        ));
    }

    #[tokio::test]
    async fn agent_rejects_invalid_effort_before_spawning() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             agent(\"work\", #{ effort: \"turbo\" });"
                .into(),
        )
        .unwrap();
        let (_run_id, outcome_rx) = manager.launch(resolved, spec()).unwrap();

        match outcome_rx.await.unwrap() {
            WorkflowOutcome::Failed { error } => {
                assert!(error.contains("invalid workflow agent effort"), "{error}");
                assert!(error.contains("turbo"), "{error}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(
            subagent_rx.try_recv().is_err(),
            "invalid effort must not reach the coordinator"
        );
    }

    #[tokio::test]
    async fn parallel_nulls_invalid_child_effort_without_spawning() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let results = parallel([#{ prompt: \"work\", effort: \"turbo\" }]);\n\
             complete(results);"
                .into(),
        )
        .unwrap();
        let (_run_id, outcome_rx) = manager.launch(resolved, spec()).unwrap();

        match outcome_rx.await.unwrap() {
            WorkflowOutcome::Completed { result } => {
                assert_eq!(result, serde_json::json!([null]));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
        assert!(
            subagent_rx.try_recv().is_err(),
            "invalid effort must not reach the coordinator"
        );
    }

    #[tokio::test]
    async fn active_run_admission_is_bounded_per_session() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\n\
                      let r = agent(\"work\");\ncomplete(r.output);";
        let mut outcomes = Vec::new();
        let mut spawned = Vec::new();
        for _ in 0..WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION {
            let (_, outcome) = manager
                .launch(resolve_inline(script.into()).unwrap(), spec())
                .unwrap();
            outcomes.push(outcome);
            spawned.push(subagent_rx.recv().await.expect("spawn event"));
        }
        let error = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap_err();
        assert!(matches!(error, LaunchError::TooManyActiveRuns));
        drop(spawned);
        let _ = manager
            .cancel_all_and_drain(std::time::Duration::from_secs(1))
            .await;
        drop(outcomes);
    }

    #[tokio::test]
    async fn retiring_runs_still_consume_session_admission() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, _subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let mut done_senders = Vec::new();
        for index in 0..WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION {
            let (done_tx, done_rx) = oneshot::channel();
            manager
                .retiring
                .push((format!("retiring-{index}"), done_rx));
            done_senders.push(done_tx);
            assert_eq!(manager.retiring.len(), index + 1);
        }

        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\ncomplete(\"done\");".into(),
        )
        .unwrap();
        assert!(matches!(
            manager.launch(resolved, spec()).unwrap_err(),
            LaunchError::TooManyActiveRuns
        ));

        done_senders.pop().unwrap().send(()).unwrap();
        manager.reap_terminal_runs();
        assert_eq!(
            manager.retiring.len(),
            WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION - 1
        );
        assert!(
            manager.active.len().saturating_add(manager.retiring.len())
                < WORKFLOW_MAX_ACTIVE_RUNS_PER_SESSION
        );
        drop(done_senders);
    }

    #[tokio::test]
    async fn untrusted_workflow_cannot_fork_parent_context() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let r = agent(\"work\", #{ fork_context: true });\n\
             complete(r.output);"
                .into(),
        )
        .unwrap();
        let (_run_id, outcome_rx) = manager.launch(resolved, spec()).unwrap();

        match outcome_rx.await.unwrap() {
            WorkflowOutcome::Failed { error } => {
                assert!(error.contains("fork_context is restricted to built-in workflows"));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(
            subagent_rx.try_recv().is_err(),
            "rejected fork_context must not reach the coordinator"
        );
    }

    #[tokio::test]
    async fn output_schema_stays_host_side_with_one_corrective_retry() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let r = agent(\"scan\", #{ output_schema: #{ \"type\": \"object\", \
             \"required\": [\"ok\"], \"properties\": #{ \"ok\": #{ \"type\": \"boolean\" } } } });\n\
             complete(r.output.ok);"
                .into(),
        )
        .unwrap();
        let (_run_id, outcome_rx) = manager
            .launch(
                resolved,
                LaunchSpec {
                    agent_budget: Some(5),
                    ..spec()
                },
            )
            .unwrap();

        let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("first spawn") else {
            panic!("expected spawn event");
        };
        assert!(
            req.prompt.contains("<output-contract>"),
            "prompt must carry the schema contract"
        );
        assert!(req.resume_from.is_none());
        let first_id = req.id.clone();
        let _ = req.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("All files scanned, nothing found."),

            child_session_id: first_id.clone(),

            total_tokens_used: 100,
            ..crate::test_backend::empty_result()
        });

        let SubagentEvent::Spawn(retry) = subagent_rx.recv().await.expect("corrective retry")
        else {
            panic!("expected retry spawn event");
        };
        assert_eq!(retry.resume_from.as_deref(), Some(first_id.as_str()));
        assert!(retry.prompt.contains("did not satisfy the output contract"));
        let retry_id = retry.id.clone();
        let _ = retry.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("```json\n{\"ok\": true}\n```"),

            child_session_id: retry_id,

            total_tokens_used: 50,
            ..crate::test_backend::empty_result()
        });

        let outcome = outcome_rx.await.unwrap();
        match outcome {
            WorkflowOutcome::Completed { result } => {
                assert_eq!(result, serde_json::json!(true));
            }
            other => panic!("expected Completed, got {other:?}"),
        }
        let state = manager.tracker.lock().list().into_iter().next().unwrap();
        assert_eq!(state.agents_used, 1, "schema retry is one logical agent");
        assert_eq!(state.agent_budget, Some(5));
    }

    #[tokio::test]
    async fn explicit_max_output_tokens_is_ignored_and_run_charges_totals() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let r = agent(\"work\", #{ max_output_tokens: 900 });\n\
             complete(r.output);"
                .into(),
        )
        .unwrap();
        let (run_id, outcome_rx) = manager
            .launch(
                resolved,
                LaunchSpec {
                    agent_budget: Some(2),
                    ..spec()
                },
            )
            .unwrap();
        let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("spawn") else {
            panic!("expected spawn");
        };
        let id = req.id.clone();
        let _ = req.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("done"),

            child_session_id: id,

            total_tokens_used: 120,
            ..crate::test_backend::empty_result()
        });
        assert!(matches!(
            outcome_rx.await.unwrap(),
            WorkflowOutcome::Completed { .. }
        ));
        let state = manager.tracker.lock().get(&run_id).unwrap();
        assert_eq!(state.agents_used, 1);
        assert_eq!(state.agent_budget, Some(2));
    }

    #[tokio::test]
    async fn children_spawn_without_output_clamp() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let r = agent(\"work\");\ncomplete(r.output);"
                .into(),
        )
        .unwrap();
        let (_run_id, _outcome_rx) = manager.launch(resolved, spec()).unwrap();
        let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("spawn") else {
            panic!("expected spawn");
        };
        let id = req.id.clone();
        let _ = req.result_tx.send(SubagentResult {
            success: true,
            output: std::sync::Arc::from("done"),

            child_session_id: id,

            ..crate::test_backend::empty_result()
        });
    }

    #[tokio::test]
    async fn cancellation_uses_run_owned_cancel_event_without_parent_detach() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx, cancels) =
            test_manager_with_cancels(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let r = agent(\"work\");\ncomplete(r.output);"
                .into(),
        )
        .unwrap();
        let (run_id, outcome_rx) = manager
            .launch(
                resolved,
                LaunchSpec {
                    agent_budget: Some(100),
                    ..spec()
                },
            )
            .unwrap();
        let SubagentEvent::Spawn(req) = subagent_rx.recv().await.expect("spawn") else {
            panic!("expected spawn");
        };
        assert_eq!(req.run_id, run_id);
        assert!(manager.cancel(&run_id));
        let _ = outcome_rx.await;
        assert!(
            req.cancel_token.is_cancelled(),
            "run cancel must cancel the child token, not silently detach the receiver"
        );
        assert!(
            cancels.lock().iter().any(|target| matches!(
                target,
                id if id == &run_id
            )),
            "cancellation must emit an explicit run-owned cancel event"
        );
    }

    #[tokio::test]
    async fn control_run_stops_by_run_id_and_pauses_by_display_name() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\n\
                      let r = agent(\"work\");\ncomplete(r.output);";

        let (stopped_id, stopped_outcome) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();
        let stopped_child = recv_spawn(&mut subagent_rx).await;
        let (paused_id, paused_outcome) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();
        let paused_child = recv_spawn(&mut subagent_rx).await;
        let paused_name = manager.tracker.lock().get(&paused_id).unwrap().name;

        assert_eq!(
            manager
                .control_run("wf_missing", WorkflowControl::Stop)
                .unwrap_err(),
            ControlError::UnknownRun("wf_missing".to_owned())
        );

        let stopped = manager
            .control_run(&stopped_id, WorkflowControl::Stop)
            .unwrap();
        assert_eq!(
            (stopped.run_id.as_str(), stopped.name.as_str()),
            (stopped_id.as_str(), "t")
        );
        assert!(matches!(
            stopped_outcome.await.unwrap(),
            WorkflowOutcome::Cancelled
        ));
        assert!(stopped_child.cancel_token.is_cancelled());
        let stopped_state = manager.tracker.lock().get(&stopped_id).unwrap();
        assert_eq!(stopped_state.status, WorkflowRunStatus::Cancelled);
        // The slash path relies on the completion wake to tell the model about a user stop.
        assert!(
            manager
                .tracker
                .lock()
                .is_unreported_completion(&stopped_id, stopped_state.revision)
        );
        assert_eq!(
            manager
                .control_run(&stopped_id, WorkflowControl::Stop)
                .unwrap_err(),
            ControlError::NotApplicable {
                name: "t".to_owned(),
                status: WorkflowRunStatus::Cancelled,
                control: WorkflowControl::Stop,
            }
        );

        let paused = manager
            .control_run(&paused_name, WorkflowControl::Pause)
            .unwrap();
        assert_eq!(paused.run_id, paused_id);
        // A user pause cancels the engine; the pause intent maps that outcome to UserPaused.
        assert!(matches!(
            paused_outcome.await.unwrap(),
            WorkflowOutcome::Cancelled
        ));
        assert!(paused_child.cancel_token.is_cancelled());
        assert_eq!(
            manager.tracker.lock().get(&paused_id).unwrap().status,
            WorkflowRunStatus::UserPaused
        );
        assert_eq!(
            manager
                .control_run(&paused_name, WorkflowControl::Pause)
                .unwrap_err(),
            ControlError::NotApplicable {
                name: paused_name.clone(),
                status: WorkflowRunStatus::UserPaused,
                control: WorkflowControl::Pause,
            }
        );
        assert!(
            manager
                .control_run(&paused_name, WorkflowControl::Stop)
                .is_ok()
        );
        assert_eq!(
            manager.tracker.lock().get(&paused_id).unwrap().status,
            WorkflowRunStatus::Cancelled
        );
    }

    #[tokio::test]
    async fn control_run_refuses_to_stop_a_budget_limited_run_so_resume_still_needs_a_raised_cap() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\n\
                      let r = agent(\"work\");\ncomplete(r.output);";
        let (run_id, _outcome) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();
        let _child = recv_spawn(&mut subagent_rx).await;
        manager.tracker.lock().apply_outcome(
            &run_id,
            &WorkflowOutcome::BudgetExceeded {
                message: "budget".into(),
            },
        );

        assert_eq!(
            manager
                .control_run(&run_id, WorkflowControl::Stop)
                .unwrap_err(),
            ControlError::NotApplicable {
                name: "t".to_owned(),
                status: WorkflowRunStatus::BudgetLimited,
                control: WorkflowControl::Stop,
            }
        );
        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().status,
            WorkflowRunStatus::BudgetLimited
        );
        let resume = LaunchSpec {
            resume_run_id: Some(run_id.clone()),
            ..spec()
        };
        assert!(matches!(
            manager
                .launch(resolve_inline(script.into()).unwrap(), resume)
                .unwrap_err(),
            LaunchError::BudgetNotRaised { .. }
        ));
    }

    #[tokio::test]
    async fn control_run_refuses_to_pause_an_engine_paused_run() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let script = "let meta = #{ name: \"t\", description: \"d\" };\n\
                      let r = agent(\"work\");\ncomplete(r.output);";
        let (run_id, _outcome) = manager
            .launch(resolve_inline(script.into()).unwrap(), spec())
            .unwrap();
        let _child = recv_spawn(&mut subagent_rx).await;
        manager.tracker.lock().apply_outcome(
            &run_id,
            &WorkflowOutcome::Paused {
                kind: crate::PauseKind::BackOff,
                message: "backing off".into(),
            },
        );

        // The run is still in `active`, so a bare pause() would report success without changing anything.
        assert_eq!(
            manager
                .control_run(&run_id, WorkflowControl::Pause)
                .unwrap_err(),
            ControlError::NotApplicable {
                name: "t".to_owned(),
                status: WorkflowRunStatus::BackOffPaused,
                control: WorkflowControl::Pause,
            }
        );
        assert_eq!(
            manager.tracker.lock().get(&run_id).unwrap().status,
            WorkflowRunStatus::BackOffPaused
        );
    }

    #[tokio::test]
    async fn backgrounded_stub_fails_loudly() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        let resolved = resolve_inline(
            "let meta = #{ name: \"t\", description: \"d\" };\n\
             let r = agent(\"work\");\n\
             complete(r.output);"
                .into(),
        )
        .unwrap();
        let (run_id, outcome_rx) = manager.launch(resolved, spec()).unwrap();

        let spawn_req = subagent_rx.recv().await.expect("spawn event");
        let SubagentEvent::Spawn(req) = spawn_req else {
            panic!("expected spawn event");
        };
        let _ = req.result_tx.send(SubagentResult {
            backgrounded: true,

            ..crate::test_backend::empty_result()
        });
        let outcome = outcome_rx.await.unwrap();
        match outcome {
            WorkflowOutcome::Failed { error } => {
                assert!(
                    error.contains("auto-backgrounded"),
                    "distinct engine-bug message expected, got: {error}"
                );
            }
            other => panic!("expected Failed, got {other:?}"),
        }
        let state = manager.tracker.lock().get(&run_id).unwrap();
        assert_eq!(state.status, crate::tracker::WorkflowRunStatus::Failed);
    }

    #[tokio::test]
    async fn parallel_panel_respects_concurrency_cap() {
        const CAP: usize = 2;
        const N: usize = 6;

        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        manager.test_set_max_concurrent_agents(CAP);
        let (_run_id, outcome_rx) = manager
            .launch(resolve_inline(parallel_n_script(N)).unwrap(), spec())
            .unwrap();

        let mut live = Vec::new();
        for _ in 0..CAP {
            live.push(recv_spawn(&mut subagent_rx).await);
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(150), subagent_rx.recv())
                .await
                .is_err(),
            "more than {CAP} children were live"
        );

        let mut completed = 0usize;
        while completed + live.len() < N {
            complete_spawn(live.remove(0));
            completed += 1;
            live.push(recv_spawn(&mut subagent_rx).await);
        }
        for req in live {
            complete_spawn(req);
        }

        match outcome_rx.await.unwrap() {
            WorkflowOutcome::Completed { result } => assert_eq!(result, serde_json::json!(N)),
            other => panic!("expected Completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancel_drops_queued_spawns_before_coordinator() {
        let dir = tempfile::tempdir().unwrap();
        let (mut manager, mut subagent_rx) = test_manager(Some(dir.path().to_path_buf()));
        manager.test_set_max_concurrent_agents(1);
        let (run_id, outcome_rx) = manager
            .launch(resolve_inline(parallel_n_script(4)).unwrap(), spec())
            .unwrap();

        let first = recv_spawn(&mut subagent_rx).await;
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(150), subagent_rx.recv())
                .await
                .is_err(),
            "queued agents reached the coordinator before cancel"
        );

        assert!(manager.cancel(&run_id));
        let _ = outcome_rx.await;
        assert!(first.cancel_token.is_cancelled());
        assert!(subagent_rx.try_recv().is_err());
    }
}
