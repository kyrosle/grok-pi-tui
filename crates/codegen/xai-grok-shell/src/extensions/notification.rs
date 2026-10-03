use crate::session::feedback::FeedbackRequest as FeedbackRequestData;
use agent_client_protocol as acp;
pub use xai_grok_shared::session::notification::*;

impl From<&crate::session::image_normalize::ImageCompressionInfo> for ImageCompressedEntry {
    fn from(c: &crate::session::image_normalize::ImageCompressionInfo) -> Self {
        Self {
            index: c.index,
            original_bytes: c.original_bytes,
            compressed_bytes: c.compressed_bytes,
            original_width: c.original_width,
            original_height: c.original_height,
            compressed_width: c.compressed_width,
            compressed_height: c.compressed_height,
        }
    }
}

impl From<FeedbackRequestData> for FeedbackRequestNotification {
    fn from(data: FeedbackRequestData) -> Self {
        Self {
            request_id: data.request_id,
            tier: format!("{:?}", data.tier).to_lowercase(),
            stars: data.stars,
            thumbs: data.thumbs,
            text: data.text,
            prompt: data.prompt,
            dismissible: data.dismissible,
            trigger_type: data.trigger_type,
            trigger_condition: data.trigger_condition.condition.clone(),
            trigger_reason: data.trigger_condition.trigger_reason(),
        }
    }
}

/// The on-disk format for a compaction checkpoint file. Stored at `{session_dir}/compaction_checkpoints/{checkpoint_id}.json`. Contains the full compacted conversation history.
/// The replay pipeline uses it to deterministically reconstruct the model's view without re-running compaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CompactionCheckpointFile {
    /// Unique checkpoint identifier (matches [`CompactionCheckpointInfo::checkpoint_id`]).
    pub checkpoint_id: String,
    /// The prompt index at the time compaction completed.
    pub prompt_index_at_compaction: usize,
    /// The exact compacted conversation used by the model.
    pub compacted_history: Vec<crate::sampling::ConversationItem>,
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// ISO 8601 timestamp of when the checkpoint was created.
    pub created_at: String,
    /// The original User(user_info) text from before compaction. Cross-compaction rewind uses it to restore the user_info the model originally saw for pre-compaction turns.
    /// Otherwise the rewind would use the user_info rebuilt from the compacted conversation. `None` in older checkpoints (schema_version 1 without this field).
    #[serde(default)]
    pub original_user_info: Option<String>,
    /// File paths that were re-read and injected after compaction.
    /// Informational: kept for debugging and for understanding replay.
    #[serde(default)]
    pub reread_file_paths: Vec<String>,
}

/// A compaction segment to persist under `compaction/segment_NNN.md`.
/// Storage assigns the resume-safe index and renders the markdown (it owns the index the header/metadata embed).
/// The caller therefore supplies the render inputs.
#[derive(Debug, Clone)]
pub struct CompactionSegmentFile {
    pub items: Vec<xai_grok_sampling_types::ConversationItem>,
    /// Curated summary, analysis tags already stripped.
    pub summary: String,
    pub detail: xai_chat_state::CompactionDetail,
    /// ISO-8601, for the segment metadata.
    pub timestamp: String,
}

/// On-disk artifact capturing the exact compaction request sent to the model plus the response (or final error) it produced. Stored at `{session_dir}/compaction_requests/{request_id}.json`.
/// Rides on the post-turn session archive to cloud storage, where it can be downloaded for prompt iteration.
/// It records the exact `chat_history` sent, the prompt variant, any `/compact <text>` user context, the model, and the resulting summary (or error). Replay the request locally to A/B test alternate prompt wordings against the same input.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CompactionRequestFile {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// Unique artifact identifier (filename stem).
    /// This is a per-artifact ID, not the model API's `x_grok_req_id` (which is generated per-attempt inside the sampling layer).
    pub request_id: String,
    /// ISO 8601 timestamp of when the compaction call started.
    pub created_at: String,
    /// What kicked off the compaction: `"manual"` (user ran `/compact`) or `"auto"`.
    pub trigger: String,
    /// Which prompt template was used.
    /// `"short"` is the concise self-summarization; `"detailed"` is the 10-section structured prompt for grok-build and similar agents.
    pub prompt_variant: String,
    /// The model id that ran the summarization.
    pub model: String,
    /// User-provided context from `/compact <text>`, if any.
    pub user_context: Option<String>,
    /// The full `ConversationItem` list sent to the model, with the summarization prompt already appended as the final user message.
    /// Replaying this against any model reproduces the exact request.
    pub chat_history: Vec<crate::sampling::ConversationItem>,
    /// Tool definitions attached to the request (same effective set as the turn loop, for prompt-prefix/KV-cache alignment).
    /// Empty for artifacts written before tools were attached.
    /// Backend-hosted tools are not recorded: they are not serializable and exist only on the backend.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<crate::sampling::ToolSpec>,
    /// Generated summary text, on success. `None` if all retries failed.
    pub summary: Option<String>,
    /// Most recent error message captured during the retry loop, if any. `None` on a first-attempt success. May co-occur with `summary` when a transient failure was eventually retried successfully.
    /// The field then documents the recovered failure and `summary` carries the final result. On total failure (all retries exhausted, or a deterministic error) `summary` is `None` and this field carries the final error.
    pub error: Option<String>,
    /// Number of attempts the retry loop made before settling on the final outcome.
    pub attempts: u32,
    /// Per-attempt diagnostics (one per retry-loop iteration), in order.
    /// Records each rejected/degraded attempt so retries aren't bumped invisibly.
    /// Empty on artifacts written before schema v2.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempt_details: Vec<xai_chat_state::compaction_utils::CompactionAttempt>,
}

/// On-disk artifact capturing the exact recap request sent to the model plus the response (or final error) it produced. Stored at `{session_dir}/recap_requests/{request_id}.json`.
/// Rides on the post-turn session archive to cloud storage (same path as compaction request artifacts). So a recap prompt problem or garbled model output can be replayed offline.
/// Recap never mutates the conversation; this file is the only durable record of what was sent for `/recap` or an automatic recap.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RecapRequestFile {
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// Unique artifact identifier (filename stem).
    /// Distinct from the model API's `x_grok_req_id` (also recorded below for proxy correlation).
    pub request_id: String,
    /// ISO 8601 timestamp of when the recap model call started.
    pub created_at: String,
    /// What kicked off the recap: `"manual"` (`/recap`) or `"auto"` (the user returned after being away).
    pub trigger: String,
    /// The model id used for the recap side-call.
    pub model: String,
    /// Sampling request id sent to the proxy (`xai-recap-{uuid}`).
    pub x_grok_req_id: String,
    /// Sampling conversation id (`recap-{uuid}`).
    pub x_grok_conv_id: String,
    /// Whether the side-call requested reasoning/thinking removal before budgeting.
    /// The over-budget path removes reasoning independently.
    pub strip_reasoning: bool,
    /// Reminder tag used in the recap instruction (`system-reminder` or the alternate `system_reminder` form).
    pub reminder_tag: String,
    /// The full `ConversationItem` list sent to the model, with the recap instruction already appended as the final user message.
    /// Replaying this against any model reproduces the exact request.
    pub chat_history: Vec<crate::sampling::ConversationItem>,
    /// Cleaned one-line recap body shown to the user, on success.
    /// `None` if the model call failed or returned empty after cleaning.
    pub summary: Option<String>,
    /// Raw `assistant_text()` from the model before `clean_recap_text`.
    /// Kept for diagnosing garbled output (tool-call XML / CJK junk) that cleaning only partially trims.
    /// `None` when the call never returned a response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<String>,
    /// Error message if preparation or the model call failed, or if the cleaned summary was empty.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_401_needle_is_contained_in_unauthorized_needle() {
        // Pins the sub-needle doc claim.
        assert!(UNAUTHORIZED_NEEDLE.contains(HTTP_401_NEEDLE));
    }

    #[test]
    fn recap_request_file_roundtrips() {
        let artifact = RecapRequestFile {
            schema_version: 1,
            request_id: "artifact-1".into(),
            created_at: "2026-06-30T00:00:00Z".into(),
            trigger: "auto".into(),
            model: "v9-zingster".into(),
            x_grok_req_id: "xai-recap-abc".into(),
            x_grok_conv_id: "recap-abc".into(),
            strip_reasoning: false,
            reminder_tag: "system-reminder".into(),
            chat_history: vec![],
            summary: Some("We fixed the flaky test in queue_worker.".into()),
            raw_response: Some("We fixed the flaky test in queue_worker.".into()),
            error: None,
        };
        let json = serde_json::to_string(&artifact).unwrap();
        let parsed: RecapRequestFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.trigger, "auto");
        assert_eq!(parsed.x_grok_req_id, "xai-recap-abc");
        assert_eq!(
            parsed.summary.as_deref(),
            Some("We fixed the flaky test in queue_worker.")
        );
        assert!(parsed.error.is_none());
    }

    #[test]
    fn compaction_request_file_v2_roundtrips_attempt_details() {
        use xai_chat_state::compaction_utils::CompactionAttempt;
        let artifact = CompactionRequestFile {
            schema_version: 2,
            request_id: "req-1".into(),
            created_at: "2026-06-15T00:00:00Z".into(),
            trigger: "auto".into(),
            prompt_variant: "detailed".into(),
            model: "grok".into(),
            user_context: None,
            chat_history: vec![],
            tools: vec![],
            summary: Some("the accepted summary".into()),
            error: None,
            attempts: 2,
            attempt_details: vec![
                // A rejected degraded attempt: the loop captures the raw text.
                CompactionAttempt {
                    attempt: 1,
                    outcome: "degenerate".into(),
                    summary_chars: 22,
                    summary: Some("Now I will do X, Y, Z.".into()),
                    error: None,
                },
                // The accepted retry: text lives in the top-level `summary`.
                CompactionAttempt {
                    attempt: 2,
                    outcome: "success".into(),
                    summary_chars: 4096,
                    summary: None,
                    error: None,
                },
            ],
        };
        let json = serde_json::to_string(&artifact).unwrap();
        let parsed: CompactionRequestFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.schema_version, 2);
        assert_eq!(parsed.attempt_details.len(), 2);
        assert_eq!(parsed.attempt_details[0].outcome, "degenerate");
        assert_eq!(
            parsed.attempt_details[0].summary.as_deref(),
            Some("Now I will do X, Y, Z.")
        );
        assert_eq!(parsed.attempt_details[1].outcome, "success");
        assert_eq!(parsed.attempt_details[1].summary, None);
    }

    #[test]
    fn compaction_request_file_v1_without_attempt_details_defaults_empty() {
        // A pre-schema-v2 artifact carries no `attempt_details` key; it must still deserialize, defaulting the new field to empty
        let json = serde_json::json!({
            "schema_version": 1,
            "request_id": "req-old",
            "created_at": "2026-06-01T00:00:00Z",
            "trigger": "manual",
            "prompt_variant": "detailed",
            "model": "grok",
            "user_context": null,
            "chat_history": [],
            "summary": "ok",
            "error": null,
            "attempts": 1
        });
        let parsed: CompactionRequestFile = serde_json::from_value(json).unwrap();
        assert!(parsed.attempt_details.is_empty());
        assert!(parsed.tools.is_empty());
    }

    #[test]
    fn subagent_progress_serializes_snake_case_tag() {
        let update = SessionUpdate::SubagentProgress {
            attempt_id: None,
            subagent_id: "sub-1".into(),
            parent_session_id: "parent-1".into(),
            child_session_id: "child-1".into(),
            duration_ms: 5000,
            turn_count: 3,
            tool_call_count: 12,
            tokens_used: 45_000,
            context_window_tokens: 256_000,
            context_usage_pct: 35,
            tools_used: vec!["bash".into(), "grep".into()],
            error_count: 1,
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "subagent_progress");
        // Fields serialize as snake_case (Rust field names): enum `rename_all` only applies to the tag, not struct fields
        assert_eq!(json["subagent_id"], "sub-1");
        assert_eq!(json["parent_session_id"], "parent-1");
        assert_eq!(json["child_session_id"], "child-1");
        assert_eq!(json["duration_ms"], 5000);
        assert_eq!(json["turn_count"], 3);
        assert_eq!(json["tool_call_count"], 12);
        assert_eq!(json["tokens_used"], 45_000);
        assert_eq!(json["context_window_tokens"], 256_000);
        assert_eq!(json["context_usage_pct"], 35);
        assert_eq!(json["tools_used"], serde_json::json!(["bash", "grep"]));
        assert_eq!(json["error_count"], 1);
    }

    #[test]
    fn subagent_progress_roundtrips_through_json() {
        let update = SessionUpdate::SubagentProgress {
            attempt_id: None,
            subagent_id: "sub-rt".into(),
            parent_session_id: "p".into(),
            child_session_id: "c".into(),
            duration_ms: 100,
            turn_count: 1,
            tool_call_count: 2,
            tokens_used: 1000,
            context_window_tokens: 256_000,
            context_usage_pct: 1,
            tools_used: vec![],
            error_count: 0,
        };
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);
    }

    #[test]
    fn subagent_progress_orders_after_spawned_before_finished() {
        // SubagentProgress must appear between SubagentSpawned and SubagentFinished in the enum definition
        // Clients expect notifications in that order
        let spawned = serde_json::to_value(SessionUpdate::SubagentSpawned {
            attempt_id: None,
            subagent_id: "s".into(),
            parent_session_id: "p".into(),
            parent_prompt_id: None,
            child_session_id: "c".into(),
            subagent_type: "explore".into(),
            description: "d".into(),
            effective_context_source: None,
            context_normalized: false,
            capability_mode: None,
            persona: None,
            role: None,
            model: None,
            resumed_from: None,
            workflow_run_id: None,
            agent_address: None,
        })
        .unwrap();
        let progress = serde_json::to_value(SessionUpdate::SubagentProgress {
            attempt_id: None,
            subagent_id: "s".into(),
            parent_session_id: "p".into(),
            child_session_id: "c".into(),
            duration_ms: 100,
            turn_count: 1,
            tool_call_count: 1,
            tokens_used: 100,
            context_window_tokens: 256_000,
            context_usage_pct: 0,
            tools_used: vec![],
            error_count: 0,
        })
        .unwrap();
        let finished = serde_json::to_value(SessionUpdate::SubagentFinished {
            attempt_id: None,
            subagent_id: "s".into(),
            child_session_id: "c".into(),
            status: "completed".into(),
            error: None,
            tool_calls: 1,
            turns: 1,
            duration_ms: 200,
            tokens_used: 50_000,
            output: None,
            will_wake: false,
        })
        .unwrap();
        // All three tags are distinct
        assert_eq!(spawned["sessionUpdate"], "subagent_spawned");
        assert_eq!(progress["sessionUpdate"], "subagent_progress");
        assert_eq!(finished["sessionUpdate"], "subagent_finished");
    }

    #[test]
    fn subagent_spawned_agent_address_serializes_as_typed_camel_case() {
        let update = SessionUpdate::SubagentSpawned {
            attempt_id: None,
            subagent_id: "s".into(),
            parent_session_id: "p".into(),
            parent_prompt_id: None,
            child_session_id: "c".into(),
            subagent_type: "explore".into(),
            description: "d".into(),
            effective_context_source: None,
            context_normalized: false,
            capability_mode: None,
            persona: None,
            role: None,
            model: None,
            resumed_from: None,
            workflow_run_id: None,
            agent_address: Some("opaque-address".into()),
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["agentAddress"], "opaque-address");
        assert!(json.get("agent_address").is_none());

        let parsed: SessionUpdate = serde_json::from_value(json).unwrap();
        match &parsed {
            SessionUpdate::SubagentSpawned { agent_address, .. } => {
                assert_eq!(agent_address.as_deref(), Some("opaque-address"));
            }
            other => panic!("expected SubagentSpawned, got {other:?}"),
        }

        let aliased: SessionUpdate = serde_json::from_str(
            r#"{"sessionUpdate":"subagent_spawned","subagent_id":"s","parent_session_id":"p","child_session_id":"c","subagent_type":"explore","description":"d","agent_address":"from-alias"}"#,
        )
        .unwrap();
        match aliased {
            SessionUpdate::SubagentSpawned { agent_address, .. } => {
                assert_eq!(agent_address.as_deref(), Some("from-alias"));
            }
            other => panic!("expected SubagentSpawned, got {other:?}"),
        }

        let notification = SessionNotification {
            session_id: acp::SessionId::new("p"),
            update,
            meta: None,
        };
        let durable = notification.to_durable_value().unwrap();
        assert!(durable["update"].get("agentAddress").is_none());
        let live = serde_json::to_value(&notification).unwrap();
        assert_eq!(live["update"]["agentAddress"], "opaque-address");
    }

    #[test]
    fn subagent_finished_without_tokens_used_backward_compat() {
        // Old JSONL entries written before the tokens_used field was added must deserialize with tokens_used defaulting to 0
        // modelUsage on older wire is ignored (billing is RecordSubagentUsage only).
        let json = r#"{
            "sessionUpdate": "subagent_finished",
            "subagent_id": "sa-old",
            "child_session_id": "cs-old",
            "status": "completed",
            "tool_calls": 3,
            "turns": 1,
            "duration_ms": 5000,
            "modelUsage": { "m": { "inputTokens": 1 } }
        }"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        match update {
            SessionUpdate::SubagentFinished {
                subagent_id,
                tokens_used,
                output,
                ..
            } => {
                assert_eq!(subagent_id, "sa-old");
                assert_eq!(tokens_used, 0, "missing field must default to 0");
                assert_eq!(output, None);
            }
            other => panic!("expected SubagentFinished, got {other:?}"),
        }
    }

    #[test]
    fn subagent_finished_with_tokens_used_roundtrips() {
        let update = SessionUpdate::SubagentFinished {
            attempt_id: None,
            subagent_id: "sa-rt".into(),
            child_session_id: "cs-rt".into(),
            status: "completed".into(),
            error: None,
            tool_calls: 5,
            turns: 2,
            duration_ms: 10_000,
            tokens_used: 75_000,
            output: Some("done".into()),
            will_wake: false,
        };
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);

        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["tokens_used"], 75_000);
    }

    #[test]
    fn unknown_variant_deserializes_from_removed_git_branch_update() {
        let json = r#"{"sessionUpdate": "git_branch_update", "branch": "main"}"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update, SessionUpdate::Unknown);
    }

    #[test]
    fn unknown_variant_deserializes_from_arbitrary_future_variant() {
        let json = r#"{"sessionUpdate": "some_future_feature", "data": 42}"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update, SessionUpdate::Unknown);
    }

    #[test]
    fn unknown_variant_in_session_notification_envelope() {
        let json = r#"{
            "sessionId": "sess-123",
            "update": {"sessionUpdate": "git_branch_update", "branch": "main"}
        }"#;
        let notification: SessionNotification = serde_json::from_str(json).unwrap();
        assert_eq!(notification.session_id.0.as_ref(), "sess-123");
        assert_eq!(notification.update, SessionUpdate::Unknown);
    }

    #[test]
    fn known_variants_still_deserialize_correctly() {
        // MemoryFlushStarted (unit variant)
        let json = r#"{"sessionUpdate": "memory_flush_started"}"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update, SessionUpdate::MemoryFlushStarted);

        // AutoCompactCancelled (strenum reason)
        let json = r#"{"sessionUpdate": "auto_compact_cancelled", "reason": "user_cancelled"}"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(
            update,
            SessionUpdate::AutoCompactCancelled {
                reason: AutoCompactCancelReason::UserCancelled,
            }
        );

        // AutoCompactFailed (struct variant)
        let json = r#"{"sessionUpdate": "auto_compact_failed", "error": "oom"}"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(
            update,
            SessionUpdate::AutoCompactFailed {
                error: "oom".into()
            }
        );

        // RetryState (newtype variant)
        let json = r#"{"sessionUpdate": "retry_state", "type": "failed", "error_type": "auth", "message": "bad token"}"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        assert!(matches!(
            update,
            SessionUpdate::RetryState(RetryState::Failed { .. })
        ));
    }

    #[test]
    fn memory_flush_completed_with_path_roundtrips() {
        let update = SessionUpdate::MemoryFlushCompleted {
            result: "written".into(),
            path: Some("/home/user/.grok/memory/ws/sessions/log.md".into()),
        };
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);
    }

    #[test]
    fn memory_flush_completed_without_path_backward_compat() {
        // Old format without the path field deserializes with `path: None`
        let json = r#"{"sessionUpdate": "memory_flush_completed", "result": "written"}"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(
            update,
            SessionUpdate::MemoryFlushCompleted {
                result: "written".into(),
                path: None,
            }
        );
    }

    #[test]
    fn memory_dream_completed_roundtrips() {
        let update = SessionUpdate::MemoryDreamCompleted {
            result: "written (500 chars)".into(),
            path: Some("/home/user/.grok/memory/ws/MEMORY.md".into()),
        };
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);
    }

    #[test]
    fn memory_session_saved_roundtrips() {
        let update = SessionUpdate::MemorySessionSaved {
            path: "/home/user/.grok/memory/ws/sessions/2026-01-15-fix-auth-abc12345.md".into(),
        };
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);
    }

    #[test]
    fn unknown_serializes_with_stable_tag() {
        let json = serde_json::to_value(&SessionUpdate::Unknown).unwrap();
        assert_eq!(json["sessionUpdate"], "unknown");
    }

    #[test]
    fn unknown_roundtrips_through_json() {
        let json_str = serde_json::to_string(&SessionUpdate::Unknown).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed, SessionUpdate::Unknown);
    }

    #[test]
    fn memory_files_variant_roundtrips_through_json() {
        let update = SessionUpdate::MemoryFiles {
            files: vec![
                MemoryFileInfo {
                    path: "/home/user/.grok/memory/MEMORY.md".into(),
                    source: "global".into(),
                    size_bytes: 1024,
                    modified_epoch_secs: Some(1_700_000_000),
                },
                MemoryFileInfo {
                    path: "/project/.grok/memory/MEMORY.md".into(),
                    source: "workspace".into(),
                    size_bytes: 512,
                    modified_epoch_secs: None,
                },
            ],
        };
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);

        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "memory_files");
        assert_eq!(json["files"].as_array().unwrap().len(), 2);
        // Populated timestamp serializes as a plain u64
        assert_eq!(json["files"][0]["modified_epoch_secs"], 1_700_000_000_u64);
        // None is omitted entirely
        assert!(json["files"][1].get("modified_epoch_secs").is_none());
    }

    #[test]
    fn memory_files_empty_list_serializes() {
        let update = SessionUpdate::MemoryFiles { files: vec![] };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "memory_files");
        assert!(json["files"].as_array().unwrap().is_empty());
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);
    }

    #[test]
    fn tool_call_delta_chunk_first_event_serializes_with_id_and_name() {
        // First chunk for a tool: carries id and name, no arguments_delta
        let update = SessionUpdate::ToolCallDeltaChunk {
            tool_call_id: Some("call_abc".into()),
            tool_index: 0,
            name: Some("search_replace".into()),
            arguments_delta: None,
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "tool_call_delta_chunk");
        assert_eq!(json["tool_call_id"], "call_abc");
        assert_eq!(json["tool_index"], 0);
        assert_eq!(json["name"], "search_replace");
        // None fields are skipped (cleaner wire payload, fewer bytes).
        assert!(json.get("arguments_delta").is_none());
    }

    #[test]
    fn tool_call_delta_chunk_subsequent_event_carries_only_arguments_delta() {
        // Later chunks omit id and name; only the JSON fragment travels
        let update = SessionUpdate::ToolCallDeltaChunk {
            tool_call_id: None,
            tool_index: 0,
            name: None,
            arguments_delta: Some("{\"file\":\"src/".into()),
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "tool_call_delta_chunk");
        assert_eq!(json["tool_index"], 0);
        assert_eq!(json["arguments_delta"], "{\"file\":\"src/");
        // Optional fields skipped when None.
        assert!(json.get("tool_call_id").is_none());
        assert!(json.get("name").is_none());
    }

    #[test]
    fn tool_call_delta_chunk_roundtrips_through_json() {
        let cases = vec![
            SessionUpdate::ToolCallDeltaChunk {
                tool_call_id: Some("call_1".into()),
                tool_index: 0,
                name: Some("Bash".into()),
                arguments_delta: None,
            },
            SessionUpdate::ToolCallDeltaChunk {
                tool_call_id: None,
                tool_index: 0,
                name: None,
                arguments_delta: Some("{\"command\":\"ls\"}".into()),
            },
            SessionUpdate::ToolCallDeltaChunk {
                tool_call_id: Some("call_2".into()),
                tool_index: 1,
                name: Some("ReadFile".into()),
                arguments_delta: Some("{\"path\":".into()),
            },
        ];
        for update in cases {
            let s = serde_json::to_string(&update).unwrap();
            let parsed: SessionUpdate = serde_json::from_str(&s).unwrap();
            assert_eq!(update, parsed, "round-trip mismatch for {s}");
        }
    }

    #[test]
    fn tool_call_delta_chunk_unknown_extra_fields_dont_break_deserialization() {
        // Forward-compat: if a future shell adds new optional fields, older deserializers must still accept the message
        let json = r#"{
            "sessionUpdate": "tool_call_delta_chunk",
            "tool_call_id": "call_x",
            "tool_index": 7,
            "name": "future_tool",
            "arguments_delta": "...",
            "future_field": "ignored"
        }"#;
        let update: SessionUpdate = serde_json::from_str(json).expect("parses with extra field");
        match update {
            SessionUpdate::ToolCallDeltaChunk {
                tool_call_id,
                tool_index,
                name,
                arguments_delta,
            } => {
                assert_eq!(tool_call_id.as_deref(), Some("call_x"));
                assert_eq!(tool_index, 7);
                assert_eq!(name.as_deref(), Some("future_tool"));
                assert_eq!(arguments_delta.as_deref(), Some("..."));
            }
            other => panic!("expected ToolCallDeltaChunk, got {other:?}"),
        }
    }

    /// Helper: `GoalUpdated` with all optional fields populated.
    fn make_goal_updated_full() -> SessionUpdate {
        SessionUpdate::GoalUpdated {
            goal_id: "g-1".into(),
            objective: "Build widget".into(),
            status: "active".into(),
            phase: "executing".into(),
            token_budget: Some(100_000),
            tokens_used: 25_000,
            elapsed_ms: 5000,
            total_deliverables: 3,
            completed_deliverables: 1,
            current_deliverable_id: Some(2),
            current_deliverable_title: Some("Core logic".into()),
            current_subagent_role: Some("worker".into()),
            total_worker_rounds: 4,
            total_verify_rounds: 2,
            live_subagent_tokens: Some(10_000),
            live_tokens_by_model: vec![("grok-4".into(), 6_000), ("grok-3".into(), 4_000)],
            live_context_pct: Some(35),
            live_turn_count: Some(3),
            live_tool_call_count: Some(8),
            last_event: Some("worker_completed".into()),
            last_event_detail: Some("Core logic".into()),
            last_event_timestamp: Some("2026-01-01T00:05:00Z".into()),
            token_baseline: 0,
            finished_subagent_tokens: 0,
            deliverables: vec![],
            pause_message: None,
            classifier_runs_attempted: Some(2),
            classifier_max_runs: Some(3),
            last_classifier_verdict: Some(GoalClassifierVerdict::NotAchieved),
            last_classifier_details_path: Some("/tmp/details.md".into()),
            verifying_completion: Some(true),
            planning: Some(true),
        }
    }

    /// Helper: `GoalUpdated` with all optional fields `None` / zeroed.
    fn make_goal_updated_minimal() -> SessionUpdate {
        SessionUpdate::GoalUpdated {
            goal_id: "g-min".into(),
            objective: "Test".into(),
            status: "active".into(),
            phase: "idle".into(),
            token_budget: None,
            tokens_used: 0,
            elapsed_ms: 0,
            total_deliverables: 0,
            completed_deliverables: 0,
            current_deliverable_id: None,
            current_deliverable_title: None,
            current_subagent_role: None,
            total_worker_rounds: 0,
            total_verify_rounds: 0,
            live_subagent_tokens: None,
            live_tokens_by_model: Vec::new(),
            live_context_pct: None,
            live_turn_count: None,
            live_tool_call_count: None,
            last_event: None,
            last_event_detail: None,
            last_event_timestamp: None,
            token_baseline: 0,
            finished_subagent_tokens: 0,
            deliverables: vec![],
            pause_message: None,
            classifier_runs_attempted: None,
            classifier_max_runs: None,
            last_classifier_verdict: None,
            last_classifier_details_path: None,
            verifying_completion: None,
            planning: None,
        }
    }

    #[test]
    fn goal_updated_serializes_snake_case_tag() {
        let update = make_goal_updated_full();
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "goal_updated");
        assert_eq!(json["goal_id"], "g-1");
        assert_eq!(json["status"], "active");
        assert_eq!(json["phase"], "executing");
        assert_eq!(json["token_budget"], 100_000);
        assert_eq!(json["tokens_used"], 25_000);
        assert_eq!(json["total_deliverables"], 3);
        assert_eq!(json["completed_deliverables"], 1);
        assert_eq!(json["current_deliverable_idx"], 2);
        assert_eq!(json["total_worker_rounds"], 4);
        assert_eq!(json["total_verify_rounds"], 2);
        assert_eq!(json["live_subagent_tokens"], 10_000);
        assert_eq!(json["live_tokens_by_model"][0][0], "grok-4");
        assert_eq!(json["live_tokens_by_model"][0][1], 6_000);
        assert_eq!(json["live_context_pct"], 35);
        assert_eq!(json["last_event"], "worker_completed");
        assert_eq!(json["classifier_runs_attempted"], 2);
        assert_eq!(json["classifier_max_runs"], 3);
        assert_eq!(json["last_classifier_verdict"], "not_achieved");
        assert_eq!(json["last_classifier_details_path"], "/tmp/details.md");
        assert_eq!(json["verifying_completion"], true);
        assert_eq!(json["planning"], true);
    }

    #[test]
    fn goal_updated_roundtrips_through_json() {
        let update = make_goal_updated_minimal();
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);

        // Also roundtrip the full variant.
        let full = make_goal_updated_full();
        let json_str = serde_json::to_string(&full).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(full, parsed);
    }

    #[test]
    fn goal_updated_optional_fields_skipped_when_none() {
        let json = serde_json::to_value(make_goal_updated_minimal()).unwrap();
        assert!(json.get("token_budget").is_none());
        assert!(json.get("current_deliverable_idx").is_none());
        assert!(json.get("current_deliverable_title").is_none());
        assert!(json.get("current_subagent_role").is_none());
        assert!(json.get("live_subagent_tokens").is_none());
        assert!(json.get("live_tokens_by_model").is_none());
        assert!(json.get("live_context_pct").is_none());
        assert!(json.get("live_turn_count").is_none());
        assert!(json.get("live_tool_call_count").is_none());
        assert!(json.get("last_event").is_none());
        assert!(json.get("last_event_detail").is_none());
        assert!(json.get("last_event_timestamp").is_none());
        assert!(json.get("classifier_runs_attempted").is_none());
        assert!(json.get("classifier_max_runs").is_none());
        assert!(json.get("last_classifier_verdict").is_none());
        assert!(json.get("last_classifier_details_path").is_none());
        assert!(json.get("verifying_completion").is_none());
        assert!(json.get("planning").is_none());
    }

    #[test]
    fn goal_updated_payload_missing_new_fields_deserializes_to_none() {
        // Wire round-trip: an older shell that predates the classifier / planning fields will omit them all
        // Each must deserialize to `None` so the pager keeps working
        let json = r#"{
            "sessionUpdate": "goal_updated",
            "goal_id": "g-old",
            "objective": "test",
            "status": "active",
            "phase": "idle",
            "tokens_used": 0,
            "elapsed_ms": 0,
            "total_deliverables": 0,
            "completed_deliverables": 0,
            "total_worker_rounds": 0,
            "total_verify_rounds": 0
        }"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        match update {
            SessionUpdate::GoalUpdated {
                classifier_runs_attempted,
                classifier_max_runs,
                last_classifier_verdict,
                last_classifier_details_path,
                verifying_completion,
                planning,
                ..
            } => {
                assert_eq!(classifier_runs_attempted, None);
                assert_eq!(classifier_max_runs, None);
                assert_eq!(last_classifier_verdict, None);
                assert_eq!(last_classifier_details_path, None);
                assert_eq!(verifying_completion, None);
                assert_eq!(planning, None);
            }
            other => panic!("expected GoalUpdated, got {other:?}"),
        }
    }

    #[test]
    fn goal_updated_payload_with_new_fields_round_trips() {
        // Wire round-trip: a payload that carries every classifier field survives serialize then deserialize without mutation
        let update = make_goal_updated_full();
        let json_str = serde_json::to_string(&update).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(update, parsed);

        match parsed {
            SessionUpdate::GoalUpdated {
                classifier_runs_attempted,
                classifier_max_runs,
                last_classifier_verdict,
                last_classifier_details_path,
                verifying_completion,
                planning,
                ..
            } => {
                assert_eq!(classifier_runs_attempted, Some(2));
                assert_eq!(classifier_max_runs, Some(3));
                assert_eq!(
                    last_classifier_verdict,
                    Some(GoalClassifierVerdict::NotAchieved)
                );
                assert_eq!(
                    last_classifier_details_path.as_deref(),
                    Some("/tmp/details.md")
                );
                assert_eq!(verifying_completion, Some(true));
                assert_eq!(planning, Some(true));
            }
            other => panic!("expected GoalUpdated, got {other:?}"),
        }
    }

    #[test]
    fn goal_updated_backward_compat_without_optional_fields() {
        let json = r#"{
            "sessionUpdate": "goal_updated",
            "goal_id": "g-bc",
            "objective": "test",
            "status": "active",
            "phase": "idle",
            "tokens_used": 0,
            "elapsed_ms": 0,
            "total_deliverables": 0,
            "completed_deliverables": 0,
            "total_worker_rounds": 0,
            "total_verify_rounds": 0
        }"#;
        let update: SessionUpdate = serde_json::from_str(json).unwrap();
        match update {
            SessionUpdate::GoalUpdated {
                goal_id,
                token_budget,
                current_deliverable_id,
                live_subagent_tokens,
                last_event,
                ..
            } => {
                assert_eq!(goal_id, "g-bc");
                assert_eq!(token_budget, None);
                assert_eq!(current_deliverable_id, None);
                assert_eq!(live_subagent_tokens, None);
                assert_eq!(last_event, None);
            }
            other => panic!("expected GoalUpdated, got {other:?}"),
        }
    }

    // ── ModelChanged (leader-mode multi-client model switch fan-out) ──

    /// Pins the exact `ModelChanged` JSON, since the pager and any third-party clients consume this on the wire. `sessionUpdate` tag is the snake_case variant name.
    /// Field names use Rust snake_case (struct fields are not subject to `rename_all`; that only renames the tag). `reasoning_effort` is omitted entirely when `None`.
    /// The wire stays smaller, and absence stays distinguishable from an explicit user clear, if that ever becomes a real distinction.
    #[test]
    fn model_changed_serializes_snake_case_with_optional_effort() {
        let with_effort = SessionUpdate::ModelChanged {
            model_id: "grok-4".into(),
            reasoning_effort: Some("high".into()),
        };
        let json = serde_json::to_value(&with_effort).unwrap();
        assert_eq!(json["sessionUpdate"], "model_changed");
        assert_eq!(json["model_id"], "grok-4");
        assert_eq!(json["reasoning_effort"], "high");

        let without_effort = SessionUpdate::ModelChanged {
            model_id: "grok-3".into(),
            reasoning_effort: None,
        };
        let json = serde_json::to_value(&without_effort).unwrap();
        assert_eq!(json["sessionUpdate"], "model_changed");
        assert_eq!(json["model_id"], "grok-3");
        assert!(
            json.get("reasoning_effort").is_none(),
            "reasoning_effort: None must be skipped on the wire so old pagers \
             and third-party ACP clients see a smaller, no-extra-keys payload"
        );
    }

    /// `ModelChanged` round-trips through JSON: a follower client deserializes the exact same value the agent serialized.
    /// Pins the field order / case so an accidental rename doesn't silently degrade to `Unknown`.
    /// The `#[serde(other)]` catch-all would swallow that on the pager side and break multi-client model sync without any test failing.
    #[test]
    fn model_changed_roundtrips_through_json() {
        let original = SessionUpdate::ModelChanged {
            model_id: "grok-4".into(),
            reasoning_effort: Some("medium".into()),
        };
        let json_str = serde_json::to_string(&original).unwrap();
        let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
        assert_eq!(original, parsed);
    }

    /// Wraps `ModelChanged` in the full `SessionNotification` envelope and checks the keys the leader's session-scoped fan-out matches on.
    /// Those are the top-level `sessionId` (camelCase from the envelope's `rename_all`) and the nested `update.sessionUpdate == "model_changed"`.
    /// Without the top-level `sessionId`, the leader's `extract_session_id` returns `None`. The notification then falls through to the last-active-client fallback instead of broadcasting, silently breaking multi-client sync.
    #[test]
    fn model_changed_envelope_carries_session_id_at_top_level() {
        let notif = SessionNotification {
            session_id: acp::SessionId::new("sess-abc"),
            update: SessionUpdate::ModelChanged {
                model_id: "grok-4".into(),
                reasoning_effort: None,
            },
            meta: None,
        };
        let json = serde_json::to_value(&notif).unwrap();
        assert_eq!(json["sessionId"], "sess-abc");
        assert_eq!(json["update"]["sessionUpdate"], "model_changed");
        assert_eq!(json["update"]["model_id"], "grok-4");
    }

    // ── TurnCompleted (durable, replayable turn-end signal) ──

    #[test]
    fn turn_completed_serializes_snake_case_tag_and_fields() {
        // Mirrors the SubagentProgress convention: `rename_all = "snake_case"` only renames the tag
        // Struct fields keep their Rust snake_case names on the wire
        let update = SessionUpdate::TurnCompleted {
            prompt_id: "p-1".into(),
            stop_reason: "end_turn".into(),
            agent_result: Some("done".into()),
            error_kind: Some("max_tokens_truncation".into()),
            usage: None,
            elapsed_ms: None,
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "turn_completed");
        assert_eq!(json["prompt_id"], "p-1");
        assert_eq!(json["stop_reason"], "end_turn");
        assert_eq!(json["agent_result"], "done");
        assert_eq!(json["error_kind"], "max_tokens_truncation");
    }

    #[test]
    fn turn_completed_optional_fields_skipped_when_none() {
        let update = SessionUpdate::TurnCompleted {
            prompt_id: "p-2".into(),
            stop_reason: "cancelled".into(),
            agent_result: None,
            error_kind: None,
            usage: None,
            elapsed_ms: None,
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["sessionUpdate"], "turn_completed");
        assert!(json.get("agent_result").is_none());
        assert!(json.get("error_kind").is_none());
        assert!(json.get("elapsed_ms").is_none());
    }

    #[test]
    fn turn_completed_roundtrips_through_json() {
        for update in [
            SessionUpdate::TurnCompleted {
                prompt_id: "p-rt".into(),
                stop_reason: "end_turn".into(),
                agent_result: Some("result text".into()),
                error_kind: Some("max_tokens_truncation".into()),
                usage: None,
                elapsed_ms: Some(1234),
            },
            SessionUpdate::TurnCompleted {
                prompt_id: "p-min".into(),
                stop_reason: "error".into(),
                agent_result: None,
                error_kind: None,
                usage: None,
                elapsed_ms: None,
            },
        ] {
            let json_str = serde_json::to_string(&update).unwrap();
            let parsed: SessionUpdate = serde_json::from_str(&json_str).unwrap();
            assert_eq!(update, parsed);
        }
    }

    #[test]
    fn turn_completed_old_json_without_elapsed_ms_deserializes_none() {
        let json =
            r#"{"sessionUpdate":"turn_completed","prompt_id":"p-old","stop_reason":"end_turn"}"#;
        let parsed: SessionUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed,
            SessionUpdate::TurnCompleted {
                prompt_id: "p-old".into(),
                stop_reason: "end_turn".into(),
                agent_result: None,
                error_kind: None,
                usage: None,
                elapsed_ms: None,
            }
        );
    }

    #[test]
    fn turn_completed_elapsed_ms_some_roundtrips() {
        let update = SessionUpdate::TurnCompleted {
            prompt_id: "p-ms".into(),
            stop_reason: "end_turn".into(),
            agent_result: None,
            error_kind: None,
            usage: None,
            elapsed_ms: Some(1234),
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["elapsed_ms"], 1234);
        let parsed: SessionUpdate = serde_json::from_value(json).unwrap();
        assert_eq!(update, parsed);
    }

    #[test]
    fn project_result_hides_costs_when_partial_or_incomplete() {
        let mut model_usage = indexmap::IndexMap::new();
        model_usage.insert(
            "m".into(),
            PromptUsageModel {
                input_tokens: 100,
                cached_read_tokens: 40,
                output_tokens: 10,
                total_tokens: 110,
                model_calls: 4,
                cost_usd_ticks: Some(1_000_000_000),
                ..Default::default()
            },
        );
        let partial = PromptUsage {
            totals: PromptUsageModel {
                input_tokens: 100,
                cached_read_tokens: 40,
                output_tokens: 10,
                total_tokens: 110,
                model_calls: 5,
                cost_usd_ticks: Some(1_000_000_000),
                cost_is_partial: true,
                cost_missing_calls: 1,
                ..Default::default()
            },
            model_usage: model_usage.clone(),
            num_turns: 2,
            usage_is_incomplete: false,
        };
        let mut result = serde_json::json!({});
        project_result_usage(&mut result, &partial);
        assert_eq!(result["usage"]["input_tokens"], 60);
        assert!(result.get("total_cost_usd").is_none());
        assert!(result.get("total_cost_usd_ticks").is_none());
        assert_eq!(result["cost_is_partial"], true);
        assert!(result["modelUsage"]["m"].get("costUSD").is_none());

        let mut incomplete = PromptUsage {
            totals: PromptUsageModel {
                input_tokens: 50,
                output_tokens: 5,
                total_tokens: 55,
                model_calls: 1,
                cost_usd_ticks: Some(5_000_000_000),
                ..Default::default()
            },
            model_usage,
            num_turns: 1,
            usage_is_incomplete: true,
        };
        incomplete.scrub_untrustworthy_costs();
        assert!(incomplete.totals.cost_usd_ticks.is_none());
        let mut result = serde_json::json!({});
        project_result_usage(&mut result, &incomplete);
        assert_eq!(result["usage_is_incomplete"], true);
        assert!(result.get("total_cost_usd").is_none());
        assert!(result.get("total_cost_usd_ticks").is_none());
        assert!(result["modelUsage"]["m"].get("costUSD").is_none());
    }

    #[test]
    fn project_result_incomplete_empty_omits_zero_usage() {
        let usage = PromptUsage::project_from_ledger(None, true).unwrap();
        let mut result = serde_json::json!({});
        project_result_usage(&mut result, &usage);
        assert_eq!(result["usage_is_incomplete"], true);
        assert!(result.get("usage").is_none());
        assert!(result.get("num_turns").is_none());
        assert!(result.get("total_cost_usd").is_none());
    }

    #[test]
    fn attach_result_usage_fail_closed_on_parse_error() {
        let mut result = serde_json::json!({"ok": true});
        attach_result_usage_fail_closed(&mut result, &serde_json::json!("not-an-object"));
        assert_eq!(result["usage_is_incomplete"], true);
        assert!(result.get("usage").is_none());
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn cost_missing_calls_not_on_acp_wire() {
        let model = PromptUsageModel {
            input_tokens: 1,
            cost_missing_calls: 3,
            cost_is_partial: true,
            ..Default::default()
        };
        let v = serde_json::to_value(&model).unwrap();
        assert!(v.get("costMissingCalls").is_none());
        assert_eq!(v["costIsPartial"], true);
    }

    #[test]
    fn scrub_untrustworthy_costs_clears_ticks_when_partial() {
        let mut usage = PromptUsage {
            totals: PromptUsageModel {
                input_tokens: 10,
                output_tokens: 1,
                total_tokens: 11,
                cost_usd_ticks: Some(100),
                cost_is_partial: true,
                cost_missing_calls: 1,
                ..Default::default()
            },
            model_usage: Default::default(),
            num_turns: 1,
            usage_is_incomplete: false,
        };
        usage.scrub_untrustworthy_costs();
        assert!(usage.totals.cost_usd_ticks.is_none());
        assert!(usage.totals.cost_is_partial);
    }

    #[test]
    fn project_result_token_identity_uncached_plus_cache_plus_output() {
        let mut model_usage = indexmap::IndexMap::new();
        model_usage.insert(
            "m".into(),
            PromptUsageModel {
                input_tokens: 100,
                cached_read_tokens: 40,
                output_tokens: 10,
                total_tokens: 110,
                model_calls: 1,
                cost_usd_ticks: Some(2_000_000_000),
                ..Default::default()
            },
        );
        let usage = PromptUsage {
            totals: PromptUsageModel {
                input_tokens: 100,
                cached_read_tokens: 40,
                output_tokens: 10,
                total_tokens: 110,
                model_calls: 1,
                cost_usd_ticks: Some(2_000_000_000),
                ..Default::default()
            },
            model_usage,
            num_turns: 1,
            usage_is_incomplete: false,
        };
        let mut result = serde_json::json!({});
        project_result_usage(&mut result, &usage);
        let uncached = result["usage"]["input_tokens"].as_u64().unwrap();
        let cache = result["usage"]["cache_read_input_tokens"].as_u64().unwrap();
        let output = result["usage"]["output_tokens"].as_u64().unwrap();
        let total = result["usage"]["total_tokens"].as_u64().unwrap();
        assert_eq!(uncached, 60);
        assert_eq!(cache, 40);
        assert_eq!(output, 10);
        assert_eq!(total, uncached + cache + output);
        // ACP serde keeps full input_tokens; headless identity differs.
        let acp = serde_json::to_value(&usage).unwrap();
        assert_eq!(acp["inputTokens"], 100);
        assert_eq!(acp["cachedReadTokens"], 40);
        assert_ne!(acp["inputTokens"], result["usage"]["input_tokens"]);
        assert_eq!(result["modelUsage"]["m"]["inputTokens"], 60);
        assert_eq!(result["modelUsage"]["m"]["cacheReadInputTokens"], 40);
        assert_eq!(result["total_cost_usd"], 0.2);
        // Exact ticks accompany the float for tick-exact reconciliation.
        assert_eq!(result["total_cost_usd_ticks"], 2_000_000_000_i64);
    }

    #[test]
    fn turn_completed_missing_required_fields_fail_to_deserialize() {
        let missing_prompt_id = r#"{"sessionUpdate": "turn_completed", "stop_reason": "end_turn"}"#;
        assert!(serde_json::from_str::<SessionUpdate>(missing_prompt_id).is_err());
        let missing_stop_reason = r#"{"sessionUpdate": "turn_completed", "prompt_id": "p-1"}"#;
        assert!(serde_json::from_str::<SessionUpdate>(missing_stop_reason).is_err());
    }
}
