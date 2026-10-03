//! Original ACP model-switch error contract, shared with the native consumer.
use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

pub const MODEL_SWITCH_INCOMPATIBLE_AGENT: &str = "MODEL_SWITCH_INCOMPATIBLE_AGENT";
/// Error code for model switch failure during the zero-turn full harness rebuild path.
/// Emitted when `RebuildAgentForDefinition` fails.
/// That covers a definition that could not be resolved at handler time, an `AgentBuilder::build()` error, or a turn racing the rebuild.
pub const MODEL_SWITCH_REBUILD_FAILED: &str = "MODEL_SWITCH_REBUILD_FAILED";
/// Structured error payload for model switch rejection due to agent type incompatibility.
/// Serialized into `acp::Error.data` by the shell and deserialized by the TUI for user-friendly error rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelSwitchIncompatibleAgentError {
    /// Stable machine-readable error code (always `MODEL_SWITCH_INCOMPATIBLE_AGENT`).
    pub code: String,
    pub active_agent_type: String,
    /// The agent type required by the target model.
    pub required_agent_type: String,
    pub model_id: String,
    /// Remediation hint for the client.
    pub suggestion: String,
}
impl ModelSwitchIncompatibleAgentError {
    /// Build an `acp::Error` with this structured payload.
    pub fn into_acp_error(self) -> acp::Error {
        let message = format!(
            "Cannot switch to model '{}': it requires agent '{}' but the active agent is '{}'. \
             Start a new session to use this model.",
            self.model_id, self.required_agent_type, self.active_agent_type,
        );
        acp::Error::new(acp::ErrorCode::InvalidRequest.into(), message)
            .data(serde_json::to_value(&self).ok())
    }
    /// Try to parse from an `acp::Error.data` field.
    pub fn from_acp_error(err: &acp::Error) -> Option<Self> {
        let data = err.data.as_ref()?;
        let code = data.get("code")?.as_str()?;
        if code != MODEL_SWITCH_INCOMPATIBLE_AGENT {
            return None;
        }
        serde_json::from_value(data.clone()).ok()
    }
    /// Render a user-friendly error message for the TUI.
    pub fn user_message(&self) -> String {
        format!(
            "Cannot switch to '{}' — it requires agent '{}' but the active agent is '{}'. \
             Start /new to use this model.",
            self.model_id, self.required_agent_type, self.active_agent_type,
        )
    }
}
