//! Canonical prompt provenance and visibility policy; no agent runtime.

pub use xai_agent_lifecycle::{
    AnalyticsClass, CompactionClass, InputAuthority, InputPolicy, QueuePolicy, ShutdownPolicy,
    SlashAuthority, TurnBoundary,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptOrigin {
    User,
    /// Auto-wake prompt injected when a background terminal task completed.
    TaskCompleted {
        /// The background task ID (without the `task-completed-` prefix).
        task_id: String,
    },
    /// Auto-wake prompt injected when a background subagent completed.
    SubagentCompleted {
        /// The subagent ID (without the `subagent-completed-` prefix).
        subagent_id: String,
    },
    /// Model-authored context from the owning root session.
    ParentAgentMessage {
        message_id: String,
        sender_session_id: String,
    },
    /// Human text from the owning parent. Slash-inert; `@file` stays closed.
    ParentHumanMessage {
        message_id: String,
        sender_session_id: String,
    },
    WorkflowCompleted {
        completion_id: String,
    },
    /// Server-initiated prompt from the idle-gated notification drain (`maybe_drain_notifications`).
    /// Batches one or more monitor-event or bash-task-completed notifications into a single turn while the user is idle.
    NotificationDrain,
    /// The goal orchestrator injects a system reminder into context and then triggers a model turn so the model can print a visible progress update.
    GoalSummary,
    /// Nudge injected when the verification stage rejects an `update_goal(completed: true)` attempt.
    /// Carries the "not yet achieved — keep working" system-reminder body alongside the path to the persisted details file.
    /// The variant name retains the `Classifier` prefix for wire stability.
    GoalClassifierNudge,
    /// Scheduled task (`/loop`) prompt fired by the scheduler via the pager.
    SchedulerFired,
    /// The shell re-parked `exit_plan_mode` on resume, the user approved/revised, and the shell injects the follow-up turn.
    /// Synthetic: the user never typed it, so it stays out of prompt history, but it still runs a real turn.
    PlanResume,
}
impl PromptOrigin {
    pub fn from_prompt_id(prompt_id: &str) -> Self {
        if let Some(task_id) = prompt_id.strip_prefix("task-completed-") {
            Self::TaskCompleted {
                task_id: task_id.to_string(),
            }
        } else if let Some(subagent_id) = prompt_id.strip_prefix("subagent-completed-") {
            Self::SubagentCompleted {
                subagent_id: subagent_id.to_string(),
            }
        } else if let Some(parent_message_id) = prompt_id.strip_prefix("parent-agent-message-") {
            Self::ParentAgentMessage {
                message_id: parent_message_id.to_string(),
                sender_session_id: String::new(),
            }
        } else if let Some(parent_message_id) = prompt_id.strip_prefix("parent-message-") {
            Self::ParentHumanMessage {
                message_id: parent_message_id.to_string(),
                sender_session_id: String::new(),
            }
        } else if let Some(completion_id) = prompt_id.strip_prefix("workflow-completed-") {
            Self::WorkflowCompleted {
                completion_id: completion_id.to_string(),
            }
        } else if prompt_id.starts_with("notifications-") {
            Self::NotificationDrain
        } else if prompt_id.starts_with("goal-summary-") {
            Self::GoalSummary
        } else if prompt_id.starts_with("goal-classifier-nudge-") {
            Self::GoalClassifierNudge
        } else if prompt_id.starts_with("scheduler-fired-") {
            Self::SchedulerFired
        } else if prompt_id.starts_with("plan-resume-") {
            Self::PlanResume
        } else {
            Self::User
        }
    }
    pub const fn policy(&self) -> InputPolicy {
        match self {
            Self::User => InputPolicy {
                authority: InputAuthority::HumanIntent,
                slash: SlashAuthority::HumanCatalog,
                turn_boundary: TurnBoundary::Conversational,
                analytics: AnalyticsClass::HumanPrompt,
                compaction: CompactionClass::HumanAnchor,
                queue: QueuePolicy::VisibleEditable,
                shutdown: ShutdownPolicy::Drain,
            },
            Self::ParentAgentMessage { .. } => InputPolicy {
                authority: InputAuthority::ModelAuthoredUntrusted,
                slash: SlashAuthority::ModelAuthored,
                turn_boundary: TurnBoundary::Conversational,
                analytics: AnalyticsClass::AgentMessage,
                compaction: CompactionClass::ConversationalAgentAnchor,
                queue: QueuePolicy::VisibleProtected,
                shutdown: ShutdownPolicy::Drain,
            },
            Self::ParentHumanMessage { .. } => InputPolicy {
                authority: InputAuthority::ModelAuthoredUntrusted,
                slash: SlashAuthority::Inert,
                turn_boundary: TurnBoundary::Conversational,
                analytics: AnalyticsClass::AgentMessage,
                compaction: CompactionClass::ConversationalAgentAnchor,
                queue: QueuePolicy::VisibleProtected,
                shutdown: ShutdownPolicy::Drain,
            },
            Self::TaskCompleted { .. }
            | Self::SubagentCompleted { .. }
            | Self::WorkflowCompleted { .. }
            | Self::SchedulerFired => InputPolicy {
                authority: InputAuthority::RuntimeControl,
                slash: SlashAuthority::Inert,
                turn_boundary: TurnBoundary::Conversational,
                analytics: AnalyticsClass::RuntimeWake,
                compaction: CompactionClass::RuntimeEphemera,
                queue: QueuePolicy::Hidden,
                shutdown: ShutdownPolicy::CancelWithProducer,
            },
            Self::NotificationDrain
            | Self::GoalSummary
            | Self::GoalClassifierNudge
            | Self::PlanResume => InputPolicy {
                authority: InputAuthority::RuntimeControl,
                slash: SlashAuthority::Inert,
                turn_boundary: TurnBoundary::Conversational,
                analytics: AnalyticsClass::RuntimeWake,
                compaction: CompactionClass::RuntimeEphemera,
                queue: QueuePolicy::Hidden,
                shutdown: ShutdownPolicy::DropEphemeral,
            },
        }
    }
    /// Returns `true` for auto-wake (synthetic) prompts.
    pub fn is_synthetic(&self) -> bool {
        !matches!(self, Self::User)
    }
    /// A queued user follow-up must wait for these to finish; Steer must not
    /// promote into them.
    pub fn is_auto_wake(&self) -> bool {
        matches!(
            self,
            Self::TaskCompleted { .. }
                | Self::SubagentCompleted { .. }
                | Self::WorkflowCompleted { .. }
                | Self::ParentAgentMessage { .. }
                | Self::ParentHumanMessage { .. }
                | Self::NotificationDrain
        )
    }
    /// Whether a `UserMessageChunk` echo for this origin must stay out of client scrollback (live and on resume).
    /// The hidden origins carry model-only, side-channel content the UI already shows elsewhere (task pane, monitor gutter, etc.).
    pub fn hide_user_echo_from_scrollback(&self) -> bool {
        match self {
            Self::User
            | Self::ParentAgentMessage { .. }
            | Self::ParentHumanMessage { .. }
            | Self::SchedulerFired
            | Self::PlanResume => false,
            Self::TaskCompleted { .. }
            | Self::SubagentCompleted { .. }
            | Self::WorkflowCompleted { .. }
            | Self::NotificationDrain
            | Self::GoalSummary
            | Self::GoalClassifierNudge => true,
        }
    }
    pub fn completion_id(&self) -> Option<&str> {
        match self {
            Self::TaskCompleted { task_id } => Some(task_id),
            Self::SubagentCompleted { subagent_id } => Some(subagent_id),
            Self::WorkflowCompleted { completion_id } => Some(completion_id),
            Self::User
            | Self::ParentAgentMessage { .. }
            | Self::ParentHumanMessage { .. }
            | Self::NotificationDrain
            | Self::GoalSummary
            | Self::GoalClassifierNudge
            | Self::SchedulerFired
            | Self::PlanResume => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{PromptOrigin, QueuePolicy};
    #[test]
    fn from_prompt_id_user() {
        assert_eq!(
            PromptOrigin::from_prompt_id("my-prompt"),
            PromptOrigin::User
        );
        assert!(!PromptOrigin::from_prompt_id("my-prompt").is_synthetic());
    }
    #[test]
    fn from_prompt_id_task_completed() {
        let origin = PromptOrigin::from_prompt_id("task-completed-abc-123");
        assert_eq!(
            origin,
            PromptOrigin::TaskCompleted {
                task_id: "abc-123".into()
            }
        );
        assert!(origin.is_synthetic());
        assert!(origin.is_auto_wake());
        assert_eq!(origin.completion_id(), Some("abc-123"));
        assert!(!PromptOrigin::from_prompt_id("my-prompt").is_auto_wake());
    }
    #[test]
    fn from_prompt_id_parent_message() {
        let origin = PromptOrigin::from_prompt_id("parent-message-msg-123");
        assert_eq!(
            origin,
            PromptOrigin::ParentHumanMessage {
                message_id: "msg-123".into(),
                sender_session_id: String::new(),
            }
        );
        assert_eq!(origin.policy().slash, super::SlashAuthority::Inert);
        assert!(origin.is_synthetic());
    }
    #[test]
    fn wake_compact_slash_preserves_parent_authority() {
        for (prompt_id, expected, slash) in [
            (
                "parent-agent-message-wake",
                PromptOrigin::ParentAgentMessage {
                    message_id: "wake".into(),
                    sender_session_id: String::new(),
                },
                super::SlashAuthority::ModelAuthored,
            ),
            (
                "parent-message-wake",
                PromptOrigin::ParentHumanMessage {
                    message_id: "wake".into(),
                    sender_session_id: String::new(),
                },
                super::SlashAuthority::Inert,
            ),
        ] {
            let origin = PromptOrigin::from_prompt_id(prompt_id);
            assert_eq!(origin, expected);
            assert_ne!(
                origin.policy().authority,
                super::InputAuthority::HumanIntent,
                "/compact must not enter the human command path",
            );
            assert_eq!(origin.policy().slash, slash);
        }
    }
    #[test]
    fn from_prompt_id_subagent_completed() {
        let origin = PromptOrigin::from_prompt_id("subagent-completed-xyz-789");
        assert_eq!(
            origin,
            PromptOrigin::SubagentCompleted {
                subagent_id: "xyz-789".into()
            }
        );
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), Some("xyz-789"));
    }
    #[test]
    fn from_prompt_id_notification_drain() {
        let origin =
            PromptOrigin::from_prompt_id("notifications-019e0000-0000-7000-8000-0000000000aa");
        assert_eq!(origin, PromptOrigin::NotificationDrain);
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn goal_summary_origin_from_prompt_id() {
        let origin = PromptOrigin::from_prompt_id("goal-summary-019e2d3e");
        assert!(matches!(origin, PromptOrigin::GoalSummary));
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn goal_classifier_nudge_origin_from_prompt_id() {
        let origin = PromptOrigin::from_prompt_id("goal-classifier-nudge-019e2d3e");
        assert!(matches!(origin, PromptOrigin::GoalClassifierNudge));
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn goal_classifier_nudge_origin_round_trips_through_from_prompt_id() {
        let prompt_id = format!("goal-classifier-nudge-{}", uuid::Uuid::now_v7());
        let origin = PromptOrigin::from_prompt_id(&prompt_id);
        assert!(matches!(origin, PromptOrigin::GoalClassifierNudge));
        assert!(origin.is_synthetic());
    }
    #[test]
    fn scheduler_fired_origin_from_prompt_id() {
        let origin = PromptOrigin::from_prompt_id("scheduler-fired-019e51a3-abcd-1234");
        assert!(matches!(origin, PromptOrigin::SchedulerFired));
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn plan_resume_origin_from_prompt_id() {
        let origin = PromptOrigin::from_prompt_id("plan-resume-1730000000000");
        assert!(matches!(origin, PromptOrigin::PlanResume));
        assert!(origin.is_synthetic());
        assert_eq!(origin.completion_id(), None);
    }
    #[test]
    fn notification_drain_is_server_initiated() {
        let prompt_id = "notifications-019e0000-0000-7000-8000-0000000000aa";
        assert!(PromptOrigin::from_prompt_id(prompt_id).is_synthetic());
    }
    #[test]
    fn current_origin_queue_policies_are_preserved() {
        let cases = [
            (PromptOrigin::User, QueuePolicy::VisibleEditable),
            (
                PromptOrigin::TaskCompleted {
                    task_id: "t".into(),
                },
                QueuePolicy::Hidden,
            ),
            (
                PromptOrigin::SubagentCompleted {
                    subagent_id: "s".into(),
                },
                QueuePolicy::Hidden,
            ),
            (
                PromptOrigin::ParentAgentMessage {
                    message_id: "m".into(),
                    sender_session_id: "root".into(),
                },
                QueuePolicy::VisibleProtected,
            ),
            (
                PromptOrigin::ParentHumanMessage {
                    message_id: "h".into(),
                    sender_session_id: "root".into(),
                },
                QueuePolicy::VisibleProtected,
            ),
            (
                PromptOrigin::WorkflowCompleted {
                    completion_id: "w".into(),
                },
                QueuePolicy::Hidden,
            ),
            (PromptOrigin::NotificationDrain, QueuePolicy::Hidden),
            (PromptOrigin::GoalSummary, QueuePolicy::Hidden),
            (PromptOrigin::GoalClassifierNudge, QueuePolicy::Hidden),
            (PromptOrigin::SchedulerFired, QueuePolicy::Hidden),
            (PromptOrigin::PlanResume, QueuePolicy::Hidden),
        ];
        for (origin, queue) in cases {
            assert_eq!(origin.policy().queue, queue, "{origin:?}");
        }
    }
    #[test]
    fn hide_user_echo_from_scrollback_by_origin() {
        assert!(!PromptOrigin::User.hide_user_echo_from_scrollback());
        assert!(
            !PromptOrigin::ParentAgentMessage {
                message_id: "m".into(),
                sender_session_id: "root".into(),
            }
            .hide_user_echo_from_scrollback()
        );
        assert!(
            !PromptOrigin::ParentHumanMessage {
                message_id: "h".into(),
                sender_session_id: "root".into(),
            }
            .hide_user_echo_from_scrollback()
        );
        assert!(
            !PromptOrigin::from_prompt_id("scheduler-fired-abc").hide_user_echo_from_scrollback()
        );
        assert!(!PromptOrigin::from_prompt_id("plan-resume-1").hide_user_echo_from_scrollback());
        assert!(PromptOrigin::from_prompt_id("task-completed-t1").hide_user_echo_from_scrollback());
        assert!(
            PromptOrigin::from_prompt_id("subagent-completed-s1").hide_user_echo_from_scrollback()
        );
        assert!(
            PromptOrigin::from_prompt_id("notifications-uuid").hide_user_echo_from_scrollback()
        );
        assert!(
            PromptOrigin::from_prompt_id("workflow-completed-wf-1-9")
                .hide_user_echo_from_scrollback()
        );
        assert!(PromptOrigin::from_prompt_id("goal-summary-1").hide_user_echo_from_scrollback());
        assert!(
            PromptOrigin::from_prompt_id("goal-classifier-nudge-1")
                .hide_user_echo_from_scrollback()
        );
    }
}
