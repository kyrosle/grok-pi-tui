//! Public wire types (DTOs) for the ACP session actor.
//!
//! These are the request/response structs exchanged between the agent layer and the session actor.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::session::persistence::Summary;
use crate::util::config::DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT;

// ── Session list ───────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionListRequest {
    pub workspace_directory: PathBuf,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct AllSessionOverviewRequest {}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionListResponse {
    pub session_summaries: Vec<Summary>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct AllSessionOverviewResponse {
    pub all_sessions: BTreeMap<PathBuf, Vec<Summary>>,
}

// ── Compaction ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct CompactConversationRequest {
    #[serde(alias = "sessionId")]
    pub session_id: String,
    #[serde(default, alias = "userContext")]
    pub user_context: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct CompactConversationResponse {}

pub use xai_grok_shared::session::feedback::{FeedbackRequest, FeedbackRequestDismiss, FeedbackOutcome, FeedbackResponse, ClientFeedbackInput, FeedbackDraftUpdateRequest, FeedbackDraftSendRequest, FeedbackDraftEditedBody, FeedbackTraceUploadIntent};

// ── Rollout survey ──────────────────────────────────────────────────────

/// Request to submit rollout survey responses about worktree improvements
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RolloutSurveyRequest {
    pub session_id: String,
    pub preferences: Vec<String>,
    pub feedback: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct RolloutSurveyResponse {
    pub success: bool,
}

// ── Citations / comments ────────────────────────────────────────────────

/// A reference to a range of lines in a file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
}

/// Request to record an inline comment on a prompt turn.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentRequest {
    pub session_id: String,
    /// 0-indexed prompt turn this comment is associated with
    pub prompt_index: u32,
    pub comment: String,
    pub citation: Citation,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentResponse {
    pub comment_id: String,
    pub recorded: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentDeleteRequest {
    pub session_id: String,
    pub comment_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentDeleteResponse {
    pub comment_id: String,
    pub deleted: bool,
}

// ── Rewind ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewindMode {
    /// Roll back both conversation and files (full time-travel).
    All,
    /// Roll back conversation only; leave files untouched.
    /// Use when the agent went in the wrong direction but the code is fine.
    ConversationOnly,
    /// Roll back files only; leave conversation untouched.
    /// Use when the files went wrong but the conversation context is valuable.
    #[serde(alias = "code_only")]
    FilesOnly,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewindRequest {
    /// Target prompt index to rewind to (0-based).
    /// Rewinding to N restores the state from before prompt N ran; prompts 0..N-1 are kept.
    pub target_prompt_index: usize,
    /// Whether to force rewind even with conflicts
    pub force: bool,
    /// Clients must specify this explicitly.
    /// Defaults to `All` for backwards compatibility with older clients.
    #[serde(default = "default_rewind_mode")]
    pub mode: RewindMode,
}

pub(crate) fn default_rewind_mode() -> RewindMode {
    RewindMode::All
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewindResponse {
    pub success: bool,
    pub target_prompt_index: usize,
    pub mode: RewindMode,
    /// List of file paths that were reverted (only populated on success with All or FilesOnly)
    pub reverted_files: Vec<String>,
    /// List of file paths that can be cleanly reverted (no conflicts)
    #[serde(default)]
    pub clean_files: Vec<String>,
    /// List of conflicts that were encountered (when `force` is false and conflicts exist, `success` is false)
    pub conflicts: Vec<RewindConflictInfo>,
    /// The original prompt text at target_prompt_index, for pre-filling the input field.
    /// Populated on successful conversation rewind (All or ConversationOnly).
    #[serde(default)]
    pub prompt_text: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewindConflictInfo {
    pub path: String,
    pub conflict_type: String, // "missing_file", "extra_file", "content_mismatch"
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewindPointsRequest {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewindPointsResponse {
    pub rewind_points: Vec<RewindPointInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RewindPointInfo {
    pub prompt_index: usize,
    pub created_at: String,
    pub num_file_snapshots: usize,
    /// Whether this prompt has file snapshots that can be reverted.
    /// When false, only conversation rewind is available for this checkpoint.
    #[serde(default)]
    pub has_file_changes: bool,
    /// Preview of the user prompt text (truncated)
    #[serde(default)]
    pub prompt_preview: Option<String>,
}

// ── Session info ────────────────────────────────────────────────────────

pub use xai_grok_shared::session::{ContextInfo, TokenUsageCategory, count_detail};

pub use xai_grok_shared::session::{
    AssistantUsageMetric, CacheSessionMetrics, CacheUsageTotals, SessionInfoData,
    SessionInfoResponse, SessionTokenTotals, SessionUsageStats, model_display_name,
};

// ── Feedback context ────────────────────────────────────────────────────

/// Context gathered from a session to enrich feedback notifications.
///
/// Uses the shared feedback wire types directly so consumers can assign fields to `FeedbackSubmission` without mapping.
#[derive(Debug, Clone, Default)]
pub struct FeedbackContext {
    pub last_user_message: Option<String>,
    pub last_assistant_message: Option<String>,
    pub tool_outcomes: Vec<prod_mc_cli_chat_proxy_types::feedback_types::FeedbackToolOutcome>,
    pub compaction_count: i64,
    pub context_window_usage: u8,
    pub context_tokens_used: u64,
    pub context_window_tokens: u64,
    pub session_cwd: String,
}

// ── Startup hints ───────────────────────────────────────────────────────

// `pub` (not `pub(crate)`): carried by the public `SessionCommand` enum (`UpdateAttachPolicy`), whose fields are reachable at `pub`
// A `pub(crate)` field type there trips the `private_interfaces` lint
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupHints {
    #[serde(default)]
    pub non_interactive: bool,
    #[serde(default)]
    pub skip_git_status: bool,
    /// Leading conversation items to preserve verbatim across compaction (the immutable head).
    /// A fresh subagent's head is its spawn-injected items; a `resume_from` subagent's is just the System head so the resumed body stays compactable.
    #[serde(default)]
    pub inherited_prefix_len: Option<usize>,
    /// When true, this session is a subagent child and its prompts should not be appended to the per-CWD prompt_history.jsonl file.
    #[serde(default)]
    pub is_subagent: bool,
    /// Parent session id when this session is a subagent child.
    /// Emitted as `parent_agent_id` on the turn span for trace attribution.
    #[serde(default)]
    pub parent_session_id: Option<String>,
    /// The task's `subagent_type` when this session is a subagent child, put on hook payloads for attribution.
    /// It matches the `SubagentStart`/`SubagentStop` events the parent emits, which also key off the task type, not the resolved agent name.
    #[serde(default)]
    pub subagent_type: Option<String>,
    /// Set on a fork spawn so `install_system_prompt` does NOT overwrite the inherited System at `conversation[0]`.
    /// The verbatim parent copy already holds the parent's System, and overwriting it would bust the cache prefix.
    #[serde(default)]
    pub preserve_inherited_system: bool,
    /// Tool names the session delivers its reply through (e.g. a messaging MCP tool); listing any keeps the full MCP waits at the prefix and tool-definition gates instead of the short startup grace.
    #[serde(default)]
    pub delivery_tools: Vec<String>,
    /// Only `"alwaysAllow"` is honored: would-be prompts resolve as allow at the manager's dispatch gate.
    /// Clamped off by the managed always-approve pin; a configured `defaultMode` wins.
    /// Unlike `yoloMode` / `autoMode`, a warm re-attach to an already-resident actor does NOT re-apply it.
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(skip)]
    pub startup_traceparent: std::cell::RefCell<Option<String>>,
}

impl StartupHints {
    /// Shared by the spawn path and the resident re-attach path so both resolve identically.
    pub(crate) fn resolve_mcp_strategy(&self) -> xai_grok_telemetry::enums::McpInitStrategy {
        use xai_grok_telemetry::enums::McpInitStrategy;
        match std::env::var("MCP_INIT_STRATEGY") {
            Ok(v) if !v.trim().is_empty() => McpInitStrategy::from(v),
            _ if self.non_interactive => McpInitStrategy::Blocking,
            _ => McpInitStrategy::Progressive,
        }
    }

    pub(crate) fn take_mcp_reroot_traceparent(&self) -> Option<String> {
        if self.is_subagent {
            return None;
        }
        self.startup_traceparent.borrow_mut().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_mcp_reroot_traceparent_one_shot_and_skips_subagent() {
        let hints = StartupHints {
            startup_traceparent: std::cell::RefCell::new(Some("tp".to_owned())),
            ..Default::default()
        };
        assert_eq!(hints.take_mcp_reroot_traceparent().as_deref(), Some("tp"));
        assert_eq!(hints.take_mcp_reroot_traceparent(), None);

        let subagent = StartupHints {
            is_subagent: true,
            startup_traceparent: std::cell::RefCell::new(Some("tp".to_owned())),
            ..Default::default()
        };
        assert_eq!(subagent.take_mcp_reroot_traceparent(), None);
    }

    #[test]
    fn unknown_feedback_outcome_deserializes_as_unknown() {
        let response: FeedbackResponse = serde_json::from_value(serde_json::json!({
            "success": false,
            "outcome": "submitted_after_retry",
        }))
        .expect("newer outcome should remain backward compatible");

        assert_eq!(response.outcome, Some(FeedbackOutcome::Other));
    }

    #[test]
    fn desktop_client_type_deserializes_and_round_trips() {
        let json = r#"{
            "session_id": "sess-1",
            "client_type": "desktop",
            "rating_type": "thumbs",
            "rating_value": 1,
            "feedback_text": "great session",
            "feedback_categories": ["accuracy"]
        }"#;

        let mut input: ClientFeedbackInput = serde_json::from_str(json).unwrap();
        assert_eq!(
            input.client_type,
            prod_mc_cli_chat_proxy_types::feedback_types::ClientType::Desktop
        );
        assert_eq!(input.session_id, "sess-1");

        let submission = input.take_submission(Some("grok-3".into()), None, None, Some(5));
        assert_eq!(
            submission.client_type,
            prod_mc_cli_chat_proxy_types::feedback_types::ClientType::Desktop
        );
        assert_eq!(submission.client_type.to_string(), "desktop");
    }

    /// The agent uses `turn_number` to attach that turn's user/assistant text instead of the latest.
    #[test]
    fn turn_number_deserializes_from_snake_and_camel_case() {
        let snake = r#"{
            "session_id": "sess-1",
            "client_type": "desktop",
            "turn_number": 3
        }"#;
        let snake_input: ClientFeedbackInput = serde_json::from_str(snake).unwrap();
        assert_eq!(snake_input.turn_number, Some(3));

        let camel = r#"{
            "session_id": "sess-1",
            "client_type": "desktop",
            "turnNumber": 7
        }"#;
        let camel_input: ClientFeedbackInput = serde_json::from_str(camel).unwrap();
        assert_eq!(camel_input.turn_number, Some(7));

        let absent = r#"{
            "session_id": "sess-1",
            "client_type": "desktop"
        }"#;
        let absent_input: ClientFeedbackInput = serde_json::from_str(absent).unwrap();
        assert_eq!(absent_input.turn_number, None);
    }

    use serde_json::json;

    // ── RewindMode serialization ──────────────────────────────────────

    #[test]
    fn rewind_mode_serializes_to_snake_case() {
        assert_eq!(serde_json::to_value(RewindMode::All).unwrap(), json!("all"));
        assert_eq!(
            serde_json::to_value(RewindMode::ConversationOnly).unwrap(),
            json!("conversation_only")
        );
        assert_eq!(
            serde_json::to_value(RewindMode::FilesOnly).unwrap(),
            json!("files_only")
        );
    }

    #[test]
    fn rewind_mode_deserializes_from_snake_case() {
        assert_eq!(
            serde_json::from_value::<RewindMode>(json!("all")).unwrap(),
            RewindMode::All
        );
        assert_eq!(
            serde_json::from_value::<RewindMode>(json!("conversation_only")).unwrap(),
            RewindMode::ConversationOnly
        );
        assert_eq!(
            serde_json::from_value::<RewindMode>(json!("files_only")).unwrap(),
            RewindMode::FilesOnly
        );
        // Backwards-compat alias: "code_only" still deserializes to FilesOnly
        assert_eq!(
            serde_json::from_value::<RewindMode>(json!("code_only")).unwrap(),
            RewindMode::FilesOnly
        );
    }

    #[test]
    fn rewind_mode_default_is_all() {
        assert_eq!(default_rewind_mode(), RewindMode::All);
    }

    #[test]
    fn rewind_mode_rejects_unknown_variant() {
        assert!(serde_json::from_value::<RewindMode>(json!("code_only_v2")).is_err());
    }

    // ── RewindRequest backwards compatibility ─────────────────────────

    #[test]
    fn rewind_request_missing_mode_defaults_to_all() {
        let req: RewindRequest =
            serde_json::from_value(json!({"target_prompt_index": 2, "force": false})).unwrap();
        assert_eq!(req.mode, RewindMode::All);
        assert_eq!(req.target_prompt_index, 2);
        assert!(!req.force);
    }

    #[test]
    fn rewind_request_explicit_mode_is_respected() {
        let req: RewindRequest = serde_json::from_value(
            json!({"target_prompt_index": 5, "force": true, "mode": "code_only"}),
        )
        .unwrap();
        assert_eq!(req.mode, RewindMode::FilesOnly);
        assert!(req.force);
    }

    #[test]
    fn rewind_request_roundtrip() {
        let original = RewindRequest {
            target_prompt_index: 3,
            force: false,
            mode: RewindMode::ConversationOnly,
        };
        let json = serde_json::to_value(&original).unwrap();
        let decoded: RewindRequest = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.target_prompt_index, 3);
        assert_eq!(decoded.mode, RewindMode::ConversationOnly);
    }

    // ── RewindResponse fields ─────────────────────────────────────────

    #[test]
    fn rewind_response_includes_mode_and_prompt_text() {
        let resp = RewindResponse {
            success: true,
            target_prompt_index: 1,
            mode: RewindMode::ConversationOnly,
            reverted_files: vec![],
            clean_files: vec![],
            conflicts: vec![],
            prompt_text: Some("fix the bug".into()),
            error: None,
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["mode"], json!("conversation_only"));
        assert_eq!(v["prompt_text"], json!("fix the bug"));
        assert_eq!(v["success"], json!(true));
    }

    #[test]
    fn rewind_response_prompt_text_null_when_none() {
        let resp = RewindResponse {
            success: true,
            target_prompt_index: 0,
            mode: RewindMode::FilesOnly,
            reverted_files: vec!["src/main.rs".into()],
            clean_files: vec![],
            conflicts: vec![],
            prompt_text: None,
            error: None,
        };
        let v = serde_json::to_value(&resp).unwrap();
        assert!(v["prompt_text"].is_null());
        assert_eq!(v["reverted_files"], json!(["src/main.rs"]));
    }

    #[test]
    fn rewind_response_deserialize_with_defaults() {
        let v = json!({
            "success": false,
            "target_prompt_index": 4,
            "mode": "all",
            "reverted_files": [],
            "conflicts": [{"path": "a.rs", "conflict_type": "content_mismatch"}],
            "error": "dirty working tree"
        });
        let resp: RewindResponse = serde_json::from_value(v).unwrap();
        assert!(!resp.success);
        assert_eq!(resp.mode, RewindMode::All);
        assert!(resp.prompt_text.is_none());
        assert!(resp.clean_files.is_empty());
        assert_eq!(resp.conflicts.len(), 1);
        assert_eq!(resp.conflicts[0].path, "a.rs");
    }

    // ── RewindPointInfo.has_file_changes ──────────────────────────────

    #[test]
    fn rewind_point_info_has_file_changes_true() {
        let point = RewindPointInfo {
            prompt_index: 2,
            created_at: "2025-01-01T00:00:00Z".into(),
            num_file_snapshots: 3,
            has_file_changes: true,
            prompt_preview: Some("refactor auth".into()),
        };
        let v = serde_json::to_value(&point).unwrap();
        assert_eq!(v["has_file_changes"], json!(true));
        assert_eq!(v["num_file_snapshots"], json!(3));
    }

    #[test]
    fn rewind_point_info_has_file_changes_false_when_no_snapshots() {
        let point = RewindPointInfo {
            prompt_index: 0,
            created_at: "2025-01-01T00:00:00Z".into(),
            num_file_snapshots: 0,
            has_file_changes: false,
            prompt_preview: None,
        };
        let v = serde_json::to_value(&point).unwrap();
        assert_eq!(v["has_file_changes"], json!(false));
        assert_eq!(v["num_file_snapshots"], json!(0));
    }

    #[test]
    fn rewind_point_info_has_file_changes_defaults_to_false() {
        let v = json!({
            "prompt_index": 1,
            "created_at": "2025-01-01T00:00:00Z",
            "num_file_snapshots": 5
        });
        let point: RewindPointInfo = serde_json::from_value(v).unwrap();
        assert!(!point.has_file_changes);
        assert_eq!(point.num_file_snapshots, 5);
        assert!(point.prompt_preview.is_none());
    }

    #[test]
    fn session_info_stats_are_optional_and_round_trip() {
        let legacy = serde_json::json!({
            "sessionId": "s1",
            "cwd": "/tmp",
            "model": null,
            "resolvedModelId": null,
            "modelFingerprint": null,
            "turns": 0,
            "turnIndex": 0,
            "context": ContextInfo::default()
        });
        let parsed: SessionInfoResponse = serde_json::from_value(legacy).unwrap();
        assert!(parsed.session_name.is_none());
        assert!(parsed.session_stats.is_none());

        let populated = SessionInfoResponse {
            session_id: "s2".into(),
            cwd: "/repo".into(),
            session_name: Some("named".into()),
            session_file: Some("/repo/session.jsonl".into()),
            session_stats: Some(SessionUsageStats {
                total_messages: 7,
                user_messages: 2,
                assistant_messages: 3,
                tool_calls: 2,
                tool_results: 2,
                tokens: SessionTokenTotals {
                    input: 10,
                    output: 20,
                    cache_read: 30,
                    cache_write: 40,
                    total: 100,
                },
                cost: 0.125,
                model_usage: Vec::new(),
            }),
            cache_metrics: None,
            data: SessionInfoData {
                agent_name: None,
                model: None,
                model_display_name: None,
                resolved_model_id: None,
                model_fingerprint: None,
                show_model_fingerprint: false,
                api_backend: None,
                conversation_id: None,
                turns: 0,
                turn_index: 0,
                context: ContextInfo::default(),
            },
        };
        let value = serde_json::to_value(&populated).unwrap();
        assert_eq!(value["sessionName"], "named");
        assert_eq!(value["sessionStats"]["tokens"]["cacheRead"], 30);
        let roundtrip: SessionInfoResponse = serde_json::from_value(value).unwrap();
        assert_eq!(roundtrip.session_stats.unwrap().cost, 0.125);
    }

    #[test]
    fn context_info_from_notification_computes_derived_fields() {
        let c = ContextInfo::from_notification(50_000, 200_000);
        assert_eq!(c.used, 50_000);
        assert_eq!(c.total, 200_000);
        assert_eq!(c.usage_pct, 25);
        assert_eq!(c.free_tokens, 150_000);
        assert_eq!(c.system_prompt_tokens, 0);
        assert_eq!(c.message_count, 0);
        assert_eq!(c.compaction_count, 0);
    }

    #[test]
    fn context_info_from_notification_zero_total() {
        let c = ContextInfo::from_notification(100, 0);
        assert_eq!(c.usage_pct, 0);
        assert_eq!(c.free_tokens, 0);
    }

    #[test]
    fn usage_categories_tolerate_serde_skew_in_both_directions() {
        // Old agents omit the field entirely: deserialize to empty.
        let from_old_agent: ContextInfo = serde_json::from_str(r#"{"used":1,"total":2}"#).unwrap();
        assert!(from_old_agent.usage_categories.is_empty());

        // Empty vec is skipped on serialize (old clients see no new field).
        let json = serde_json::to_string(&ContextInfo::default()).unwrap();
        assert!(!json.contains("usageCategories"), "{json}");

        // Extra fields from newer agents are ignored, keeping the label renderable
        let row: TokenUsageCategory =
            serde_json::from_str(r#"{"kind":"agents_md","label":"AGENTS.md","tokens":42}"#)
                .unwrap();
        assert_eq!(row.label, "AGENTS.md");

        // Rows round-trip.
        let original = TokenUsageCategory::skills_listing("t", 2);
        let json = serde_json::to_string(&original).unwrap();
        let roundtripped: TokenUsageCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped, original);

        let agents = TokenUsageCategory::agents_md("rules", 1);
        assert_eq!(agents.label, "AGENTS.md");
        assert_eq!(agents.detail.as_deref(), Some("1 file"));
        assert!(agents.tokens > 0);
    }
}
