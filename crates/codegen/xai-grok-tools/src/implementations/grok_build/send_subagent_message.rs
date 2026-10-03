//! `send_subagent_message` — send an active message to an owned subagent.

use crate::implementations::grok_build::task::backend::SubagentBackendResource;
use crate::implementations::grok_build::task::types::{
    ActiveAgentMessageOperation, ActiveAgentMessageOutcome, ActiveAgentMessageRequest,
    SubagentDepthCounter,
};
use crate::types::tool::{ToolKind, ToolNamespace};

pub use xai_tool_types::tool_names::SEND_SUBAGENT_MESSAGE_TOOL_NAME;

pub use xai_tool_types::output_dependencies::{
    SendSubagentMessageDisposition, SendSubagentMessageInput, SendSubagentMessageOutput,
};

impl From<ActiveAgentMessageOutcome> for SendSubagentMessageOutput {
    fn from(outcome: ActiveAgentMessageOutcome) -> Self {
        match outcome {
            ActiveAgentMessageOutcome::Accepted { message_id } => Self::Accepted { message_id },
            ActiveAgentMessageOutcome::NotFoundOrNotOwned => Self::NotFoundOrNotOwned,
            ActiveAgentMessageOutcome::NotActiveOrFinalizing => Self::NotActiveOrFinalizing,
            ActiveAgentMessageOutcome::Saturated { max_in_flight } => {
                Self::Saturated { max_in_flight }
            }
            ActiveAgentMessageOutcome::AdmissionUncertain => Self::AdmissionUncertain,
            ActiveAgentMessageOutcome::NotAcceptedBeforeDeadline => Self::NotAcceptedBeforeDeadline,
            ActiveAgentMessageOutcome::Unsupported => Self::Unsupported,
            ActiveAgentMessageOutcome::Limit {
                max_bytes,
                observed_bytes,
            } => Self::Limit {
                max_bytes,
                observed_bytes,
            },
            ActiveAgentMessageOutcome::ChannelClosed => Self::ChannelClosed,
        }
    }
}

#[derive(Debug, Default)]
pub struct SendSubagentMessageTool;

impl crate::types::tool_metadata::ToolMetadata for SendSubagentMessageTool {
    fn kind(&self) -> ToolKind {
        ToolKind::ActiveAgentMessage
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Send a follow-up message to a subagent owned by this session. An inactive subagent resumes with the same identity and receives the message as its next turn. For an active subagent, the default steers the current turn at its next safe point; set queue to true to wait for a later turn."
    }
}

impl xai_tool_runtime::Tool for SendSubagentMessageTool {
    type Args = SendSubagentMessageInput;
    type Output = SendSubagentMessageOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(SEND_SUBAGENT_MESSAGE_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            SEND_SUBAGENT_MESSAGE_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::sanitized_description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(
        name = "tool.send_subagent_message",
        skip_all,
        fields(subagent_id = %input.subagent_id)
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: SendSubagentMessageInput,
    ) -> Result<SendSubagentMessageOutput, xai_tool_runtime::ToolError> {
        let resources = crate::types::tool_metadata::shared_resources(&ctx)?;
        let (depth, backend) = {
            let res = resources.lock().await;
            (
                res.get::<SubagentDepthCounter>().map(|value| value.0),
                res.get::<SubagentBackendResource>().cloned(),
            )
        };

        let (Some(0), Some(backend)) = (depth, backend) else {
            return Ok(SendSubagentMessageOutput::Unsupported);
        };
        let operation = if input.queue {
            ActiveAgentMessageOperation::Queue
        } else {
            ActiveAgentMessageOperation::Steer
        };
        let request = match ActiveAgentMessageRequest::try_new_with_operation(
            input.subagent_id,
            input.text,
            operation,
        ) {
            Ok(request) => request,
            Err(outcome) => return Ok(outcome.into()),
        };

        Ok(backend.backend().send_active_message(request).await.into())
    }
}

#[cfg(test)]
#[path = "send_subagent_message_tests.rs"]
mod tests;
