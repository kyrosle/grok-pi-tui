use crate::store::WorkflowRunStore;
use crate::tracker::WorkflowRunState;
use std::sync::Arc;
use xai_tool_types::workflow::{WorkflowAgentInfo, WorkflowPhaseInfo, WorkflowUpdate};

/// Native projection callback: stock persists ACP records; Pi publishes from its own transport.
#[derive(Clone)]
pub struct WorkflowNotifySender {
    store: WorkflowRunStore,
    publish: Arc<dyn Fn(WorkflowUpdate, bool) + Send + Sync>,
}
impl WorkflowNotifySender {
    pub fn new(
        store: WorkflowRunStore,
        publish: Arc<dyn Fn(WorkflowUpdate, bool) + Send + Sync>,
    ) -> Self {
        Self { store, publish }
    }
    pub fn emit(&self, state: &WorkflowRunState, elapsed_ms: u64, active_agents: u32) {
        self.persist(state);
        self.broadcast(state, elapsed_ms, active_agents, true);
    }
    pub fn emit_ephemeral(&self, state: &WorkflowRunState, elapsed_ms: u64, active_agents: u32) {
        self.persist(state);
        self.broadcast(state, elapsed_ms, active_agents, false);
    }
    fn persist(&self, state: &WorkflowRunState) {
        if let Err(error) = self.store.persist(state) {
            tracing::warn!(run_id = %state.run_id, %error, "failed to queue workflow manifest");
        }
    }
    pub fn broadcast(
        &self,
        state: &WorkflowRunState,
        elapsed_ms: u64,
        active_agents: u32,
        persist_update: bool,
    ) {
        (self.publish)(
            build_workflow_updated(state, elapsed_ms, active_agents),
            persist_update,
        );
    }
}

pub fn build_workflow_updated(
    state: &WorkflowRunState,
    elapsed_ms: u64,
    _active_agents: u32,
) -> WorkflowUpdate {
    let last = state.history.last();

    let run_complete = state.status == super::tracker::WorkflowRunStatus::Complete;
    let current_idx = state
        .current_phase
        .as_deref()
        .and_then(|cur| state.phases.iter().position(|p| p.title == cur));
    let phases = state
        .phases
        .iter()
        .enumerate()
        .map(|(idx, p)| WorkflowPhaseInfo {
            title: p.title.clone(),
            state: match current_idx {
                Some(cur) if idx < cur => "done",
                Some(cur) if idx == cur && run_complete => "done",
                Some(cur) if idx == cur => "active",
                _ => "pending",
            }
            .to_string(),
        })
        .collect();

    let agents: Vec<WorkflowAgentInfo> = state
        .agents
        .iter()
        .map(|a| WorkflowAgentInfo {
            agent_id: a.agent_id.clone(),
            label: a.label.clone(),
            phase: a.phase.clone(),
            model: a.model.clone(),
            state: a.state.clone(),
            tokens_used: a.tokens_used,
            duration_ms: a.duration_ms,
        })
        .collect();
    let current_agent_label = agents
        .iter()
        .rev()
        .find(|a| a.state == "running")
        .map(|a| a.label.clone());

    let active_agents = agents
        .iter()
        .filter(|agent| agent.state == "running")
        .count() as u32;

    let agents_reserved = 0u64;
    let agents_remaining = state
        .agent_budget
        .map(|total| total.saturating_sub(state.agents_used.saturating_add(agents_reserved)));

    WorkflowUpdate {
        run_id: state.run_id.clone(),
        revision: state.revision,
        name: state.name.clone(),
        objective: state.objective.clone(),
        status: state.status.as_ref().to_string(),
        foreground: false,
        phases,
        current_phase: state.current_phase.clone(),
        agent_budget: state.agent_budget,
        agents_used: state.agents_used,
        agents_reserved,
        agents_remaining,
        agent_usage_incomplete: state.agent_usage_incomplete,
        elapsed_ms,
        active_agents,
        current_agent_label,
        agents,
        last_event: last.map(|e| e.event.clone()),
        last_event_detail: last.and_then(|e| e.detail.clone()),
        last_event_timestamp: last.map(|e| e.at.clone()),
        pause_message: state.pause_message.clone(),
        result_summary: state.result_summary.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::WorkflowTracker;

    #[test]
    fn phase_states_derive_from_current() {
        let mut t = WorkflowTracker::default();
        t.start_run(
            "wf_1".into(),
            "demo".into(),
            "obj".into(),
            vec![
                crate::PhaseMeta {
                    title: "Plan".into(),
                    detail: None,
                },
                crate::PhaseMeta {
                    title: "Execute".into(),
                    detail: None,
                },
                crate::PhaseMeta {
                    title: "Verify".into(),
                    detail: None,
                },
            ],
            None,
            None,
        );
        t.set_phase("wf_1", "Execute");
        let state = t.get("wf_1").unwrap();
        match build_workflow_updated(&state, 0, 0) {
            WorkflowUpdate { phases, .. } => {
                let states: Vec<(String, String)> =
                    phases.into_iter().map(|p| (p.title, p.state)).collect();
                assert_eq!(
                    states,
                    vec![
                        ("Plan".to_string(), "done".to_string()),
                        ("Execute".to_string(), "active".to_string()),
                        ("Verify".to_string(), "pending".to_string()),
                    ]
                );
            }
        }
    }

    #[test]
    fn completed_run_marks_current_phase_done() {
        let mut t = WorkflowTracker::default();
        t.start_run(
            "wf_2".into(),
            "demo".into(),
            "obj".into(),
            vec![
                crate::PhaseMeta {
                    title: "Plan".into(),
                    detail: None,
                },
                crate::PhaseMeta {
                    title: "Verify".into(),
                    detail: None,
                },
            ],
            None,
            None,
        );
        t.set_phase("wf_2", "Verify");
        t.apply_outcome(
            "wf_2",
            &crate::WorkflowOutcome::Completed {
                result: serde_json::json!("done"),
            },
        );
        let state = t.get("wf_2").unwrap();
        match build_workflow_updated(&state, 0, 0) {
            WorkflowUpdate { phases, status, .. } => {
                assert_eq!(status, "complete");
                let states: Vec<(String, String)> =
                    phases.into_iter().map(|p| (p.title, p.state)).collect();
                assert_eq!(
                    states,
                    vec![
                        ("Plan".to_string(), "done".to_string()),
                        ("Verify".to_string(), "done".to_string()),
                    ]
                );
            }
        }
    }
}

/// Serialize the existing x.ai notification envelope for external hosts.
pub fn workflow_session_notification_json(
    session_id: &str,
    state: &WorkflowRunState,
    elapsed_ms: u64,
) -> serde_json::Value {
    serde_json::json!({"sessionId": session_id, "update": build_workflow_updated(state, elapsed_ms, 0)})
}
