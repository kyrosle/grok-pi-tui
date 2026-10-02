//! Canonical wire data for session/context views and feedback outcomes.

use super::ContextInfo;

/// Unified session info data returned by GetSessionInfo.
/// One query, all the fields needed for /session-info and /context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfoData {
    /// Agent definition name for this session (e.g. `grok-build`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_display_name: Option<String>,
    pub resolved_model_id: Option<String>,
    pub model_fingerprint: Option<String>,
    /// Catalog opt-in to display checkpoint identity (the served fingerprint and the resolved model ID) for this model.
    /// Sole control: the client keeps no built-in per-slug default, so turning this off in the catalog hides both.
    #[serde(default)]
    pub show_model_fingerprint: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_backend: Option<String>,
    /// Gateway chat conversation id when this session is gateway-proxied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub turns: u64,
    /// Current turn (0-based).
    /// Matches the `turn_number` used in TurnStarted events, traces, and rewinds.
    #[serde(default)]
    pub turn_index: u64,
    pub context: ContextInfo,
}

pub fn model_display_name(
    name: Option<&str>,
    model: &str,
    resolved: Option<&str>,
    show_resolved: bool,
) -> String {
    // If the catalogue entry has a name, that's the displayed model.
    if let Some(n) = name {
        return n.to_string();
    }

    // For displaying the resolved model slug from the API response.
    if show_resolved {
        return match resolved.filter(|r| *r != model) {
            Some(r) => format!("{model} ({r})"),
            None => model.to_string(),
        };
    }

    model.to_string()
}

/// Per-turn assistant usage row for cache hit graph/stats (pi-cache-graph shape).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantUsageMetric {
    pub sequence: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_branch_sequence: Option<u32>,
    pub entry_id: String,
    pub timestamp: String,
    pub provider: String,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total_tokens: u64,
    pub cache_hit_percent: f64,
    pub is_on_active_branch: bool,
    /// True when input/output were estimated from content (provider usage was 0).
    #[serde(default)]
    pub usage_estimated: bool,
}

/// Aggregate token totals for cache stats.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CacheUsageTotals {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total_tokens: u64,
    pub assistant_messages: u64,
}

/// Session-wide cache metrics for Context modal graph/stats views.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CacheSessionMetrics {
    pub all_messages: Vec<AssistantUsageMetric>,
    pub active_branch_messages: Vec<AssistantUsageMetric>,
    pub tree_totals: CacheUsageTotals,
    pub active_branch_totals: CacheUsageTotals,
    /// Prompt tokens that should have been cache reads but were billed again.
    #[serde(default)]
    pub rebilled_tokens: u64,
    /// Number of cache misses above Pi's 1,024-token noise floor.
    #[serde(default)]
    pub cache_miss_count: u32,
    /// How many assistant rows used content-size estimates (provider wrote 0 usage).
    #[serde(default)]
    pub estimated_count: u32,
}

/// Session-wide token totals from Pi's `get_session_stats` RPC.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionTokenTotals {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total: u64,
}

/// Session-wide message, tool, token, and billing totals.
///
/// Kept separate from [`ContextInfo`]: these aggregate the entire persisted
/// session (including compacted history), while context is the current window.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionUsageStats {
    pub total_messages: u64,
    pub user_messages: u64,
    pub assistant_messages: u64,
    pub tool_calls: u64,
    pub tool_results: u64,
    pub tokens: SessionTokenTotals,
    pub cost: f64,
}

/// Full wire response for `x.ai/session/info`.
///
/// Wraps `SessionInfoData` with session-level fields (`session_id`, `cwd`) that come from the agent layer rather than the session actor.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfoResponse {
    pub session_id: String,
    pub cwd: String,
    /// User-facing Pi session name when one has been assigned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_name: Option<String>,
    /// On-disk session path when the agent persists JSONL (Pi `sessionFile`).
    /// Absent for in-memory / Grok cloud-only sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
    /// Optional full-session counts, token totals, and cost (Pi `SessionStats`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_stats: Option<SessionUsageStats>,
    /// Optional Pi cache-hit series for Context modal graph/stats (grok-pi).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_metrics: Option<CacheSessionMetrics>,
    #[serde(flatten)]
    pub data: SessionInfoData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FeedbackOutcome {
    Submitted,
    SubmittedCleanupFailed,
    LocalOnly,
    OutcomeUnknown,
    /// Unknown wire variant from a newer shell. Treat like [`Self::OutcomeUnknown`]:
    /// do not claim a definite failure or invite a resend.
    #[serde(other)]
    Other,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackResponse {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<FeedbackOutcome>,
    /// Single-use capability returned only for a successful, explicitly consented modal report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_upload_token: Option<String>,
}
