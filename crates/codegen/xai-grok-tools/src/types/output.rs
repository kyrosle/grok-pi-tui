//! Compatibility output definitions; native consumers use the canonical wire layer.
use serde::{Deserialize, Serialize};
pub use xai_tool_types::output::*;
#[cfg(test)]
use xai_tool_types::output_dependencies::{SendSubagentMessageOutput, SkillOutput};
#[cfg(test)]
use xai_tool_types::todo::{TodoItem, TodoState};
#[cfg(test)]
use xai_tool_types::{KillTaskOutput, SubagentCompletedOutput, TaskOutputOutput};

/// Result of running a tool through the ToolRunner pipeline. This is the **single return type**
/// from `ToolRunner::run()`. Clean `output` — never mutated by layers; for JSON serialization,
/// protocol translation. `prompt_text` — rendered with system reminders appended; for model prompt.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolRunResult {
    /// Clean tool output — never mutated by layers.
    /// Consumers use this for: JSON serialization, protocol translation, hunk tracking.
    pub output: ToolOutput,
    /// Prompt-ready text — layers can append system reminders, etc.
    /// Consumers use this for: model prompt (ConversationItem::tool_result).
    pub prompt_text: String,
    /// When a meta-tool dispatches to a different underlying tool (for example
    /// `use_tool` → `linear__save_issue`), this carries the effective tool name.
    /// `None` means the requested tool and executed tool are the same.
    pub effective_tool_name: Option<String>,
}
impl ToolRunResult {
    /// Like [`TypedToolOutput::from_value`], but reattaches `chat_completion_output` from `output`.
    pub fn into_typed_tool_output(
        self,
        tool_id: xai_tool_protocol::ToolId,
    ) -> xai_tool_runtime::TypedToolOutput {
        typed_tool_output_preserving_cco(tool_id, &self, &self.output)
    }
}
/// Like [`TypedToolOutput::from_value`], but reattaches `chat_completion_output` from `source`.
pub(crate) fn typed_tool_output_preserving_cco(
    tool_id: xai_tool_protocol::ToolId,
    payload: &impl Serialize,
    source: &impl xai_tool_runtime::ToolOutput,
) -> xai_tool_runtime::TypedToolOutput {
    let cco = source.chat_completion_output();
    let value = serde_json::to_value(payload).unwrap_or(serde_json::Value::Null);
    xai_tool_runtime::TypedToolOutput::from_value(tool_id, value).with_chat_completion_output(cco)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::grok_build::todo::{TodoPriority, TodoStatus};
    use serde_json::json;
    use xai_tool_types::KillTaskResult;
    use xai_tool_types::TaskOutputResult;
    #[test]
    fn send_subagent_message_error_classification_is_closed() {
        use crate::implementations::grok_build::send_subagent_message::SendSubagentMessageOutput::*;
        for (output, is_error) in [
            (
                Accepted {
                    message_id: "m-1".into(),
                },
                false,
            ),
            (NotFoundOrNotOwned, true),
            (NotActiveOrFinalizing, true),
            (Saturated { max_in_flight: 8 }, true),
            (AdmissionUncertain, false),
            (NotAcceptedBeforeDeadline, true),
            (Unsupported, true),
            (
                Limit {
                    max_bytes: 8,
                    observed_bytes: 9,
                },
                true,
            ),
            (ChannelClosed, true),
        ] {
            assert_eq!(ToolOutput::SendSubagentMessage(output).is_error(), is_error);
        }
    }
    #[test]
    fn legacy_send_agent_message_output_envelope_deserializes() {
        let output: ToolOutput = serde_json::from_value(serde_json::json!({
            "type": "SendAgentMessage",
            "outcome": "channel_closed",
        }))
        .expect("legacy output envelope must remain replayable");
        assert!(matches!(
            output,
            ToolOutput::SendSubagentMessage(SendSubagentMessageOutput::ChannelClosed)
        ));
    }
    /// Serialize a ToolOutput to JSON value
    fn to_json(output: ToolOutput) -> serde_json::Value {
        serde_json::to_value(&output).unwrap()
    }
    #[test]
    fn mcp_extracted_images_survive_hub_json_roundtrip() {
        let mut mcp = MCPOutput::okay_output(
            "browser_screenshot".into(),
            "browser-use".into(),
            crate::util::base64_images::IMAGE_CONTENT_PLACEHOLDER.into(),
        );
        let payload = "A".repeat(50_000);
        mcp.extracted_images = vec![crate::util::base64_images::ExtractedImage {
            data: payload.clone(),
            mime_type: "image/png".into(),
        }];
        let v = serde_json::to_value(&mcp).unwrap();
        assert!(
            v.get("extracted_images").is_some(),
            "hub ToolDyn to_value must keep non-empty extracted_images"
        );
        let back: MCPOutput = serde_json::from_value(v).unwrap();
        assert_eq!(back.extracted_images.len(), 1);
        assert_eq!(back.extracted_images[0].data, payload);
        assert_eq!(back.extracted_images[0].mime_type, "image/png");
    }
    #[test]
    fn tool_output_mcp_extracted_images_survive_hub_roundtrip() {
        let mut mcp = MCPOutput::okay_output(
            "t".into(),
            "s".into(),
            crate::util::base64_images::IMAGE_CONTENT_PLACEHOLDER.into(),
        );
        let payload = "C".repeat(12_000);
        mcp.extracted_images = vec![crate::util::base64_images::ExtractedImage {
            data: payload.clone(),
            mime_type: "image/webp".into(),
        }];
        let output = ToolOutput::MCP(mcp);
        let v = serde_json::to_value(&output).unwrap();
        let back: ToolOutput = serde_json::from_value(v).unwrap();
        let ToolOutput::MCP(mcp) = back else {
            panic!("expected MCP");
        };
        assert_eq!(mcp.extracted_images.len(), 1);
        assert_eq!(mcp.extracted_images[0].data, payload);
    }
    #[test]
    fn file_content_extracted_images_survive_hub_json_roundtrip() {
        let payload = "B".repeat(40_000);
        let fc = FileContent {
            content: crate::util::base64_images::IMAGE_CONTENT_PLACEHOLDER.into(),
            content_concise: None,
            absolute_path: PathBuf::from("/tmp/x.png"),
            offset: None,
            limit: None,
            raw_output: String::new(),
            total_lines: 1,
            extracted_images: vec![crate::util::base64_images::ExtractedImage {
                data: payload.clone(),
                mime_type: "image/jpeg".into(),
            }],
        };
        let v = serde_json::to_value(&fc).unwrap();
        assert!(v.get("extracted_images").is_some());
        let back: FileContent = serde_json::from_value(v).unwrap();
        assert_eq!(back.extracted_images.len(), 1);
        assert_eq!(back.extracted_images[0].data, payload);
    }
    #[test]
    fn empty_extracted_images_omitted_from_json() {
        let mcp = MCPOutput::okay_output("t".into(), "s".into(), "plain".into());
        let v = serde_json::to_value(&mcp).unwrap();
        assert!(v.get("extracted_images").is_none());
    }
    #[test]
    fn tool_output_read_file_extracted_images_survive_hub_roundtrip() {
        let payload = "D".repeat(18_000);
        let fc = FileContent {
            content: crate::util::base64_images::IMAGE_CONTENT_PLACEHOLDER.into(),
            content_concise: None,
            absolute_path: PathBuf::from("/tmp/y.png"),
            offset: None,
            limit: None,
            raw_output: String::new(),
            total_lines: 1,
            extracted_images: vec![crate::util::base64_images::ExtractedImage {
                data: payload.clone(),
                mime_type: "image/png".into(),
            }],
        };
        let output = ToolOutput::ReadFile(ReadFileOutput::FileContent(fc));
        let v = serde_json::to_value(&output).unwrap();
        let back: ToolOutput = serde_json::from_value(v).unwrap();
        let ToolOutput::ReadFile(ReadFileOutput::FileContent(fc)) = back else {
            panic!("expected FileContent");
        };
        assert_eq!(fc.extracted_images.len(), 1);
        assert_eq!(fc.extracted_images[0].data, payload);
        assert_eq!(fc.extracted_images[0].mime_type, "image/png");
    }
    fn empty_file_content(offset: Option<usize>, total_lines: usize) -> FileContent {
        FileContent {
            content: String::new(),
            content_concise: None,
            absolute_path: PathBuf::from("/tmp/f.txt"),
            offset,
            limit: None,
            raw_output: String::new(),
            total_lines,
            extracted_images: vec![],
        }
    }
    /// An empty file must render an explicit notice, not a blank result.
    #[test]
    fn read_empty_file_prompt_says_file_is_empty() {
        let output = ToolOutput::ReadFile(ReadFileOutput::FileContent(empty_file_content(None, 0)));
        assert_eq!(output.to_prompt_format(), "File is empty.");
    }
    /// An offset beyond the last line must say past-EOF and report the real
    /// line count.
    #[test]
    fn read_past_eof_prompt_reports_line_count() {
        let output = ToolOutput::ReadFile(ReadFileOutput::FileContent(empty_file_content(
            Some(101),
            100,
        )));
        let prompt = output.to_prompt_format();
        assert!(
            prompt.contains("past the end of the file"),
            "expected past-EOF notice, got: {prompt}"
        );
        assert!(
            prompt.contains("100 lines"),
            "expected real line count, got: {prompt}"
        );
    }
    /// An empty window with an in-range offset (e.g. `limit: 0`) must render
    /// the generic notice, not a bogus past-EOF claim.
    #[test]
    fn read_empty_window_in_range_offset_is_not_past_eof() {
        let output = ToolOutput::ReadFile(ReadFileOutput::FileContent(empty_file_content(
            Some(5),
            100,
        )));
        let prompt = output.to_prompt_format();
        assert_eq!(prompt, "(no lines returned)");
        assert!(
            !prompt.contains("past the end of the file"),
            "in-range empty window must not claim past-EOF: {prompt}"
        );
    }
    /// Non-empty content renders unchanged.
    #[test]
    fn read_non_empty_content_renders_verbatim() {
        let mut fc = empty_file_content(None, 3);
        fc.content = "1→a\nb\nc".to_string();
        let output = ToolOutput::ReadFile(ReadFileOutput::FileContent(fc));
        assert_eq!(output.to_prompt_format(), "1→a\nb\nc");
    }
    #[test]
    fn text_output_to_prompt_format_omits_consumed_completion_task_id() {
        let output = ToolOutput::Text(TextOutput {
            text: "Task completed in 100ms with exit code: 0.".into(),
            consumed_completion_task_id: Some("bg-uuid-42".into()),
        });
        assert_eq!(
            output.to_prompt_format(),
            "Task completed in 100ms with exit code: 0."
        );
    }
    #[test]
    fn text_output_serde_omits_consumed_id_when_none() {
        let text = TextOutput {
            text: "hello".into(),
            consumed_completion_task_id: None,
        };
        let json = serde_json::to_value(&text).unwrap();
        let obj = json.as_object().expect("object");
        assert_eq!(obj.len(), 1);
        assert!(!obj.contains_key("consumed_completion_task_id"));
        let round_trip: TextOutput = serde_json::from_value(json).unwrap();
        assert_eq!(round_trip.text, "hello");
        assert!(round_trip.consumed_completion_task_id.is_none());
    }
    #[test]
    fn text_output_serde_includes_consumed_id_when_some() {
        let text = TextOutput {
            text: "Task completed".into(),
            consumed_completion_task_id: Some("task-abc".into()),
        };
        let json = serde_json::to_value(&text).unwrap();
        assert_eq!(json["consumed_completion_task_id"], "task-abc");
        let round_trip: TextOutput = serde_json::from_value(json).unwrap();
        assert_eq!(
            round_trip.consumed_completion_task_id.as_deref(),
            Some("task-abc")
        );
    }
    #[test]
    fn media_gen_output() {
        let cases = [
            (
                ToolOutput::ImageGen(MediaGenOutput::new("/tmp/images/1.jpg".into())),
                "ImageGen",
                "/tmp/images/1.jpg",
                "1.jpg",
                "images",
                "Image generated and saved to /tmp/images/1.jpg. Do not read or re-display it, and do not describe how it appears to the user.",
            ),
            (
                ToolOutput::ImageToVideo(MediaGenOutput::new("/tmp/videos/2.mp4".into())),
                "ImageToVideo",
                "/tmp/videos/2.mp4",
                "2.mp4",
                "videos",
                "Video generated and saved to /tmp/videos/2.mp4. Do not read or re-display it, and do not describe how it appears to the user.",
            ),
            (
                ToolOutput::ReferenceToVideo(MediaGenOutput::new("/tmp/videos/3.mp4".into())),
                "ReferenceToVideo",
                "/tmp/videos/3.mp4",
                "3.mp4",
                "videos",
                "Video generated and saved to /tmp/videos/3.mp4. Do not read or re-display it, and do not describe how it appears to the user.",
            ),
            (
                ToolOutput::ImageEdit(MediaGenOutput::new("/tmp/images/2.jpg".into())),
                "ImageEdit",
                "/tmp/images/2.jpg",
                "2.jpg",
                "images",
                "Image edited and saved to /tmp/images/2.jpg. Do not read or re-display it, and do not describe how it appears to the user.",
            ),
        ];
        for (output, ty, path, filename, session_folder, message) in cases {
            let prompt_json: serde_json::Value =
                serde_json::from_str(&output.to_prompt_format()).unwrap();
            assert_eq!(prompt_json["path"], path);
            assert_eq!(prompt_json["filename"], filename);
            assert_eq!(prompt_json["session_folder"], session_folder);
            assert_eq!(prompt_json["message"], message);
            let json = to_json(output);
            assert_eq!(json["type"], ty);
            assert_eq!(json["path"], path);
            assert_eq!(json["filename"], filename);
            assert_eq!(json["session_folder"], session_folder);
            let (ToolOutput::ImageGen(m)
            | ToolOutput::ImageToVideo(m)
            | ToolOutput::ReferenceToVideo(m)
            | ToolOutput::ImageEdit(m)) = serde_json::from_value(json).unwrap()
            else {
                panic!("unexpected variant");
            };
            assert_eq!(m.path, PathBuf::from(path));
            assert_eq!(m.filename, filename);
            assert_eq!(m.session_folder, session_folder);
        }
    }
    #[test]
    fn media_gen_output_uploaded() {
        let url = "https://files.example.com/team/video-abc.mp4";
        let output = ToolOutput::ImageToVideo(MediaGenOutput::uploaded(url.to_string()));
        let prompt = output.to_prompt_format();
        assert!(prompt.contains(url), "prompt must include the upload URL");
        let json = to_json(output);
        assert_eq!(json["uploaded_url"], url);
        assert!(
            json.get("path").is_some(),
            "path field must be present (empty for uploaded)"
        );
        let ToolOutput::ImageToVideo(m) = serde_json::from_value(json).unwrap() else {
            panic!("unexpected variant");
        };
        assert_eq!(m, MediaGenOutput::uploaded(url.to_string()));
    }
    #[test]
    fn read_file_not_found_json() {
        let json =
            to_json(ReadFileOutput::FileNotFound("Error: /tmp/x does not exist.".into()).into());
        assert_eq!(
            json,
            json!({"type": "ReadFile", "FileNotFound": "Error: /tmp/x does not exist."})
        );
    }
    #[test]
    fn read_file_is_a_directory_json() {
        let json =
            to_json(ReadFileOutput::IsADirectory("Error: /tmp is a directory.".into()).into());
        assert_eq!(
            json,
            json!({"type": "ReadFile", "IsADirectory": "Error: /tmp is a directory."})
        );
    }
    #[test]
    fn read_file_permission_denied_json() {
        let json = to_json(
            ReadFileOutput::PermissionDenied("Permission denied: /etc/shadow".into()).into(),
        );
        assert_eq!(
            json,
            json!({"type": "ReadFile", "PermissionDenied": "Permission denied: /etc/shadow"})
        );
    }
    #[test]
    fn read_file_too_large_json() {
        let json = to_json(
            ReadFileOutput::FileTooLarge(
                "File content (37044 tokens) exceeds maximum allowed tokens (25000 tokens).".into(),
            )
            .into(),
        );
        assert_eq!(
            json,
            json!({"type": "ReadFile", "FileTooLarge": "File content (37044 tokens) exceeds maximum allowed tokens (25000 tokens)."})
        );
    }
    #[test]
    fn read_file_generic_error_json() {
        let json = to_json(ReadFileOutput::FileReadError("Failed to read file".into()).into());
        assert_eq!(
            json,
            json!({"type": "ReadFile", "FileReadError": "Failed to read file"})
        );
    }
    #[test]
    fn read_file_image_size_error_json() {
        let json = to_json(ReadFileOutput::ImageSizeError("Image too large".into()).into());
        assert_eq!(
            json,
            json!({"type": "ReadFile", "ImageSizeError": "Image too large"})
        );
    }
    #[test]
    fn list_dir_not_found_json() {
        let json = to_json(ListDirOutput::NotFound("does not exist".into()).into());
        assert_eq!(
            json,
            json!({"type": "ListDir", "NotFound": "does not exist"})
        );
    }
    #[test]
    fn list_dir_is_a_file_json() {
        let json = to_json(ListDirOutput::IsAFile("is a file".into()).into());
        assert_eq!(json, json!({"type": "ListDir", "IsAFile": "is a file"}));
    }
    #[test]
    fn list_dir_not_a_directory_json() {
        let json = to_json(ListDirOutput::NotADirectory("is not a directory".into()).into());
        assert_eq!(
            json,
            json!({"type": "ListDir", "NotADirectory": "is not a directory"})
        );
    }
    #[test]
    fn list_dir_permission_denied_json() {
        let json = to_json(ListDirOutput::PermissionDenied("Permission denied".into()).into());
        assert_eq!(
            json,
            json!({"type": "ListDir", "PermissionDenied": "Permission denied"})
        );
    }
    #[test]
    fn list_dir_generic_error_json() {
        let json = to_json(ListDirOutput::Error("Some error".into()).into());
        assert_eq!(json, json!({"type": "ListDir", "Error": "Some error"}));
    }
    #[test]
    fn search_replace_file_not_found_json() {
        let json = to_json(SearchReplaceOutput::FileNotFound("not found".into()).into());
        assert_eq!(
            json,
            json!({"type": "SearchReplace", "FileNotFound": "not found"})
        );
    }
    #[test]
    fn search_replace_no_matches_json() {
        let json = to_json(
            SearchReplaceOutput::NoMatchesFound(crate::types::output::NoMatchesFoundError {
                message: "no matches".into(),
                file_path: std::path::PathBuf::from("/project/src/main.c"),
                file_snapshot_at_edit: None,
            })
            .into(),
        );
        assert_eq!(
            json,
            json!({
                "type": "SearchReplace",
                "NoMatchesFound": {
                    "message": "no matches",
                    "file_path": "/project/src/main.c"
                }
            })
        );
    }
    #[test]
    fn search_replace_no_matches_omits_file_snapshot_from_json() {
        let err = crate::types::output::NoMatchesFoundError {
            message: "no matches".into(),
            file_path: std::path::PathBuf::from("/project/secret.txt"),
            file_snapshot_at_edit: Some("SECRET_PAYLOAD".repeat(20)),
        };
        let json = to_json(SearchReplaceOutput::NoMatchesFound(err).into());
        let inner = json
            .get("NoMatchesFound")
            .and_then(|v| v.as_object())
            .expect("NoMatchesFound object");
        assert_eq!(inner.len(), 2);
        assert!(!inner.contains_key("file_snapshot_at_edit"));
        assert!(!json.to_string().contains("SECRET_PAYLOAD"));
    }
    #[test]
    fn search_replace_multiple_matches_json() {
        let json = to_json(SearchReplaceOutput::MultipleMatchesFound("3 matches".into()).into());
        assert_eq!(
            json,
            json!({"type": "SearchReplace", "MultipleMatchesFound": "3 matches"})
        );
    }
    #[test]
    fn search_replace_file_already_exists_json() {
        let json = to_json(SearchReplaceOutput::FileAlreadyExists("exists".into()).into());
        assert_eq!(
            json,
            json!({"type": "SearchReplace", "FileAlreadyExists": "exists"})
        );
    }
    #[test]
    fn search_replace_invalid_input_json() {
        let json = to_json(SearchReplaceOutput::InvalidInput("same strings".into()).into());
        assert_eq!(
            json,
            json!({"type": "SearchReplace", "InvalidInput": "same strings"})
        );
    }
    #[test]
    fn search_replace_filename_too_long_json() {
        let json = to_json(SearchReplaceOutput::FilenameTooLong("name too long".into()).into());
        assert_eq!(
            json,
            json!({"type": "SearchReplace", "FilenameTooLong": "name too long"})
        );
    }
    #[test]
    fn apply_patch_parse_error_json() {
        let json = to_json(ApplyPatchOutput::ParseError("Invalid patch: boom".into()).into());
        assert_eq!(
            json,
            json!({"type": "ApplyPatch", "ParseError": "Invalid patch: boom"})
        );
    }
    #[test]
    fn apply_patch_application_error_json() {
        let json = to_json(
            ApplyPatchOutput::ApplicationError("File /tmp/x.rs does not exist".into()).into(),
        );
        assert_eq!(
            json,
            json!({"type": "ApplyPatch", "ApplicationError": "File /tmp/x.rs does not exist"})
        );
    }
    #[test]
    fn apply_patch_empty_patch_json() {
        let json = to_json(ApplyPatchOutput::EmptyPatch("No files were modified.".into()).into());
        assert_eq!(
            json,
            json!({"type": "ApplyPatch", "EmptyPatch": "No files were modified."})
        );
    }
    /// `Success` must not serialize under any of the keys Python treats as a
    /// failure, otherwise a successful patch would be taxed as a tool error.
    #[test]
    fn apply_patch_success_json_is_not_an_error_shape() {
        let json = to_json(
            ApplyPatchOutput::Success {
                files: vec![ApplyPatchFileResult {
                    path: PathBuf::from("/repo/src/main.rs"),
                    action: "modified".into(),
                    old_text: Some("old".into()),
                    new_text: "new".into(),
                    move_to: None,
                }],
                tool_output_for_prompt: "Updated /repo/src/main.rs".into(),
            }
            .into(),
        );
        assert_eq!(json["type"], "ApplyPatch");
        assert!(json.get("Success").is_some(), "missing Success key: {json}");
        for key in ["ParseError", "ApplicationError", "EmptyPatch"] {
            assert!(
                json.get(key).is_none(),
                "success must not serialize under the error key {key}: {json}"
            );
        }
    }
    #[test]
    fn kill_task_result_json() {
        let json = to_json(
            KillTaskOutput::Result(KillTaskResult {
                task_id: "task-1".into(),
                outcome: "killed".into(),
                message: "Task was terminated successfully".into(),
            })
            .into(),
        );
        assert_eq!(json["type"], "KillTask");
        assert!(json.get("Result").is_some(), "missing Result key: {json}");
        assert_eq!(json["Result"]["task_id"], "task-1");
        assert_eq!(json["Result"]["outcome"], "killed");
    }
    #[test]
    fn kill_task_not_found_json() {
        let json = to_json(
            KillTaskOutput::TaskNotFound(
                "Task abc not found. No background tasks exist in this session.".into(),
            )
            .into(),
        );
        assert_eq!(
            json,
            json!({
                "type": "KillTask",
                "TaskNotFound": "Task abc not found. No background tasks exist in this session."
            })
        );
    }
    #[test]
    fn kill_task_not_found_round_trip() {
        let original = KillTaskOutput::TaskNotFound("not found".into());
        let serialized = serde_json::to_value(&original).unwrap();
        let deserialized: KillTaskOutput = serde_json::from_value(serialized).unwrap();
        assert!(
            matches!(deserialized, KillTaskOutput::TaskNotFound(ref msg) if msg == "not found")
        );
    }
    #[test]
    fn task_output_result_json() {
        let json = to_json(
            TaskOutputOutput::Result(TaskOutputResult {
                task_id: "task-1".into(),
                command: "sleep 10".into(),
                status: "running".into(),
                exit_code: None,
                started: "2026-03-09T00:00:00Z".into(),
                ended: None,
                duration_secs: 5.0,
                output: "hello".into(),
                output_file: "/tmp/task-1.log".into(),
                truncated: false,
                truncation_hint: String::new(),
                raw_output_bytes: 5,
            })
            .into(),
        );
        assert_eq!(json["type"], "TaskOutput");
        assert!(json.get("Result").is_some(), "missing Result key: {json}");
        assert_eq!(json["Result"]["task_id"], "task-1");
        assert_eq!(json["Result"]["status"], "running");
    }
    /// The single-task detail view is duration-only: absolute `started` /
    /// `ended` instants stay on the wire struct but must not reach the prompt.
    #[test]
    fn task_output_prompt_is_duration_only() {
        let out = ToolOutput::TaskOutput(TaskOutputOutput::Result(TaskOutputResult {
            task_id: "task-1".into(),
            command: "sleep 10".into(),
            status: "completed".into(),
            exit_code: Some(0),
            started: "2026-03-09T00:00:00Z".into(),
            ended: Some("2026-03-09T00:00:05Z".into()),
            duration_secs: 5.0,
            output: "hello".into(),
            output_file: "/tmp/task-1.log".into(),
            truncated: false,
            truncation_hint: String::new(),
            raw_output_bytes: 5,
        }));
        let prompt = out.to_prompt_format();
        assert!(prompt.contains("Duration: 5.00s"), "{prompt}");
        assert!(prompt.contains("Output File: /tmp/task-1.log"), "{prompt}");
        assert!(
            !prompt.contains("Started") && !prompt.contains("Ended"),
            "absolute instants must not be model-visible: {prompt}"
        );
        assert!(
            !prompt.contains("2026-03-09"),
            "no wall-clock date may survive into the prompt: {prompt}"
        );
    }
    #[test]
    fn task_output_prompt_omits_empty_output_file() {
        let out = ToolOutput::TaskOutput(TaskOutputOutput::Result(TaskOutputResult {
            task_id: "task-2".into(),
            command: "true".into(),
            status: "completed".into(),
            exit_code: Some(0),
            started: String::new(),
            ended: None,
            duration_secs: 0.1,
            output: "done".into(),
            output_file: String::new(),
            truncated: false,
            truncation_hint: String::new(),
            raw_output_bytes: 4,
        }));
        let prompt = out.to_prompt_format();
        assert!(!prompt.contains("Output File"), "{prompt}");
    }
    fn make_result(status: &str, raw_output_bytes: usize) -> TaskOutputResult {
        TaskOutputResult {
            task_id: "t".into(),
            command: "cmd".into(),
            status: status.into(),
            exit_code: None,
            started: "2026-01-01T00:00:00Z".into(),
            ended: None,
            duration_secs: 1.0,
            output: "x".repeat(raw_output_bytes.min(100)),
            output_file: "/tmp/t.log".into(),
            truncated: raw_output_bytes > 100,
            truncation_hint: String::new(),
            raw_output_bytes,
        }
    }
    /// Identical status + raw_output_bytes → same signature.
    #[test]
    fn progress_signature_same_when_no_progress() {
        let a = make_result("running", 1000);
        let b = make_result("running", 1000);
        assert_eq!(
            a.progress_signature(),
            b.progress_signature(),
            "identical results must produce the same progress signature"
        );
    }
    /// Growing raw_output_bytes must produce a different signature even when the
    /// formatted output length is unchanged (truncated output scenario).
    #[test]
    fn progress_signature_differs_on_raw_output_growth() {
        let stagnant = make_result("running", 500_000);
        let growing = make_result("running", 500_001);
        assert_ne!(
            stagnant.progress_signature(),
            growing.progress_signature(),
            "raw output growth must produce a different progress signature"
        );
    }
    /// Status change must produce a different signature.
    #[test]
    fn progress_signature_differs_on_status_change() {
        let running = make_result("running", 100);
        let completed = make_result("completed", 100);
        assert_ne!(
            running.progress_signature(),
            completed.progress_signature(),
            "status change must produce a different progress signature"
        );
    }
    /// `raw_output_bytes` field is populated in JSON output.
    #[test]
    fn task_output_result_raw_output_bytes_in_json() {
        let json = to_json(
            TaskOutputOutput::Result(TaskOutputResult {
                task_id: "t2".into(),
                command: "cmd".into(),
                status: "running".into(),
                exit_code: None,
                started: "2026-03-15T00:00:00Z".into(),
                ended: None,
                duration_secs: 0.0,
                output: "hello world".into(),
                output_file: "/tmp/t2.log".into(),
                truncated: false,
                truncation_hint: String::new(),
                raw_output_bytes: 11,
            })
            .into(),
        );
        assert_eq!(json["Result"]["raw_output_bytes"], 11);
    }
    #[test]
    fn task_output_not_found_json() {
        let json = to_json(
            TaskOutputOutput::TaskNotFound(
                "Task xyz not found. Known task IDs: [task-1, task-2]".into(),
            )
            .into(),
        );
        assert_eq!(
            json,
            json!({
                "type": "TaskOutput",
                "TaskNotFound": "Task xyz not found. Known task IDs: [task-1, task-2]"
            })
        );
    }
    #[test]
    fn task_output_not_found_round_trip() {
        let original = TaskOutputOutput::TaskNotFound("not found".into());
        let serialized = serde_json::to_value(&original).unwrap();
        let deserialized: TaskOutputOutput = serde_json::from_value(serialized).unwrap();
        assert!(
            matches!(deserialized, TaskOutputOutput::TaskNotFound(ref msg) if msg == "not found")
        );
    }
    #[test]
    fn todo_write_success_json() {
        let json = to_json(
            TodoWriteOutput::TodosUpdated(TodoWriteSuccess {
                summary_for_prompt: "- [pending] 1: Task A\n".into(),
                todos: vec![TodoItem {
                    content: "Task A".into(),
                    priority: TodoPriority::Medium,
                    status: TodoStatus::Pending,
                    meta: None,
                }],
                state: TodoState::default(),
            })
            .into(),
        );
        assert_eq!(json["type"], "Todo");
        assert!(
            json.get("TodosUpdated").is_some(),
            "missing TodosUpdated key: {json}"
        );
        assert_eq!(
            json["TodosUpdated"]["summary_for_prompt"],
            "- [pending] 1: Task A\n"
        );
        assert_eq!(json["TodosUpdated"]["todos"][0]["content"], "Task A");
        assert_eq!(json["TodosUpdated"]["todos"][0]["status"], "pending");
    }
    #[test]
    fn todo_write_duplicate_id_json() {
        let json = to_json(
            TodoWriteOutput::DuplicateId(
                "Duplicate todo ID in request: \"dup\". Each todo item must have a unique ID."
                    .into(),
            )
            .into(),
        );
        assert_eq!(
            json,
            json!({
                "type": "Todo",
                "DuplicateId": "Duplicate todo ID in request: \"dup\". Each todo item must have a unique ID."
            })
        );
    }
    #[test]
    fn todo_write_duplicate_id_round_trip() {
        let original = TodoWriteOutput::DuplicateId("dup id".into());
        let serialized = serde_json::to_value(&original).unwrap();
        let deserialized: TodoWriteOutput = serde_json::from_value(serialized).unwrap();
        assert!(matches!(deserialized, TodoWriteOutput::DuplicateId(ref msg) if msg == "dup id"));
    }
    #[test]
    fn todo_write_success_round_trip() {
        let original = TodoWriteOutput::TodosUpdated(TodoWriteSuccess {
            summary_for_prompt: "summary".into(),
            todos: vec![TodoItem {
                content: "task".into(),
                priority: TodoPriority::High,
                status: TodoStatus::InProgress,
                meta: None,
            }],
            state: TodoState::default(),
        });
        let serialized = serde_json::to_value(&original).unwrap();
        let deserialized: TodoWriteOutput = serde_json::from_value(serialized).unwrap();
        match deserialized {
            TodoWriteOutput::TodosUpdated(s) => {
                assert_eq!(s.summary_for_prompt, "summary");
                assert_eq!(s.todos.len(), 1);
                assert_eq!(s.todos[0].content, "task");
                assert_eq!(s.todos[0].status, TodoStatus::InProgress);
                assert_eq!(s.todos[0].priority, TodoPriority::High);
            }
            other => panic!("expected TodosUpdated, got {other:?}"),
        }
    }
    #[test]
    fn subagent_completed_prompt_format_includes_resume_footer() {
        let output = ToolOutput::SubagentCompleted(SubagentCompletedOutput {
            output: "I found the auth middleware.".into(),
            subagent_id: "019e0000-0000-7000-8000-0000000000bb".into(),
            subagent_type: "explore".into(),
            tool_calls: 5,
            turns: 2,
            duration_ms: 3000,
            worktree_path: None,
            persona: None,
            resume_from_hint: "019e0000-0000-7000-8000-0000000000bb".into(),
            persona_hint: None,
        });
        let rendered = output.to_prompt_format();
        assert!(
            rendered.contains("I found the auth middleware."),
            "original output preserved"
        );
        assert!(
            rendered.contains("subagent_id: 019e0000-0000-7000-8000-0000000000bb"),
            "subagent_id visible in rendered text"
        );
        assert!(
            rendered.contains("resume_from=\"019e0000-0000-7000-8000-0000000000bb\""),
            "resume_from hint with correct ID"
        );
        assert!(
            rendered.contains("subagent_type: explore"),
            "subagent_type visible"
        );
        assert!(
            rendered.contains("<subagent_result>"),
            "wrapped in subagent_result tag"
        );
        assert!(
            !rendered.contains("persona"),
            "no persona hint when persona is None"
        );
    }
    #[test]
    fn subagent_completed_prompt_format_includes_persona_hint() {
        let output = ToolOutput::SubagentCompleted(SubagentCompletedOutput {
            output: "Done implementing.".into(),
            subagent_id: "abc-123".into(),
            subagent_type: "general-purpose".into(),
            tool_calls: 10,
            turns: 3,
            duration_ms: 5000,
            worktree_path: None,
            persona: Some("implementer".into()),
            resume_from_hint: "abc-123".into(),
            persona_hint: Some("implementer".into()),
        });
        let rendered = output.to_prompt_format();
        assert!(
            rendered.contains("resume_from=\"abc-123\""),
            "resume hint present"
        );
        assert!(
            rendered.contains("persona=\"implementer\""),
            "persona hint present"
        );
    }
    #[test]
    fn subagent_completed_prompt_format_with_worktree() {
        let output = ToolOutput::SubagentCompleted(SubagentCompletedOutput {
            output: "Changes committed.".into(),
            subagent_id: "wt-agent".into(),
            subagent_type: "general-purpose".into(),
            tool_calls: 3,
            turns: 1,
            duration_ms: 2000,
            worktree_path: Some("/tmp/grok-worktree/wt-agent".into()),
            persona: None,
            resume_from_hint: "wt-agent".into(),
            persona_hint: None,
        });
        let rendered = output.to_prompt_format();
        assert!(
            rendered.contains("<worktree_path>/tmp/grok-worktree/wt-agent</worktree_path>"),
            "worktree_path preserved"
        );
        assert!(
            rendered.contains("resume_from=\"wt-agent\""),
            "resume footer still present with worktree"
        );
    }
    #[test]
    fn subagent_completed_structured_hints_serialize() {
        let output = SubagentCompletedOutput {
            output: "done".into(),
            subagent_id: "sub-abc-123".into(),
            subagent_type: "general-purpose".into(),
            tool_calls: 5,
            turns: 2,
            duration_ms: 3000,
            worktree_path: None,
            persona: Some("implementer".into()),
            resume_from_hint: "sub-abc-123".into(),
            persona_hint: Some("implementer".into()),
        };
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["resume_from_hint"], "sub-abc-123");
        assert_eq!(json["persona_hint"], "implementer");
        assert_eq!(json["subagent_id"], json["resume_from_hint"]);
    }
    #[test]
    fn enter_plan_mode_tool_hints_default() {
        let hints = EnterPlanModeToolHints::default();
        assert_eq!(hints.ask_user, "ask_user_question");
        assert_eq!(hints.exit_plan, "exit_plan_mode");
        assert!(hints.task.is_empty());
    }
    #[test]
    fn enter_plan_mode_tool_hints_serde_round_trip() {
        let hints = EnterPlanModeToolHints {
            ask_user: "AskUser".into(),
            exit_plan: "FinishPlan".into(),
            task: "task".into(),
        };
        let json = serde_json::to_value(&hints).unwrap();
        assert_eq!(json["ask_user"], "AskUser");
        assert_eq!(json["exit_plan"], "FinishPlan");
        assert_eq!(json["task"], "task");
        let deserialized: EnterPlanModeToolHints = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized.ask_user, "AskUser");
        assert_eq!(deserialized.exit_plan, "FinishPlan");
        assert_eq!(deserialized.task, "task");
    }
    #[test]
    fn enter_plan_mode_tool_hints_defaults_on_missing_fields() {
        let json = json!({});
        let hints: EnterPlanModeToolHints = serde_json::from_value(json).unwrap();
        assert_eq!(hints.ask_user, "ask_user_question");
        assert_eq!(hints.exit_plan, "exit_plan_mode");
        assert!(hints.task.is_empty());
    }
    #[test]
    fn enter_plan_mode_prompt_format_with_default_hints() {
        let output = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "entered-msg-token".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints::default(),
            plan_file_seed: PlanFileSeedStatus::Empty,
        });
        let prompt = output.to_prompt_format();
        assert!(prompt.contains("entered-msg-token"));
        assert!(prompt.contains("/tmp/plan.md"));
        assert!(prompt.contains("ask_user_question"));
        assert!(prompt.contains("exit_plan_mode"));
        assert!(
            !prompt.contains("subagent_type"),
            "should not contain subagent guidance without task tool"
        );
    }
    #[test]
    fn enter_plan_mode_prompt_format_with_task_tool() {
        let output = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints {
                ask_user: "ask_user_question".into(),
                exit_plan: "exit_plan_mode".into(),
                task: "delegate-xyz".into(),
            },
            plan_file_seed: PlanFileSeedStatus::Empty,
        });
        let prompt = output.to_prompt_format();
        assert!(prompt.contains("delegate-xyz"));
        assert!(prompt.contains("subagent_type"));
    }
    #[test]
    fn enter_plan_mode_prompt_format_with_custom_tool_names() {
        let output = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/session/plan.md".into(),
            tool_hints: EnterPlanModeToolHints {
                ask_user: "AskUser".into(),
                exit_plan: "FinishPlan".into(),
                task: String::new(),
            },
            plan_file_seed: PlanFileSeedStatus::Empty,
        });
        let prompt = output.to_prompt_format();
        assert!(prompt.contains("AskUser"));
        assert!(prompt.contains("FinishPlan"));
        assert!(!prompt.contains("ask_user_question"));
        assert!(!prompt.contains("exit_plan_mode"));
    }
    #[test]
    fn enter_plan_mode_output_serde_with_tool_hints() {
        let output = EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints {
                ask_user: "AskUser".into(),
                exit_plan: "FinishPlan".into(),
                task: "delegate".into(),
            },
            plan_file_seed: PlanFileSeedStatus::Empty,
        };
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["Entered"]["tool_hints"]["ask_user"], "AskUser");
        assert_eq!(json["Entered"]["tool_hints"]["exit_plan"], "FinishPlan");
        assert_eq!(json["Entered"]["tool_hints"]["task"], "delegate");
        assert_eq!(json["Entered"]["plan_file_seed"], "empty");
        let deserialized: EnterPlanModeOutput = serde_json::from_value(json).unwrap();
        match deserialized {
            EnterPlanModeOutput::Entered {
                tool_hints,
                plan_file_seed,
                ..
            } => {
                assert_eq!(tool_hints.ask_user, "AskUser");
                assert_eq!(tool_hints.exit_plan, "FinishPlan");
                assert_eq!(tool_hints.task, "delegate");
                assert_eq!(plan_file_seed, PlanFileSeedStatus::Empty);
            }
        }
    }
    #[test]
    fn enter_plan_mode_output_serde_defaults_tool_hints_when_absent() {
        let json = json!({
            "Entered": {
                "message": "Entered plan mode.",
                "plan_file_path": "/tmp/plan.md"
            }
        });
        let deserialized: EnterPlanModeOutput = serde_json::from_value(json).unwrap();
        match deserialized {
            EnterPlanModeOutput::Entered {
                tool_hints,
                plan_file_seed,
                ..
            } => {
                assert_eq!(tool_hints.ask_user, "ask_user_question");
                assert_eq!(tool_hints.exit_plan, "exit_plan_mode");
                assert!(tool_hints.task.is_empty());
                assert_eq!(
                    plan_file_seed,
                    PlanFileSeedStatus::Missing(PlanFileSeedFailure::NotCreated)
                );
            }
        }
    }
    #[test]
    fn enter_plan_mode_prompt_format_nonempty_seed() {
        let output = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints::default(),
            plan_file_seed: PlanFileSeedStatus::NonEmpty,
        });
        let empty = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints::default(),
            plan_file_seed: PlanFileSeedStatus::Empty,
        })
        .to_prompt_format();
        let prompt = output.to_prompt_format();
        assert!(prompt.contains("/tmp/plan.md"));
        assert_ne!(prompt, empty, "non-empty seed must change compiled status");
    }
    #[test]
    fn enter_plan_mode_prompt_format_missing_seed() {
        let output = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints::default(),
            plan_file_seed: PlanFileSeedStatus::Missing(PlanFileSeedFailure::NotCreated),
        });
        let empty = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints::default(),
            plan_file_seed: PlanFileSeedStatus::Empty,
        })
        .to_prompt_format();
        let prompt = output.to_prompt_format();
        assert!(prompt.contains("/tmp/plan.md"));
        assert_ne!(prompt, empty, "missing seed must change compiled status");
    }
    #[test]
    fn enter_plan_mode_absent_seed_field_prompt_is_missing() {
        let json = json!({
            "Entered": {
                "message": "Entered plan mode.",
                "plan_file_path": "/tmp/plan.md"
            }
        });
        let deserialized: EnterPlanModeOutput = serde_json::from_value(json).unwrap();
        let prompt = ToolOutput::EnterPlanMode(deserialized).to_prompt_format();
        let empty = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
            message: "Entered plan mode.".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints::default(),
            plan_file_seed: PlanFileSeedStatus::Empty,
        })
        .to_prompt_format();
        assert!(prompt.contains("/tmp/plan.md"));
        assert_ne!(
            prompt, empty,
            "absent seed field must compile as missing, not empty"
        );
    }
    #[test]
    fn enter_plan_mode_missing_reason_suffixes() {
        let cases = [
            PlanFileSeedFailure::NotCreated,
            PlanFileSeedFailure::NotAFile,
            PlanFileSeedFailure::Inaccessible,
            PlanFileSeedFailure::Unavailable,
        ];
        let mut compiled = Vec::new();
        for reason in cases {
            let prompt = ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
                message: "entered-msg-token".into(),
                plan_file_path: "/tmp/plan.md".into(),
                tool_hints: EnterPlanModeToolHints::default(),
                plan_file_seed: PlanFileSeedStatus::Missing(reason),
            })
            .to_prompt_format();
            assert!(prompt.contains("entered-msg-token"), "reason {reason:?}");
            assert!(
                prompt.contains("/tmp/plan.md"),
                "reason {reason:?}: {prompt}"
            );
            compiled.push(prompt);
        }
        compiled.sort();
        compiled.dedup();
        assert_eq!(
            compiled.len(),
            cases.len(),
            "each missing-seed reason must compile distinctly"
        );
    }
    #[test]
    fn plan_file_seed_missing_serde_shape() {
        let output = EnterPlanModeOutput::Entered {
            message: "m".into(),
            plan_file_path: "/tmp/plan.md".into(),
            tool_hints: EnterPlanModeToolHints::default(),
            plan_file_seed: PlanFileSeedStatus::Missing(PlanFileSeedFailure::NotAFile),
        };
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json["Entered"]["plan_file_seed"],
            json!({ "missing": "not_a_file" })
        );
        let back: EnterPlanModeOutput = serde_json::from_value(json).unwrap();
        let EnterPlanModeOutput::Entered { plan_file_seed, .. } = back;
        assert_eq!(
            plan_file_seed,
            PlanFileSeedStatus::Missing(PlanFileSeedFailure::NotAFile)
        );
    }
    #[test]
    fn subagent_completed_hints_absent_when_no_persona() {
        let output = SubagentCompletedOutput {
            output: "done".into(),
            subagent_id: "sub-xyz".into(),
            subagent_type: "explore".into(),
            tool_calls: 1,
            turns: 1,
            duration_ms: 500,
            worktree_path: None,
            persona: None,
            resume_from_hint: "sub-xyz".into(),
            persona_hint: None,
        };
        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["resume_from_hint"], "sub-xyz");
        assert!(
            json.get("persona_hint").is_none(),
            "persona_hint should be absent when None"
        );
    }
    fn make_pdf_page_images(
        page_numbers: &[usize],
        total_pages: usize,
        file_size: usize,
    ) -> PdfPageImages {
        PdfPageImages {
            pages: page_numbers
                .iter()
                .map(|&n| PdfPageImage {
                    data: format!("base64data_page{n}"),
                    mime_type: "image/jpeg".to_string(),
                    page_number: n,
                })
                .collect(),
            total_pages,
            file_size,
        }
    }
    #[test]
    fn pdf_page_images_to_prompt_format() {
        let pdf = make_pdf_page_images(&[1, 2, 3], 25, 102400);
        let output = ToolOutput::ReadFile(ReadFileOutput::PdfPageImages(pdf));
        let text = output.to_prompt_format();
        assert!(text.contains("3 pages rendered"), "got: {text}");
        assert!(text.contains("pages 1, 2, 3"), "got: {text}");
        assert!(text.contains("25 pages"), "got: {text}");
        assert!(text.contains("100.0 KB"), "got: {text}");
    }
    #[test]
    fn pdf_page_images_to_prompt_format_single_page() {
        let pdf = make_pdf_page_images(&[5], 10, 51200);
        let output = ToolOutput::ReadFile(ReadFileOutput::PdfPageImages(pdf));
        let text = output.to_prompt_format();
        assert!(text.contains("1 pages rendered"), "got: {text}");
        assert!(text.contains("pages 5"), "got: {text}");
        assert!(text.contains("10 pages"), "got: {text}");
        assert!(text.contains("50.0 KB"), "got: {text}");
    }
    #[test]
    fn pdf_page_images_json_round_trip() {
        let pdf = make_pdf_page_images(&[1, 3], 20, 8192);
        let output = ToolOutput::ReadFile(ReadFileOutput::PdfPageImages(pdf));
        let json = to_json(output);
        assert_eq!(json["type"], "ReadFile");
        assert!(
            json.get("PdfPageImages").is_some(),
            "missing PdfPageImages key: {json}"
        );
        let inner = &json["PdfPageImages"];
        assert_eq!(inner["total_pages"], 20);
        assert_eq!(inner["file_size"], 8192);
        assert_eq!(inner["pages"].as_array().unwrap().len(), 2);
        assert_eq!(inner["pages"][0]["page_number"], 1);
        assert_eq!(inner["pages"][1]["page_number"], 3);
    }
    fn sample_bash(exit_code: i32, output: &[u8], timed_out: bool) -> BashOutput {
        BashOutput {
            output: output.to_vec(),
            output_for_prompt: String::new(),
            exit_code,
            command: "cmd".into(),
            truncated: false,
            signal: None,
            timed_out,
            description: None,
            current_dir: "/tmp".into(),
            output_file: String::new(),
            total_bytes: output.len(),
            output_delta: None,
            was_bare_echo: false,
        }
    }
    fn assert_cer(
        resp: &xai_tool_runtime::ToolChatCompletionResponse,
        stdout: &str,
        exit_code: i32,
        timed_out: bool,
    ) {
        let result = resp.result.as_ref().unwrap();
        let cer = result.code_execution_result.as_ref().unwrap();
        assert_eq!(cer.stdout, stdout);
        assert!(cer.stderr.is_empty());
        assert_eq!(cer.exit_code, exit_code);
        assert_eq!(cer.command_timed_out, timed_out);
        assert_eq!(result.sender, "assistant");
        assert_eq!(result.message_tag.as_deref(), Some("raw_function_result"));
    }
    fn bg_started() -> BackgroundTaskStarted {
        BackgroundTaskStarted {
            task_id: "t1".into(),
            task_type: "bash".into(),
            output_file: "/tmp/out".into(),
            status: "running".into(),
            command: "sleep 99".into(),
            summary: "running".into(),
            retrieval_hint: String::new(),
            pre_formatted: None,
            pid: None,
        }
    }
    #[test]
    fn bash_output_chat_completion_carries_exit_and_stdout() {
        let resp = xai_tool_runtime::ToolOutput::chat_completion_output(&sample_bash(
            0, b"hello\n", false,
        ))
        .unwrap();
        assert_cer(&resp, "hello\n", 0, false);
        assert!(resp.result.as_ref().unwrap().extra.is_empty());
    }
    #[test]
    fn bash_output_chat_completion_empty_stdout_still_emits() {
        let resp =
            xai_tool_runtime::ToolOutput::chat_completion_output(&sample_bash(0, b"", false))
                .unwrap();
        assert_cer(&resp, "", 0, false);
    }
    #[test]
    fn bash_output_chat_completion_timeout_and_nonzero_exit() {
        let resp = xai_tool_runtime::ToolOutput::chat_completion_output(&sample_bash(
            124, b"partial", true,
        ))
        .unwrap();
        assert_cer(&resp, "partial", 124, true);
    }
    #[test]
    fn bash_output_chat_completion_lossy_utf8() {
        let resp = xai_tool_runtime::ToolOutput::chat_completion_output(&sample_bash(
            1,
            &[0x66, 0x6f, 0x6f, 0xff, 0x62, 0x61, 0x72],
            false,
        ))
        .unwrap();
        let stdout = &resp
            .result
            .as_ref()
            .unwrap()
            .code_execution_result
            .as_ref()
            .unwrap()
            .stdout;
        assert!(stdout.starts_with("foo"));
        assert!(stdout.ends_with("bar"));
        assert!(stdout.contains('\u{FFFD}'));
    }
    #[test]
    fn bash_output_chat_completion_truncation_marker_and_extra() {
        let mut bash = sample_bash(0, b"head...tail", false);
        bash.truncated = true;
        bash.total_bytes = 50_000;
        bash.output_file = "/tmp/out.log".into();
        let resp = xai_tool_runtime::ToolOutput::chat_completion_output(&bash).unwrap();
        let result = resp.result.as_ref().unwrap();
        let stdout = &result.code_execution_result.as_ref().unwrap().stdout;
        assert!(stdout.starts_with("head...tail"));
        assert!(stdout.contains("[truncated:"));
        assert!(stdout.contains("full output at: /tmp/out.log"));
        assert_eq!(
            result.extra.get("truncated"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            result.extra.get("total_bytes"),
            Some(&serde_json::Value::from(50_000u64))
        );
        assert_eq!(
            result.extra.get("output_file"),
            Some(&serde_json::Value::String("/tmp/out.log".into()))
        );
        assert!(
            result
                .code_execution_result
                .as_ref()
                .unwrap()
                .stderr
                .is_empty()
        );
    }
    #[test]
    fn bash_tool_output_foreground_delegates_background_skips() {
        let resp = xai_tool_runtime::ToolOutput::chat_completion_output(&BashToolOutput::Bash(
            sample_bash(0, b"ok", false),
        ))
        .unwrap();
        assert_cer(&resp, "ok", 0, false);
        assert!(
            xai_tool_runtime::ToolOutput::chat_completion_output(
                &BashToolOutput::BackgroundTaskStarted(bg_started())
            )
            .is_none()
        );
    }
    #[test]
    fn aggregate_tool_output_bash_delegates_background_skips() {
        let resp = xai_tool_runtime::ToolOutput::chat_completion_output(&ToolOutput::Bash(
            sample_bash(0, b"agg", false),
        ))
        .unwrap();
        assert_cer(&resp, "agg", 0, false);
        assert!(
            xai_tool_runtime::ToolOutput::chat_completion_output(
                &ToolOutput::BackgroundTaskStarted(bg_started())
            )
            .is_none()
        );
        assert!(
            xai_tool_runtime::ToolOutput::chat_completion_output(&ToolOutput::Text(
                TextOutput::from("noop")
            ))
            .is_none()
        );
    }
    fn sample_edits_applied(old_lines: &[usize]) -> SearchReplaceEditsApplied {
        sample_edits_applied_for("b", old_lines)
    }
    fn sample_edits_applied_for(
        old_string: &str,
        old_lines: &[usize],
    ) -> SearchReplaceEditsApplied {
        SearchReplaceEditsApplied {
            old_string: old_string.into(),
            new_string: "z".into(),
            tool_output_for_prompt: "edited".into(),
            tool_output_for_prompt_concise: None,
            absolute_path: PathBuf::from("/w/a.rs"),
            edits: SearchReplaceEditContextInformation {
                details: old_lines
                    .iter()
                    .map(|&old_line| SearchReplaceEditDetail {
                        old_string: old_string.into(),
                        old_line,
                        new_string: "z".into(),
                        new_line: old_line,
                        context_before: String::new(),
                        context_after: String::new(),
                        line_prefix: String::new(),
                    })
                    .collect(),
            },
            patch: None,
            unicode_normalized: false,
        }
    }
    /// Every applied edit settles the card through the empty success shell;
    /// only a single-match edit of an existing file also carries the anchor.
    fn assert_applied_edit_frame(
        resp: xai_tool_runtime::ToolChatCompletionResponse,
        anchor: Option<xai_tool_runtime::EditFileAnchor>,
    ) {
        let result = resp.result.unwrap();
        assert_eq!(result.message_tag.as_deref(), Some("raw_function_result"));
        assert_eq!(result.edit_file_result, anchor);
        let cer = result.code_execution_result.expect("settle shell");
        assert_eq!(
            (
                cer.stdout.as_str(),
                cer.stderr.as_str(),
                cer.exit_code,
                cer.command_timed_out
            ),
            ("", "", 0, false)
        );
        assert!(result.extra.is_empty());
    }
    #[test]
    fn search_replace_chat_completion_anchors_single_edit() {
        let anchor = Some(xai_tool_runtime::EditFileAnchor { start_line: 137 });
        let applied = SearchReplaceOutput::EditsApplied(sample_edits_applied(&[137]));
        assert_applied_edit_frame(
            xai_tool_runtime::ToolOutput::chat_completion_output(&applied).unwrap(),
            anchor,
        );
        assert_applied_edit_frame(
            xai_tool_runtime::ToolOutput::chat_completion_output(&ToolOutput::SearchReplace(
                applied,
            ))
            .unwrap(),
            anchor,
        );
    }
    #[test]
    fn search_replace_chat_completion_settles_multi_match_and_creation_without_anchor() {
        assert_applied_edit_frame(
            xai_tool_runtime::ToolOutput::chat_completion_output(&ToolOutput::SearchReplace(
                SearchReplaceOutput::EditsApplied(sample_edits_applied(&[2, 9])),
            ))
            .unwrap(),
            None,
        );
        assert_applied_edit_frame(
            xai_tool_runtime::ToolOutput::chat_completion_output(
                &SearchReplaceOutput::EditsApplied(sample_edits_applied_for("", &[1])),
            )
            .unwrap(),
            None,
        );
    }
    #[test]
    fn search_replace_chat_completion_skips_failures() {
        assert!(
            xai_tool_runtime::ToolOutput::chat_completion_output(&ToolOutput::SearchReplace(
                SearchReplaceOutput::MultipleMatchesFound("two".into())
            ))
            .is_none()
        );
    }
    fn sample_run_result(output: ToolOutput) -> ToolRunResult {
        ToolRunResult {
            prompt_text: "prompt".into(),
            effective_tool_name: None,
            output,
        }
    }
    fn bash_tool_id() -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("bash").unwrap()
    }
    #[test]
    fn into_typed_tool_output_preserves_bash_foreground_cco() {
        let run = sample_run_result(ToolOutput::Bash(sample_bash(0, b"hello-cco", false)));
        let expected_value = serde_json::to_value(&run).unwrap();
        let typed = run.into_typed_tool_output(bash_tool_id());
        assert_eq!(typed.value, expected_value);
        assert_eq!(typed.tool_id, bash_tool_id());
        assert_cer(
            typed
                .chat_completion_output
                .as_ref()
                .expect("bash foreground must preserve chat_completion_output"),
            "hello-cco",
            0,
            false,
        );
        let dropped = xai_tool_runtime::TypedToolOutput::from_value(typed.tool_id, expected_value);
        assert!(dropped.chat_completion_output.is_none());
    }
    #[test]
    fn into_typed_tool_output_non_bash_cco_is_none() {
        let run = sample_run_result(ToolOutput::Text(TextOutput::from("noop")));
        let typed = run.into_typed_tool_output(xai_tool_protocol::ToolId::new("text").unwrap());
        assert!(typed.chat_completion_output.is_none());
    }
    #[test]
    fn into_typed_tool_output_background_task_cco_is_none() {
        let run = sample_run_result(ToolOutput::BackgroundTaskStarted(bg_started()));
        let typed = run.into_typed_tool_output(bash_tool_id());
        assert!(typed.chat_completion_output.is_none());
    }
    #[test]
    fn typed_tool_output_preserving_cco_serialize_failure_yields_null_value() {
        struct AlwaysFailSerialize;
        impl Serialize for AlwaysFailSerialize {
            fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("intentional serialize failure"))
            }
        }
        let bash = ToolOutput::Bash(sample_bash(0, b"kept-cco", false));
        let typed = typed_tool_output_preserving_cco(bash_tool_id(), &AlwaysFailSerialize, &bash);
        assert_eq!(typed.value, serde_json::Value::Null);
        assert_cer(
            typed
                .chat_completion_output
                .as_ref()
                .expect("CCO still attached when payload serialize fails"),
            "kept-cco",
            0,
            false,
        );
    }
}
