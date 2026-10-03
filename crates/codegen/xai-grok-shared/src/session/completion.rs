//! Canonical prompt completion metadata and its original category mapping.

/// Structured context for a cancelled turn.
/// Clients deserialize into this same type.
/// Absent fields are skipped.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CancellationContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_name: Option<String>,
    /// What triggered the cancel (e.g. `"send_now"`, `"esc"`, `"mouse"`), sent as `cancelTrigger` on the turn-end `_meta`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
}
/// Prompt completion kind returned to the ACP layer.
#[derive(Debug, Clone)]
pub enum PromptCompletionKind {
    Completed,
    /// Silent EndTurn after stationarity/true-noop thrash.
    /// Distinct from Completed so goal continuation is not re-queued under an active goal.
    StationarityEnded,
    Cancelled {
        category: Option<xai_grok_session_events::types::CancellationCategory>,
        context: Option<CancellationContext>,
    },
    MaxTurnsReached {
        limit: usize,
    },
    Rewound,
    /// A queued prompt was removed (or cleared) from the server-authoritative queue before it ever ran.
    /// The prompt never started a turn, so the `prompt_complete` broadcast and the roster `Idle` delta must be skipped.
    /// The `Idle` delta would flip the dashboard off `Working` while the real turn is still in flight.
    RemovedFromQueue,
}
/// `_meta.completionKind` on a `PromptResponse`. Distinguishes a queued prompt
/// that never ran from a real cancelled turn (both use `StopReason::Cancelled`).
pub const COMPLETION_KIND_KEY: &str = "completionKind";
/// `_meta.completionKind` value for [`PromptCompletionKind::RemovedFromQueue`].
pub const REMOVED_FROM_QUEUE_KIND: &str = "removedFromQueue";
/// `_meta.cancellationCategory` of a hook-denied cancel; the pager matches it to render the blocked-by-a-hook marker.
pub const HOOK_DENIED_CATEGORY: &str = "HookDenied";
/// `_meta.cancellationCategory` of a max-turns end; headless matches it to drive the max-turns exit code.
pub const MAX_TURNS_REACHED_CATEGORY: &str = "max_turns_reached";
/// `_meta.cancellationCategory` of a stationarity end.
pub const ACTION_STATIONARITY_CATEGORY: &str = "action_stationarity";
/// `_meta.cancellationCategory` wire name of a cancel category: an explicit match so a variant rename cannot silently change the wire.
/// This is deliberately a second vocabulary next to the serde snake_case of the events.jsonl / after-turn rails.
/// `_meta` shipped PascalCase and clients match it.
pub fn meta_category_str(
    category: xai_grok_session_events::types::CancellationCategory,
) -> &'static str {
    use xai_grok_session_events::types::CancellationCategory;
    match category {
        CancellationCategory::HookDenied => HOOK_DENIED_CATEGORY,
        CancellationCategory::PermissionRejected => "PermissionRejected",
        CancellationCategory::PermissionCancelled => "PermissionCancelled",
        CancellationCategory::MidTurnAbort => "MidTurnAbort",
    }
}
impl PromptCompletionKind {
    /// The completion's `_meta.cancellationCategory`, shared by every terminal rail so the wires never disagree.
    /// The rails: `PromptResponse` `_meta`, the legacy `prompt_complete`, and the durable `TurnCompleted`.
    pub fn cancellation_category_meta(&self) -> Option<String> {
        match self {
            Self::Cancelled { category, .. } => {
                category.map(|cat| meta_category_str(cat).to_string())
            }
            Self::MaxTurnsReached { .. } => Some(MAX_TURNS_REACHED_CATEGORY.to_string()),
            Self::StationarityEnded => Some(ACTION_STATIONARITY_CATEGORY.to_string()),
            Self::Completed | Self::Rewound | Self::RemovedFromQueue => None,
        }
    }
    /// The completion's `_meta.cancellationContext` (hook name, reason, trigger), stamped beside `cancellationCategory`.
    /// It lets a client show WHY a turn was blocked without scraping annotations.
    /// Additive: shipped clients ignore unknown `_meta` keys.
    pub fn cancellation_context_meta(&self) -> Option<serde_json::Value> {
        match self {
            Self::Cancelled {
                context: Some(ctx), ..
            } => serde_json::to_value(ctx).ok(),
            _ => None,
        }
    }
}
