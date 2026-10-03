use super::store::WorkflowRunStore;
use super::tracker::WorkflowRunState;
use crate::extensions::notification::{
    SessionNotification as XaiSessionNotification, SessionUpdate as XaiSessionUpdate,
};
use crate::session::persistence::PersistenceMsg;

/// Compatibility constructor for the stock ACP session actor.
#[derive(Clone)]
pub struct WorkflowNotifySender(xai_workflow::notify::WorkflowNotifySender);
impl std::ops::Deref for WorkflowNotifySender {
    type Target = xai_workflow::notify::WorkflowNotifySender;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<WorkflowNotifySender> for xai_workflow::notify::WorkflowNotifySender {
    fn from(sender: WorkflowNotifySender) -> Self {
        sender.0
    }
}
impl WorkflowNotifySender {
    pub fn new(
        session_id: agent_client_protocol::SessionId,
        gateway: xai_acp_lib::AcpAgentGatewaySender,
        persistence_tx: tokio::sync::mpsc::UnboundedSender<PersistenceMsg>,
        store: WorkflowRunStore,
    ) -> Self {
        Self(xai_workflow::notify::WorkflowNotifySender::new(
            store,
            std::sync::Arc::new(move |update, persist| {
                let mut meta = None;
                crate::util::event_id::ensure_event_id_meta(&session_id.0, &mut meta);
                let notification = XaiSessionNotification {
                    session_id: session_id.clone(),
                    update: update.into(),
                    meta: meta.map(serde_json::Value::Object),
                };
                let raw = serde_json::to_value(&notification)
                    .and_then(|v| serde_json::value::to_raw_value(&v))
                    .ok();
                if persist {
                    let _ = persistence_tx.send(PersistenceMsg::Update(
                        crate::session::storage::SessionUpdate::Xai(Box::new(notification)),
                    ));
                }
                if let Some(raw) = raw {
                    gateway.forward_fire_and_forget(agent_client_protocol::ExtNotification::new(
                        "x.ai/session_notification",
                        raw.into(),
                    ));
                }
            }),
        ))
    }
}

pub fn build_workflow_updated(
    state: &WorkflowRunState,
    elapsed_ms: u64,
    active_agents: u32,
) -> XaiSessionUpdate {
    xai_workflow::notify::build_workflow_updated(state, elapsed_ms, active_agents).into()
}
pub use xai_workflow::notify::workflow_session_notification_json;

#[cfg(test)]
mod tests {
    #[test]
    fn neutral_payload_matches_stock_wire() {
        let state = super::super::tracker::WorkflowTracker::default().start_run(
            "wf_wire".into(),
            "demo".into(),
            "objective".into(),
            Vec::new(),
            None,
            None,
        );
        let neutral = xai_workflow::notify::build_workflow_updated(&state, 25, 0);
        let stock: crate::extensions::notification::SessionUpdate = neutral.clone().into();
        assert_eq!(
            serde_json::to_value(neutral).unwrap(),
            serde_json::to_value(stock).unwrap()
        );
        let envelope = super::workflow_session_notification_json("session", &state, 25);
        assert_eq!(envelope["sessionId"], "session");
        assert_eq!(envelope["update"]["sessionUpdate"], "workflow_updated");
    }
}
