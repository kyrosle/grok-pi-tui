//! Actual wire values shared by native presentations and tool producers.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SkillOutput {
    /// Whether the skill was successfully resolved
    pub success: bool,
    /// Brief fallback message, used as the tool result when there is no skill body.
    pub tool_result: String,
    /// The skill's display name
    pub skill_name: String,
    /// The formatted skill content, delivered to the model as the tool result.
    pub skill_message: Option<String>,
    /// Error message if the skill failed to load
    pub error: Option<String>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExtractedImage {
    pub data: String,
    pub mime_type: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MonitorOutput {
    /// ID of the background monitor task (used with kill_command_or_subagent to cancel).
    pub task_id: String,
    /// Timeout deadline in milliseconds. 0 when persistent.
    pub timeout_ms: u64,
    /// Whether the monitor runs until kill_command_or_subagent or session end.
    pub persistent: bool,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerCreateOutput {
    pub id: String,
    pub human_schedule: String,
    #[serde(default)]
    pub updated: bool,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SchedulerDeleteOutput {
    pub success: bool,
    pub message: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskSummary {
    pub id: String,
    pub prompt: String,
    pub interval_human: String,
    pub next_fire_at: String,
    pub created_at: String,
    pub recurring: bool,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SchedulerListOutput {
    pub tasks: Vec<ScheduledTaskSummary>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateGoalOutput {
    pub success: bool,
    pub summary: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct WorkflowToolOutput {
    pub run_id: String,
    #[schemars(
        description = "Alias of run_id; workflow runs are not background tasks — do not pass to task_output/wait_tasks. Completion notifies automatically."
    )]
    pub task_id: String,
    #[schemars(
        description = "The session-unique display handle for this run, such as review-changes or review-changes-2. Use it in user-facing status and /workflow management; keep run_id internal."
    )]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_path: Option<String>,
    pub message: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SendSubagentMessageInput {
    /// ID of the owned subagent that should receive the message.
    pub subagent_id: String,
    /// Text to send to the subagent.
    pub text: String,
    /// Queue for a later turn instead of steering the active turn.
    #[serde(default)]
    #[schemars(default)]
    pub queue: bool,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[non_exhaustive]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SendSubagentMessageOutput {
    Accepted {
        message_id: String,
    },
    NotFoundOrNotOwned,
    NotActiveOrFinalizing,
    Saturated {
        max_in_flight: usize,
    },
    AdmissionUncertain,
    NotAcceptedBeforeDeadline,
    Unsupported,
    Limit {
        max_bytes: usize,
        observed_bytes: usize,
    },
    ChannelClosed,
}

/// Delivery classification shared by tool hosts and presentations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendSubagentMessageDisposition {
    /// Admission was confirmed.
    Accepted,
    /// Admission was definitely rejected.
    Rejected,
    /// Admission or delivery could not be confirmed.
    Unconfirmed,
}

impl SendSubagentMessageOutput {
    /// Classify this output without collapsing uncertainty into failure.
    pub fn disposition(&self) -> SendSubagentMessageDisposition {
        match self {
            Self::Accepted { .. } => SendSubagentMessageDisposition::Accepted,
            Self::AdmissionUncertain => SendSubagentMessageDisposition::Unconfirmed,
            Self::NotFoundOrNotOwned
            | Self::NotActiveOrFinalizing
            | Self::Saturated { .. }
            | Self::NotAcceptedBeforeDeadline
            | Self::Unsupported
            | Self::Limit { .. }
            | Self::ChannelClosed => SendSubagentMessageDisposition::Rejected,
        }
    }
}

impl std::fmt::Display for SendSubagentMessageOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accepted { message_id } => {
                write!(f, "Message accepted (message_id: {message_id}).")
            }
            Self::NotFoundOrNotOwned => {
                f.write_str("Subagent not found or not owned by this session.")
            }
            Self::NotActiveOrFinalizing => f.write_str("Subagent is not active or is finalizing."),
            Self::Saturated { max_in_flight } => write!(
                f,
                "Message admission is saturated (maximum {max_in_flight} in flight)."
            ),
            Self::AdmissionUncertain => f.write_str(
                "Message admission could not be confirmed; the message may or may not have been accepted.",
            ),
            Self::NotAcceptedBeforeDeadline => {
                f.write_str("Message was not accepted before the delivery deadline.")
            }
            Self::Unsupported => {
                f.write_str("Active agent messages are unsupported in this context.")
            }
            Self::Limit {
                max_bytes,
                observed_bytes,
            } => write!(
                f,
                "Message size is invalid: observed {observed_bytes} bytes; maximum is {max_bytes} bytes."
            ),
            Self::ChannelClosed => {
                f.write_str("Message was not accepted because the subagent channel closed.")
            }
        }
    }
}

pub use crate::tool_names::SEND_SUBAGENT_MESSAGE_TOOL_NAME;

/// Product default advertised in the model-facing schema (FG). Not applied as a
/// serde default: omit/`None` must remain "use host/FG policy, BG unbounded".
fn schema_default_timeout_ms() -> Option<u64> {
    Some(120_000)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BashToolInput {
    #[cfg_attr(unix, schemars(description = "The bash command to run."))]
    #[cfg_attr(not(unix), schemars(description = "The command to run."))]
    pub command: String,

    /// Optional timeout in milliseconds (max 300000). Default: 120000 (2 minutes), enforced for
    /// foreground commands only. Background semantics live in the tool-description usage notes.
    /// keep in sync with the rustdoc above
    #[schemars(
        description = "Optional timeout in milliseconds (max 300000). Default: 120000 (2 minutes), enforced for foreground commands only.",
        default = "schema_default_timeout_ms"
    )]
    // Some models serialize numeric tool args as JSON strings (`"120000"`), which a plain `Option<u64>` rejects. Accept
    // string-or-number here; the schema still advertises an integer. Serde default stays None so omit ≠ Some(120000):
    // background omit must stay unbounded (see resolve_effective_timeout). Schema still advertises 120000.
    #[serde(
        default,
        deserialize_with = "crate::argument_schema::deserialize_lenient_u64",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout: Option<u64>,

    /// One sentence explanation as to why this command needs to be run and how it contributes to the goal.
    #[schemars(
        description = "One sentence explanation as to why this command needs to be run and how it contributes to the goal."
    )]
    pub description: String,

    /// Set to true for long-running commands that should run in the background (e.g., dev servers, long builds). Returns a task id immediately
    /// while the command keeps running in the background; you are notified on completion, so do not poll or sleep-wait for it. "task id" stays
    /// plain English: the kill/get-output input params are renameable, so naming a literal key here goes stale after randomization.
    #[schemars(
        description = "Set to true for long-running commands that should run in the background (e.g., dev servers, long builds). Returns a task id immediately while the command keeps running in the background${%- if system_reminders_enabled %}; you are notified on completion, so do not poll or sleep-wait for it${%- elif tools.by_kind.background_task_action %}; check on it later with the ${{ tools.by_kind.background_task_action }} tool${%- endif %}."
    )]
    #[serde(
        default,
        deserialize_with = "crate::argument_schema::deserialize_lenient_bool"
    )]
    pub is_background: bool,
}

#[cfg(test)]
mod tests {
    use super::BashToolInput;
    use serde_json::json;

    #[test]
    fn bash_timeout_preserves_original_numeric_boundaries() {
        let parse = |timeout| {
            serde_json::from_value::<BashToolInput>(json!({
                "command": "true",
                "description": "test",
                "timeout": timeout,
            }))
        };
        for timeout in [json!(120000), json!(120000.0), json!("120000")] {
            assert_eq!(parse(timeout).unwrap().timeout, Some(120000));
        }
        assert_eq!(parse(json!(u64::MAX)).unwrap().timeout, Some(u64::MAX));
        assert_eq!(
            parse(json!("9007199254740992")).unwrap().timeout,
            Some(9_007_199_254_740_992),
        );
        for timeout in [
            json!("18446744073709551615"),
            json!("9007199254740994"),
            json!(" 120000 "),
            json!(0.5),
            json!(-1),
        ] {
            assert!(parse(timeout).is_err());
        }
        assert_eq!(parse(json!(null)).unwrap().timeout, None);
    }

    #[test]
    fn bash_schema_default_keeps_omitted_background_timeout_unbounded() {
        let input: BashToolInput = serde_json::from_value(json!({
            "command": "true",
            "description": "test",
            "is_background": true,
        }))
        .unwrap();
        assert_eq!(input.timeout, None);
        let schema = serde_json::to_value(schemars::schema_for!(BashToolInput)).unwrap();
        assert_eq!(schema["properties"]["timeout"]["default"], json!(120000));
    }
}
