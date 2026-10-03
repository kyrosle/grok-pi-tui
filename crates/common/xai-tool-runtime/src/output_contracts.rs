//! Existing tool execution-frame conversions over canonical wire data.
use xai_tool_types::output::*;
use xai_tool_types::output_dependencies::*;

/// Where the client's diff lands in the file. `None` for creations (no
/// line to point at) and when `replace_all` touched several matches,
/// since the card renders one snippet.
fn edit_file_anchor(edit_result: &SearchReplaceEditsApplied) -> Option<crate::EditFileAnchor> {
    if edit_result.old_string.is_empty() {
        return None;
    }
    let [edit] = edit_result.edits.details.as_slice() else {
        return None;
    };
    let start_line = u32::try_from(edit.old_line).ok()?;
    Some(crate::EditFileAnchor { start_line })
}

impl crate::ToolOutput for BashToolOutput {
    fn chat_completion_output(&self) -> Option<crate::ToolChatCompletionResponse> {
        match self {
            Self::Bash(bash) => crate::ToolOutput::chat_completion_output(bash),
            Self::BackgroundTaskStarted(_) => None,
        }
    }
}
impl crate::ToolOutput for ToolOutput {
    fn chat_completion_output(&self) -> Option<crate::ToolChatCompletionResponse> {
        match self {
            Self::Bash(bash) => crate::ToolOutput::chat_completion_output(bash),
            Self::SearchReplace(edit) => crate::ToolOutput::chat_completion_output(edit),
            _ => None,
        }
    }
}
impl crate::ToolOutput for SearchReplaceEditsApplied {
    /// Same frame grok-computer's `FileEditTool` sends: an empty, successful
    /// `code_execution_result` shell is what settles the edit card on every
    /// client, and the anchor rides along when there is one. The reducer
    /// projects the anchor ahead of the shell only where clients accept it.
    fn chat_completion_output(&self) -> Option<crate::ToolChatCompletionResponse> {
        Some(crate::ToolChatCompletionResponse {
            result: Some(crate::ToolChatCompletion {
                sender: "assistant".into(),
                message_tag: Some("raw_function_result".into()),
                code_execution_result: Some(crate::ToolCodeExecutionResult::default()),
                edit_file_result: edit_file_anchor(self),
                ..Default::default()
            }),
            ..Default::default()
        })
    }
}
impl crate::ToolOutput for BashOutput {
    fn chat_completion_output(&self) -> Option<crate::ToolChatCompletionResponse> {
        let mut stdout = String::from_utf8_lossy(&self.output).into_owned();
        let mut extra = serde_json::Map::new();
        if self.truncated {
            let shown = xai_tool_types::presentation::format_bytes(self.output.len() as u64);
            let total = xai_tool_types::presentation::format_bytes(self.total_bytes as u64);
            stdout.push_str(&format!(
                "\n[truncated: showing first/last {shown} of {total} - full output at: {}]",
                self.output_file
            ));
            extra.insert("truncated".into(), serde_json::Value::Bool(true));
            extra.insert(
                "total_bytes".into(),
                serde_json::Value::from(self.total_bytes as u64),
            );
            if !self.output_file.is_empty() {
                extra.insert(
                    "output_file".into(),
                    serde_json::Value::String(self.output_file.clone()),
                );
            }
        }
        Some(crate::ToolChatCompletionResponse {
            result: Some(crate::ToolChatCompletion {
                sender: "assistant".into(),
                message_tag: Some("raw_function_result".into()),
                code_execution_result: Some(crate::ToolCodeExecutionResult {
                    stdout,
                    stderr: String::new(),
                    exit_code: self.exit_code,
                    command_timed_out: self.timed_out,
                }),
                extra,
                ..Default::default()
            }),
            ..Default::default()
        })
    }
}
impl crate::ToolOutput for GrepSearchOutput {}
impl crate::ToolOutput for ReadFileOutput {}
impl crate::ToolOutput for ListDirOutput {}
impl crate::ToolOutput for SearchReplaceOutput {
    fn chat_completion_output(&self) -> Option<crate::ToolChatCompletionResponse> {
        match self {
            Self::EditsApplied(applied) => crate::ToolOutput::chat_completion_output(applied),
            _ => None,
        }
    }
}
impl crate::ToolOutput for TodoWriteOutput {}
impl crate::ToolOutput for WebSearchOutput {}
impl crate::ToolOutput for WebFetchOutput {}
impl crate::ToolOutput for SkillOutput {}
impl crate::ToolOutput for ApplyPatchOutput {}
impl crate::ToolOutput for CodexGrepFilesOutput {}
impl crate::ToolOutput for SearchToolOutput {}
impl crate::ToolOutput for EnterPlanModeOutput {}
impl crate::ToolOutput for ExitPlanModeOutput {}
impl crate::ToolOutput for AskUserQuestionOutput {}
impl crate::ToolOutput for MCPOutput {}

impl crate::ToolOutput for SendSubagentMessageOutput {}
impl crate::ToolOutput for MonitorOutput {}
impl crate::ToolOutput for SchedulerCreateOutput {}
impl crate::ToolOutput for SchedulerDeleteOutput {}
impl crate::ToolOutput for SchedulerListOutput {}
impl crate::ToolOutput for UpdateGoalOutput {}
impl crate::ToolOutput for WorkflowToolOutput {}
