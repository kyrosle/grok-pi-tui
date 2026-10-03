//! Canonical native session notification wire data and pure projections.

use agent_client_protocol as acp;
use chrono::{DateTime, Utc};
use xai_tool_types::task_snapshot::{TaskKind, TaskSnapshot};

/// Retained for wire backwards compatibility; always empty in the simplified goal model (no deliverables).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GoalDeliverableInfo {
    pub id: u32,
    pub title: String,
    pub status: String,
}

pub use xai_tool_types::workflow::{WorkflowAgentInfo, WorkflowPhaseInfo};

/// `_meta` key on rename fan-out (`SessionSummaryGenerated` and ACP `SessionInfoUpdate`). Old clients ignore unknown meta.
pub const TITLE_IS_MANUAL_META_KEY: &str = "x.ai/titleIsManual";

/// `_meta` object carried on a manual-rename fan-out.
pub fn title_is_manual_meta() -> serde_json::Value {
    serde_json::json!({ TITLE_IS_MANUAL_META_KEY: true })
}

/// `_meta` object carried on `/rename --auto` fan-out.
/// Distinct from *absent* meta: an auto title must not clobber `display_name`.
pub fn title_is_unpinned_meta() -> serde_json::Value {
    serde_json::json!({ TITLE_IS_MANUAL_META_KEY: false })
}

/// xAI-specific session notification (parallel to acp::SessionNotification).
/// This wraps an XaiSessionUpdate with session context for persistence and replay.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNotification {
    /// The ID of the session this update pertains to.
    pub session_id: acp::SessionId,
    /// The actual update content.
    pub update: SessionUpdate,
    /// Extension point for implementations
    #[serde(skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

impl SessionNotification {
    /// Durable projection: typed field cleared, not a JSON patch.
    /// Sole strip site for JSONL and GCS writes.
    pub fn without_live_agent_address(&self) -> std::borrow::Cow<'_, Self> {
        match &self.update {
            SessionUpdate::SubagentSpawned {
                agent_address: Some(_),
                ..
            } => {
                let mut stripped = self.clone();
                if let SessionUpdate::SubagentSpawned { agent_address, .. } = &mut stripped.update {
                    *agent_address = None;
                }
                std::borrow::Cow::Owned(stripped)
            }
            _ => std::borrow::Cow::Borrowed(self),
        }
    }

    pub fn to_durable_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(&*self.without_live_agent_address())
    }
}

/// | Surface | `input_tokens` / `inputTokens` | Cost | |---------|--------------------------------|------| | **ACP** (`PromptUsage`) | **Full** prompt sum (includes cache reads) | `costUsdTicks` (1e10 ticks = $1), scrubbed when partial/incomplete | | **Headless** ([`project_result_usage`]) | **Uncached only** (`full − cache_read`) | Float `total_cost_usd` + exact `total_cost_usd_ticks`, only when complete | | ACP `_meta` sibling fields | **Last model call only** (not whole-prompt) | — |
/// Trust cost only when present **and** not `usageIsIncomplete` **and** not `costIsPartial`. Absence of cost means untrustworthy or unknown, not free.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PromptUsage {
    #[serde(flatten)]
    pub totals: PromptUsageModel,
    #[serde(
        default,
        rename = "modelUsage",
        skip_serializing_if = "indexmap::IndexMap::is_empty"
    )]
    pub model_usage: indexmap::IndexMap<String, PromptUsageModel>,
    /// Main-agent loop rounds (same unit as `--max-turns`).
    #[serde(default, rename = "numTurns")]
    pub num_turns: u64,
    /// Bill may under-count (open subagents, usage not applied, or drain timeout).
    #[serde(
        default,
        rename = "usageIsIncomplete",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub usage_is_incomplete: bool,
}

impl PromptUsage {
    /// Project a ledger snapshot for the wire. Always scrubs untrustworthy costs.
    /// Returns `Some` whenever `incomplete` is set (even if `ledger` is `None`) so the flag is never dropped by omission.
    #[cfg(feature = "stock-runtime")]
    pub fn project_from_ledger(
        ledger: Option<&xai_chat_state::UsageLedger>,
        incomplete: bool,
    ) -> Option<Self> {
        let mut usage = match ledger {
            Some(ledger) => {
                let mut usage = Self::from(ledger);
                if incomplete {
                    usage.usage_is_incomplete = true;
                }
                usage
            }
            None if incomplete => Self {
                usage_is_incomplete: true,
                ..Default::default()
            },
            None => return None,
        };
        usage.scrub_untrustworthy_costs();
        Some(usage)
    }

    /// On the error path any open ledger is always incomplete (it may under-count without a freeze drain).
    /// `may_undercount` only matters when the ledger is empty.
    #[cfg(feature = "stock-runtime")]
    pub fn for_error_path(
        ledger: Option<&xai_chat_state::UsageLedger>,
        may_undercount: bool,
    ) -> Option<Self> {
        match (ledger, may_undercount) {
            (Some(l), _) => Self::project_from_ledger(Some(l), true),
            (None, true) => Self::project_from_ledger(None, true),
            (None, false) => None,
        }
    }

    /// Drop cost ticks when partial or incomplete so all wire surfaces fail closed.
    /// Incomplete bills clear ticks even when `cost_is_partial` is false.
    pub fn scrub_untrustworthy_costs(&mut self) {
        if !(self.usage_is_incomplete || self.totals.cost_is_partial) {
            return;
        }
        self.totals.cost_usd_ticks = None;
        for m in self.model_usage.values_mut() {
            m.cost_usd_ticks = None;
            if self.totals.cost_is_partial {
                m.cost_is_partial = true;
            }
        }
    }

    fn is_token_empty(&self) -> bool {
        // Exhaustive destructure: a new token field must decide whether it counts as "billed something" here
        let PromptUsageModel {
            input_tokens,
            output_tokens,
            total_tokens: _, // derived from input + output
            cached_read_tokens,
            cache_creation_tokens, // subset of input_tokens on the wire
            reasoning_tokens: _,   // subset of output_tokens
            model_calls,
            api_duration_ms: _, // timing, not tokens
            cost_usd_ticks: _,  // cost without usage cannot occur
            cost_is_partial: _,
            cost_missing_calls: _,
        } = self.totals;
        model_calls == 0
            && input_tokens == 0
            && output_tokens == 0
            && cached_read_tokens == 0
            && cache_creation_tokens == 0
            && self.model_usage.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptUsageModel {
    /// Full prompt input tokens including cache reads (ACP identity).
    /// Headless projects uncached only; see [`project_result_usage`].
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub cached_read_tokens: u64,
    /// Cache-creation prompt tokens, folded into `input_tokens` on the ACP wire
    /// but projected as a disjoint bucket in the headless shape.
    #[serde(default)]
    pub cache_creation_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(default)]
    pub model_calls: u64,
    #[serde(default)]
    pub api_duration_ms: u64,
    /// Server cost in USD ticks (`USD_TICKS_PER_USD` is 1e10 ticks per $1). Absent when scrubbed, missing, or zero on the wire.
    /// Headless projects the totals as float `total_cost_usd` (plus exact `total_cost_usd_ticks`) and per-model rows as float `costUSD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd_ticks: Option<i64>,
    /// Some folded calls lacked cost, so any cost shown is a partial sum.
    /// After a scrub of a partial bill, complete per-model rows are also stamped `true`.
    /// The flag means "do not trust this row's cost", not "this row's own cost was partial".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cost_is_partial: bool,
    /// How many calls reported usage but no cost.
    /// Internal accounting for `cost_is_partial` only; never on the public ACP wire.
    #[serde(default, skip_serializing)]
    pub cost_missing_calls: u64,
}

/// One model call's token usage: the four Messages API `message.usage` fields plus `reasoning_tokens`.
/// `input_tokens` is the uncached prompt portion.
/// Distinct from [`PromptUsageModel`], which sums the whole prompt.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResponseUsage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
}

#[cfg(feature = "stock-runtime")]
impl From<&xai_chat_state::UsageTotals> for PromptUsageModel {
    fn from(t: &xai_chat_state::UsageTotals) -> Self {
        // Exhaustive destructure: a new ledger field cannot silently miss the wire
        // When one is added here, also extend `project_result_usage`
        let xai_chat_state::UsageTotals {
            input_tokens,
            output_tokens,
            cached_read_tokens,
            cache_creation_tokens,
            reasoning_tokens,
            model_calls,
            api_duration_ms,
            cost_usd_ticks,
            cost_missing_calls,
        } = *t;
        Self {
            input_tokens,
            output_tokens,
            total_tokens: t.total_tokens(),
            cached_read_tokens,
            cache_creation_tokens,
            reasoning_tokens,
            model_calls,
            api_duration_ms,
            cost_usd_ticks,
            cost_is_partial: t.cost_is_partial(),
            cost_missing_calls,
        }
    }
}

#[cfg(feature = "stock-runtime")]
impl From<&xai_chat_state::UsageLedger> for PromptUsage {
    fn from(ledger: &xai_chat_state::UsageLedger) -> Self {
        let mut usage = Self {
            totals: PromptUsageModel::from(&ledger.totals),
            model_usage: ledger
                .by_model
                .iter()
                .map(|(k, v)| (k.clone(), PromptUsageModel::from(v)))
                .collect(),
            num_turns: ledger.main_loop_model_calls,
            usage_is_incomplete: ledger.incomplete,
        };
        usage.scrub_untrustworthy_costs();
        usage
    }
}

/// Server cost scale: 1 USD is 10^10 ticks. ACP exposes ticks; headless converts to float USD.
pub const USD_TICKS_PER_USD: f64 = 1e10;

/// Convert server cost ticks to float USD (headless only).
pub fn ticks_to_usd(ticks: i64) -> f64 {
    ticks as f64 / USD_TICKS_PER_USD
}

/// Converts full ACP input to headless uncached input (`full − cache_read`).
pub fn uncached_input_tokens(full_input: u64, cached_read: u64) -> u64 {
    full_input.saturating_sub(cached_read)
}

/// Project usage onto a headless result object. `usage.input_tokens` is uncached (`full − cache_read − cache_creation`), so the three prompt buckets are disjoint.
/// The identity is `input_tokens + cache_read + cache_creation + output = total_tokens`. Omits all cost floats when partial or incomplete (absent cost does not mean free).
/// Incomplete with no tokens emits only `usage_is_incomplete` (no zero usage object). `modelUsage` rows are a reduced external-compat schema (camelCase; no reasoning/duration).
pub fn project_result_usage(result: &mut serde_json::Value, usage: &PromptUsage) {
    if usage.usage_is_incomplete && usage.is_token_empty() {
        result["usage_is_incomplete"] = true.into();
        return;
    }

    // Exhaustive destructure: a new wire field is a compile error until it is
    // either projected or named as deliberately dropped from the headless shape.
    let PromptUsageModel {
        input_tokens,
        output_tokens,
        total_tokens,
        cached_read_tokens,
        cache_creation_tokens,
        reasoning_tokens,
        model_calls: _,     // totals-level; headless carries num_turns instead
        api_duration_ms: _, // dropped: not part of the frozen headless shape
        cost_usd_ticks,
        cost_is_partial,
        cost_missing_calls: _, // internal partiality count; the flag suffices
    } = usage.totals;
    result["usage"] = serde_json::json!({
        "input_tokens": uncached_input_tokens(input_tokens, cached_read_tokens)
            .saturating_sub(cache_creation_tokens),
        "cache_read_input_tokens": cached_read_tokens,
        "cache_creation_input_tokens": cache_creation_tokens,
        "output_tokens": output_tokens,
        "reasoning_tokens": reasoning_tokens,
        "total_tokens": total_tokens,
    });
    result["num_turns"] = usage.num_turns.into();
    if usage.usage_is_incomplete {
        result["usage_is_incomplete"] = true.into();
    }
    let hide_costs = cost_is_partial || usage.usage_is_incomplete;
    if hide_costs {
        if cost_is_partial {
            result["cost_is_partial"] = true.into();
        }
    } else if let Some(ticks) = cost_usd_ticks {
        result["total_cost_usd"] = serde_json::json!(ticks_to_usd(ticks));
        // Exact integer ticks beside the float, under the same trust gate: reconciliation sums ticks exactly, which floats cannot guarantee
        result["total_cost_usd_ticks"] = serde_json::json!(ticks);
    }
    if !usage.model_usage.is_empty() {
        let mut model_usage = serde_json::Map::new();
        for (name, m) in &usage.model_usage {
            let PromptUsageModel {
                input_tokens,
                output_tokens,
                total_tokens: _, // derivable per row
                cached_read_tokens,
                cache_creation_tokens,
                reasoning_tokens: _, // dropped: reduced per-model schema
                model_calls,
                api_duration_ms: _, // dropped: reduced per-model schema
                cost_usd_ticks,
                cost_is_partial,
                cost_missing_calls: _,
            } = *m;
            let mut entry = serde_json::json!({
                "inputTokens": uncached_input_tokens(input_tokens, cached_read_tokens)
                    .saturating_sub(cache_creation_tokens),
                "outputTokens": output_tokens,
                "cacheReadInputTokens": cached_read_tokens,
                "cacheCreationInputTokens": cache_creation_tokens,
                "modelCalls": model_calls,
            });
            if !hide_costs
                && let Some(ticks) = cost_usd_ticks
                && !cost_is_partial
            {
                entry["costUSD"] = serde_json::json!(ticks_to_usd(ticks));
            }
            model_usage.insert(name.clone(), entry);
        }
        result["modelUsage"] = model_usage.into();
    }
}

/// Fail-closed attach for headless results: parse failure becomes `usage_is_incomplete` (never omit silently; absence must not look free).
pub fn attach_result_usage_fail_closed(result: &mut serde_json::Value, usage: &serde_json::Value) {
    match serde_json::from_value::<PromptUsage>(usage.clone()) {
        Ok(parsed) => project_result_usage(result, &parsed),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "headless: _meta.usage failed to parse; marking usage_is_incomplete"
            );
            result["usage_is_incomplete"] = true.into();
        }
    }
}

/// Status of a single hook run (wire format).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum HookRunStatusDto {
    Success {
        elapsed_ms: u64,
    },
    Skipped,
    Failed {
        error: String,
        elapsed_ms: u64,
        /// Stop-gate block (the hook's decision, not a failure).
        /// Rides `failed` so old pagers keep rendering it. TODO: promote to a dedicated status.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        blocked: bool,
    },
}

/// A single hook run entry (wire format).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HookRunEntryDto {
    pub name: String,
    pub status: HookRunStatusDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// What a `HookAnnotation` is, so the pager can pick the row bullet (the message itself carries none).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookAnnotationKind {
    /// A hook's own message, shown as plain text.
    #[default]
    Note,
    /// A hook's verdict on the tool call above it (a deny): the pager gives it the tool-row bullet.
    ToolOutcome,
}

impl HookAnnotationKind {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Why auto-compaction stopped before completing.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    strum::Display,
    strum::EnumString,
    strum::AsRefStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum AutoCompactCancelReason {
    UserCancelled,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "sessionUpdate")]
pub enum SessionUpdate {
    /// A diff review request containing one or more file diffs for user review.
    DiffReview {
        /// The diff content to be reviewed.
        content: Vec<DiffContent>,
    },
    /// Notification that a retry is in progress due to a transient error.
    RetryState(RetryState),
    /// Auto-compact is starting due to context window threshold
    AutoCompactStarted {
        /// Current token usage
        tokens_used: u64,
        /// Total context window size
        context_window: u64,
        /// Percentage used (e.g., 82)
        percentage: u8,
        /// Reason for compaction
        reason: String,
    },
    /// Auto-compact completed successfully
    AutoCompactCompleted {
        /// Tokens used before compaction. `None` on payloads from older shells.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tokens_before: Option<u64>,
        /// Tokens used after compaction
        tokens_after: u64,
        /// How long the compaction took (milliseconds)
        #[serde(skip_serializing_if = "Option::is_none")]
        elapsed_ms: Option<i64>,
        /// Summary preview (first ~100 chars of summary)
        summary_preview: Option<String>,
    },
    /// Auto-compact failed
    AutoCompactFailed {
        /// Error message
        error: String,
    },
    /// Memory flush is starting before compaction
    MemoryFlushStarted,
    /// Memory flush completed
    MemoryFlushCompleted {
        /// Outcome description
        result: String,
        /// Path to the written memory file (if any)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// Memory dream consolidation completed
    MemoryDreamCompleted {
        /// Outcome description
        result: String,
        /// Path to the written memory file (if any)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        path: Option<String>,
    },
    /// Session-end memory save completed
    MemorySessionSaved {
        /// Path to the written session log
        path: String,
    },
    /// Auto-compact was cancelled (user pressed Ctrl+C)
    AutoCompactCancelled {
        /// Reason for cancellation
        reason: AutoCompactCancelReason,
    },
    /// Auto-continue completed after compaction
    /// This signals the TUI to flush pending agent messages and end the turn
    AutoContinueCompleted {
        /// Total tokens used after auto-continue
        total_tokens: u64,
    },
    /// Request for user feedback based on session heuristics
    FeedbackRequest(FeedbackRequestNotification),
    /// Relay sync status update (connected, disconnected, etc.)
    RelaySyncStatus(RelaySyncStatus),
    /// Auto-recovery is starting after a prompt failure (e.g. remote/workspace recovery)
    AutoRecoveryStarted {
        /// Current recovery attempt number (1-indexed)
        attempt: u32,
        /// Maximum number of recovery attempts allowed
        max_retries: u32,
        /// The error that triggered recovery
        error: String,
        /// Delay in milliseconds before the retry
        delay_ms: u64,
    },
    /// Auto-recovery exhausted all retries and the turn is failing
    AutoRecoveryExhausted {
        /// Total attempts made
        attempts: u32,
        /// The final error message
        error: String,
    },
    /// A hook annotation message for the TUI scrollback.
    /// Rendered inline with the preceding tool call block.
    HookAnnotation {
        /// The hook message, text only: the pager draws the row bullet from `kind`.
        message: String,
        #[serde(default, skip_serializing_if = "HookAnnotationKind::is_default")]
        kind: HookAnnotationKind,
    },
    /// The turn is blocked on an awaited hook batch that just started; sent only when at least one hook will run.
    /// The pager shows a spinner phase once the batch outlives its reveal delay; the matching `HookExecution` or later turn output ends it.
    HookRunStarted {
        event_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        /// Lets the pager keep a late turn-end report (`stop_cancelled` / `stop_failure`) off the next turn's phase.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_id: Option<String>,
        /// Hooks that will run (enabled and matcher-allowed).
        count: usize,
    },
    /// Outcome of a hook batch. Successful runs leave no scrollback trace; the pager renders one line per failed run.
    HookExecution {
        /// The hook event name ("pre_tool_use" or "post_tool_use").
        event_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_name: Option<String>,
        /// Keeps a delayed turn-end batch off the wrong turn's marker.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompt_id: Option<String>,
        runs: Vec<HookRunEntryDto>,
    },
    /// Hooks registry changed (after reload or trust/untrust).
    /// Sent so the pager modal can auto-refresh if open.
    HooksChanged {
        hooks: Vec<xai_hooks_plugins_types::HookInfo>,
        project_trusted: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        load_errors: Vec<String>,
    },
    /// Plugins registry changed (after reload).
    /// Sent so the pager modal can auto-refresh if open.
    PluginsChanged {
        plugins: Vec<xai_hooks_plugins_types::PluginInfo>,
    },
    /// Marketplace plugin updates were auto-installed on session start.
    /// Sent so desktop/pager can show a notification to the user.
    PluginUpdatesInstalled {
        /// List of (plugin_name, old_version, new_version).
        updates: Vec<(String, String, String)>,
    },
    /// Status snapshot for client status lines. Send-only: never persisted, since the next emit supersedes it.
    SessionStatus(Box<xai_grok_status_line::StatusLineContext>),
    /// Session summary was generated for a new session.
    /// Sent after the first user prompt when the LLM generates a title.
    SessionSummaryGenerated {
        /// The generated session summary/title
        session_summary: String,
    },
    /// A short "where was I" recap of the session so far.
    /// The `x.ai/recap` ext method emits it: `/recap` sets `auto = false`, and returning to the terminal after being away sets `auto = true`.
    /// The pager renders it as an informational scrollback line; it is never added to the model conversation.
    SessionRecap {
        /// The one-line recap text (roughly 25 to 40 words; capped at a generous safety limit, so a normal recap is shown in full).
        summary: String,
        /// `true` when generated automatically because the user returned after being away, `false` for an explicit `/recap`.
        #[serde(default)]
        auto: bool,
    },
    /// A manual `/recap` produced no recap: no assistant turns yet, a failed prepare/model call, or an empty summary.
    /// The pager shows a loading spinner for `/recap` and clears it on receipt; without this signal that spinner would animate forever.
    /// Never emitted for an automatic recap (those show no spinner).
    SessionRecapUnavailable,
    /// Ultra-short summary of the just-finished successful turn, generated at turn end for the dashboard row's secondary line. Rows show it until the next successful turn's summary replaces it.
    /// Transient (never persisted to `updates.jsonl`): the durable copy lives in `summary.json` and reaches non-attached clients via the roster.
    /// Generation is serialized shell-side (one in-flight call, aborted by newer turns) and gateway delivery is ordered. So the latest delivery is the latest summary, and clients may apply deliveries directly.
    LastTurnSummary {
        /// One-line fragment (roughly 5 to 12 words, capped at a safety limit).
        summary: String,
        /// Prompt id of the turn this summary describes (provenance; also persisted as `Summary::last_turn_summary_prompt_id`).
        #[serde(default)]
        prompt_id: Option<String>,
    },
    /// A compaction checkpoint marker written to `updates.jsonl`. This is **persist-only**: it is never sent to the gateway/UI. It records that a compaction occurred.
    /// The replay pipeline uses it to reconstruct the model's conversation view when rewinding across the compaction boundary.
    /// The actual compacted conversation is stored under `compaction_checkpoints/{checkpoint_id}.json` to keep `updates.jsonl` lean.
    CompactionCheckpoint(Box<CompactionCheckpointInfo>),
    /// A rewind marker written to `updates.jsonl` when a rewind occurs. This is **persist-only**: it is never sent to the gateway/UI. Because `updates.jsonl` is append-only, rewinding creates a timeline branch.
    /// The marker tells the replay algorithm to discard accumulated state beyond `target_prompt_index` and continue from that point.
    RewindMarker {
        /// The prompt index being rewound to (0-based).
        target_prompt_index: usize,
        /// When the rewind occurred.
        created_at: String,
    },
    /// Task completed notification
    TaskCompleted {
        task_snapshot: TaskSnapshot,
        /// Advisory: an auto-wake prompt follows this completion.
        /// The first-party TUI no longer consumes it; its persistent "watching" status row already shows remaining background work.
        /// It stays for wire compatibility and other clients. Missing reads as `false`.
        #[serde(default)]
        will_wake: bool,
    },
    /// A subagent session has been spawned. Sent on the PARENT session's notification channel so the client knows this `child_session_id` is a subagent and can route its events.
    /// Emitted BEFORE dispatching `SessionCommand::Prompt` to the child. This prevents a race where child events arrive before the client has the session ID mapping.
    SubagentSpawned {
        /// Unique subagent identifier (same as child session ID).
        subagent_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt_id: Option<String>,
        /// The parent session that spawned this subagent.
        parent_session_id: String,
        /// The parent prompt/turn that spawned this subagent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_prompt_id: Option<String>,
        /// The child session's ACP session ID.
        child_session_id: String,
        /// Agent type used for the subagent ("general-purpose", "explore", "plan", or custom).
        subagent_type: String,
        /// Short human-readable description of the task.
        description: String,
        /// Effective context source after bootstrap: "new" or "resumed".
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effective_context_source: Option<String>,
        /// Whether the forked context was normalized into <background_context>.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        context_normalized: bool,
        /// Capability mode applied to this subagent (e.g. "read-only").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        capability_mode: Option<String>,
        /// Named persona applied to this subagent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        persona: Option<String>,
        /// Role that supplied defaults for this subagent (e.g. "researcher").
        #[serde(default, skip_serializing_if = "Option::is_none")]
        role: Option<String>,
        /// Effective model ID used by the subagent (may differ from the parent).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        /// ID of the source subagent this session was resumed from.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resumed_from: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        workflow_run_id: Option<String>,
        /// Live-only opaque child address. Wire key is `agentAddress`; omitted from `updates.jsonl`.
        #[serde(
            default,
            rename = "agentAddress",
            alias = "agent_address",
            skip_serializing_if = "Option::is_none"
        )]
        agent_address: Option<String>,
    },
    /// Periodic progress update for a running subagent. Sent on the PARENT session's notification channel, rate-limited (every ~2s while the subagent is active). Stops automatically when the subagent completes or is cancelled.
    /// The TUI merges these into the same state path used by ACP poll responses.
    SubagentProgress {
        /// Unique subagent identifier.
        subagent_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt_id: Option<String>,
        /// The parent session that owns this subagent.
        parent_session_id: String,
        /// The child session's ACP session ID.
        child_session_id: String,
        /// Elapsed wall-clock time in milliseconds.
        duration_ms: u64,
        /// Number of completed turns so far.
        turn_count: u32,
        /// Total tool calls executed so far.
        tool_call_count: u32,
        /// Current tokens used in the context window.
        tokens_used: u64,
        /// Total context window capacity (tokens).
        context_window_tokens: u64,
        /// Context window usage as a percentage (0-100).
        context_usage_pct: u8,
        /// Distinct tool names called so far.
        tools_used: Vec<String>,
        /// Number of errors encountered so far.
        error_count: u32,
    },
    /// A subagent session has finished (success, failure, or cancellation).
    ///
    /// Sent on the PARENT session's notification channel.
    SubagentFinished {
        /// Unique subagent identifier.
        subagent_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attempt_id: Option<String>,
        /// The child session's ACP session ID.
        child_session_id: String,
        /// Outcome: "completed", "failed", or "cancelled".
        status: String,
        /// Error message if the subagent failed.
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// Number of tool calls made by the subagent.
        tool_calls: u32,
        /// Number of conversation turns taken by the subagent.
        turns: u32,
        /// Total wall-clock duration in milliseconds.
        duration_ms: u64,
        /// Total tokens consumed by the subagent's context window.
        #[serde(default)]
        tokens_used: u64,
        /// Final output text from the subagent (if completed).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        /// Advisory: an auto-wake prompt follows this completion.
        /// The first-party TUI no longer consumes it; its persistent "watching" status row already shows remaining background work.
        /// It stays for wire compatibility and other clients. Missing reads as `false`.
        #[serde(default)]
        will_wake: bool,
    },
    /// A bash command transitioned to background execution.
    /// Sent both for direct `is_background=true` tasks and when a foreground task moves to the background.
    TaskBackgrounded {
        /// The tool_call_id of the bash tool invocation.
        tool_call_id: String,
        /// The background task registry ID.
        task_id: String,
        /// The shell command being executed.
        command: String,
        /// Absolute path of the working directory.
        cwd: String,
        /// Absolute path to the output log file on disk.
        output_file: String,
        /// For monitor tasks: the monitor's human-readable description. `None` for ordinary backgrounded bash commands.
        /// Lets the pager render monitors with a "Monitor" tag instead of bash-highlighting the command string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        monitor_description: Option<String>,
        /// Model-supplied tool `description` for ordinary bash bg tasks (e.g. "Wait for the server to start").
        /// The pager prefers it over the raw `command` in its "Task started" line and tasks pane. `None` when omitted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
    /// Last-wins full list for this session's backgrounded tasks.
    ///
    /// Latest snapshot replaces the previous. `tasks: []` clears Running UI.
    /// Membership is `is_backgrounded` and `owner_session_id` for this session
    /// (`None` owner counts as this session). No stdout; use incrementals or
    /// `get_task_output` for logs.
    ///
    /// `truncated` means the list was fitted to the 32 KiB session_notification
    /// frame. This is still last-wins, not a page: consumers must not treat a
    /// truncated snapshot as the complete set.
    BackgroundTasks {
        tasks: Vec<BackgroundTaskRow>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        truncated: bool,
    },
    ScheduledTaskCreated {
        task_id: String,
        prompt: String,
        human_schedule: String,
        next_fire_at: Option<String>,
    },
    ScheduledTaskFired {
        task_id: String,
        prompt: String,
        human_schedule: String,
        next_fire_at: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent_id: Option<String>,
    },
    /// A scheduled task was deleted/cancelled.
    ScheduledTaskDeleted {
        task_id: String,
        /// `Unknown` on rows persisted before the reason field existed.
        #[serde(default)]
        reason: xai_tool_types::scheduled_task::ScheduledTaskRemovedReason,
    },
    /// A monitor event (stdout line from a monitor background process).
    MonitorEvent {
        task_id: String,
        description: String,
        /// Raw event text (NOT XML-wrapped; for pager stdout display).
        event_text: String,
    },
    /// The session's model was auto-switched because the persisted model is no longer available for this user.
    ModelAutoSwitched {
        /// The model ID that was persisted in the session but is no longer available.
        previous_model_id: String,
        /// The model ID that was selected as a replacement.
        new_model_id: String,
        /// Human-readable reason for the switch.
        reason: String,
    },
    /// The session's model was switched via `session/setModel`. Broadcast to every client subscribed to the session in leader mode.
    /// Follower clients (TUI / IDE / web) mirror the change in their local state: status bar, `/model` dropdown, prompt header, etc.
    /// The originating client also receives this (the leader broadcasts to all subscribers of the session) but skips applying it. Its in-flight `SetSessionModel` response is the authority for its local state and drives the single "Switched to X" scrollback entry. Followers gate on their own `model_switch_pending` flag to distinguish "I'm waiting on my own switch" from "someone else's switch arrived."
    ModelChanged {
        /// The newly-selected model id (catalog key).
        model_id: String,
        /// Effective reasoning effort, post-resolution.
        /// `None` when the model does not support reasoning effort or no effort override was applied.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_effort: Option<String>,
    },
    /// Streaming chunk of a tool call's arguments. Behaves like `acp::SessionUpdate::AgentMessageChunk` / `AgentThoughtChunk`. It flows through the replay buffer and merges with adjacent chunks for the same `tool_call_id`.
    /// It is debounced at the session's buffering interval. Only persisted as a full `acp::SessionUpdate::ToolCall`.
    ToolCallDeltaChunk {
        /// Stable model-provided id (e.g. `"call_abc"`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        /// Positional index assigned within the assistant tool calls.
        tool_index: u32,
        /// Tool name (e.g. `"search_replace"`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Raw JSON-fragment string. NOT valid JSON in isolation.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        arguments_delta: Option<String>,
    },
    /// One or more prompt images were resized to fit within API limits.
    ImageCompressed {
        images: Vec<ImageCompressedEntry>,
        /// Human-readable summary for display.
        message: String,
    },
    /// Prompt images dropped before send (integrity failure or the upscale cap).
    /// The model is told via a system-reminder; this notification shows the drops to the UI.
    ImageDropped { notes: Vec<String> },
    /// Memory file listing for the pager's /memory modal.
    MemoryFiles { files: Vec<MemoryFileInfo> },
    WorkflowUpdated {
        run_id: String,
        #[serde(default)]
        revision: u64,
        name: String,
        objective: String,
        status: String,
        #[serde(default)]
        foreground: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        phases: Vec<WorkflowPhaseInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current_phase: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_budget: Option<u64>,
        #[serde(default)]
        agents_used: u64,
        #[serde(default)]
        agents_reserved: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agents_remaining: Option<u64>,
        #[serde(default)]
        agent_usage_incomplete: bool,
        elapsed_ms: u64,
        #[serde(default)]
        active_agents: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current_agent_label: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        agents: Vec<WorkflowAgentInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_event: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_event_detail: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_event_timestamp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pause_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result_summary: Option<String>,
    },
    /// Goal mode orchestration progress update.
    /// Sent on the parent session's notification channel at phase transitions and rate-limited from the progress handler (max 1/s).
    /// Fire-and-forget to the pager; it requires no action.
    GoalUpdated {
        goal_id: String,
        objective: String,
        /// `"active"`, `"user_paused"`, `"back_off_paused"`, `"no_progress_paused"`, `"infra_paused"`, `"blocked"`, `"budget_limited"`, `"complete"`, `"cleared"`.
        /// Legacy `"doom_loop_paused"` is accepted by pagers as user-paused.
        status: String,
        /// `"idle"`, `"planning"`, `"executing"`
        phase: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        token_budget: Option<i64>,
        #[serde(default)]
        tokens_used: i64,
        elapsed_ms: u64,
        total_deliverables: u32,
        completed_deliverables: u32,
        /// Wire compat: always `None` in the simplified goal model.
        /// Retained for cross-version compatibility with older pagers.
        #[serde(
            rename = "current_deliverable_idx",
            skip_serializing_if = "Option::is_none"
        )]
        current_deliverable_id: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        current_deliverable_title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        current_subagent_role: Option<String>,
        total_worker_rounds: u32,
        total_verify_rounds: u32,
        #[serde(default)]
        token_baseline: i64,
        #[serde(default)]
        finished_subagent_tokens: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        live_subagent_tokens: Option<u64>,
        /// Per-model marginal-token breakdown `(model_id, tokens)`, sorted by tokens descending. The producer (`build_goal_updated`) only populates this when two or more distinct models appear.
        /// A single-model goal collapses to the single tokens line, so the field is empty (and omitted on the wire). The pager re-checks the two-model minimum as defence in depth.
        /// This is a live field for the active-subagent window (it mirrors `live_subagent_tokens` and is cleared on `SubagentFinished`). The pager renders it only under the "Active subagent" block. The producer must keep its populate gate on that same window so the wire and render gates stay aligned.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        live_tokens_by_model: Vec<(String, u64)>,
        #[serde(skip_serializing_if = "Option::is_none")]
        live_context_pct: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        live_turn_count: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        live_tool_call_count: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_event: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_event_detail: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_event_timestamp: Option<String>,
        /// Wire compat: always empty in the simplified goal model.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        deliverables: Vec<GoalDeliverableInfo>,
        /// Human-readable explanation set when the goal entered a paused state with a meaningful reason (today only `"blocked"`). Rendered by the pager under the status row in the goal modal.
        /// Invariant: `Some` iff `status` is a paused-variant string AND the underlying pause was created via the message-carrying path.
        /// The shell clears this on every transition out of a paused state (resume / complete / budget_limit). The pager also gates rendering on `is_paused()` as a defence in depth.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pause_message: Option<String>,
        /// Number of times the goal-achievement classifier has run for this goal.
        /// `None` when no classifier run has occurred yet.
        /// Like `total_worker_rounds`, the field is suppressed while the counter is zero so old pagers don't see a stray zero.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        classifier_runs_attempted: Option<u32>,
        /// Hard cap on classifier runs for this goal. `None` when not configured.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        classifier_max_runs: Option<u32>,
        /// Last aggregate verdict returned by the verification stage, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_classifier_verdict: Option<GoalClassifierVerdict>,
        /// Filesystem path to the most recent verification-stage details artifact.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_classifier_details_path: Option<String>,
        /// `Some(true)` while a classifier run is in flight. Set only by the dedicated "verifying" notification path.
        /// `build_goal_updated` always emits `None` because this flag is not persisted state.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        verifying_completion: Option<bool>,
        /// `Some(true)` while the goal planner subagent is running. Set only by the dedicated "planning" notification path.
        /// `build_goal_updated` always emits `None` because this flag is not persisted state.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        planning: Option<bool>,
    },
    /// A blocking reverse-request (permission / `ask_user_question` / plan-approval) is now **pending** on the agent, keyed by `tool_call_id`.
    /// Fire-and-forget, **never persisted**: it is a request, not a notification.
    /// Subscribers show ⏳ NeedsInput for this session.
    PendingInteraction {
        tool_call_id: String,
        kind: PendingKind,
    },
    /// A previously-pending reverse-request **resolved** (answered, cancelled, or errored).
    /// Fire-and-forget, **never persisted**. Subscribers clear the pending ⏳ for this `tool_call_id`.
    InteractionResolved { tool_call_id: String },
    /// The durable, replayable signal that a turn reached its terminal outcome.
    /// Rides the persisted `_x.ai/session/update` rail, unlike the fire-and-forget `x.ai/session/prompt_complete` notification.
    /// A viewer that re-attaches mid-turn can therefore finalize the turn from replay instead of staying stuck on "Waiting…".
    TurnCompleted {
        /// Correlation key the re-attaching viewer finalizes the turn on: the prompt/turn whose terminal outcome this carries.
        prompt_id: String,
        /// Why the turn ended (the model's stop reason, or e.g. "cancelled").
        stop_reason: String,
        /// Final agent result text, when the turn produced one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_result: Option<String>,
        /// Typed kind of a failed stop (`SamplingErrorKind::as_str()`, e.g. `"max_tokens_truncation"`) for error-specific client copy.
        /// `None` for successes, unkinded errors, and files from older shells.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error_kind: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<PromptUsage>,
        /// Wall-clock turn duration in milliseconds. `None` on old files.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_ms: Option<u64>,
    },
    /// One model response opened (Messages `message_start`), carrying the real message id, model, and input-side token counts. Rides the buffered chunk rail so it is ordered AHEAD of this response's agent chunks.
    /// Headless partial-mode framing consumes it to emit the real `message_start` id and input usage. Without it, framing synthesizes a placeholder id and zero-seeded usage.
    /// Messages backend only; other backends never emit it (the reducer keeps its placeholder fallback there). `input_tokens` is the uncached prompt portion.
    ResponseStarted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default)]
        input_tokens: u64,
        #[serde(default)]
        cache_read_input_tokens: u64,
        #[serde(default)]
        cache_creation_input_tokens: u64,
    },
    /// This response's reasoning (thinking) block finished; carries its encrypted signature. Rides the buffered chunk rail so it is ordered right AFTER this response's thought chunks (and before its text).
    /// Headless partial-mode framing consumes it to emit `signature_delta` before the thinking block's `content_block_stop`, in order. Messages backend only.
    ReasoningCompleted {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    /// One completed model response, so headless can emit a Messages API assistant frame per response.
    /// Ordered with the response's chunks; a tool loop emits several.
    /// The durable outcome rides `TurnCompleted`.
    ResponseCompleted {
        /// Provider message id (Messages `message.id`), when reported.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_id: Option<String>,
        /// Verbatim wire stop reason (`end_turn`, `tool_use`, …), when reported.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<ResponseUsage>,
        /// Reasoning signature (encrypted content) for this response's thinking.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
        /// The provider's matched stop sequence (Messages API `message.stop_sequence`).
        /// Present only when the model stopped on a configured stop sequence; `None` otherwise.
        /// Headless `streaming-messages-json` stamps it onto the assistant frame.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stop_sequence: Option<String>,
    },
    /// Catch-all for unrecognized session update types.
    /// Allows forward/backward compatibility when variants are added or removed.
    /// All fields from the unrecognized variant are discarded during deserialization.
    #[serde(other)]
    Unknown,
}

/// Metadata for a single memory file, sent to the pager for the memory modal.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MemoryFileInfo {
    pub path: String,
    /// `"global"`, `"workspace"`, or `"session"`.
    pub source: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_epoch_secs: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ImageCompressedEntry {
    pub index: usize,
    pub original_bytes: usize,
    pub compressed_bytes: usize,
    pub original_width: u32,
    pub original_height: u32,
    pub compressed_width: u32,
    pub compressed_height: u32,
}

pub const DISK_FULL_ERROR_TYPE: &str = "disk_full";
pub const DISK_FULL_USER_MESSAGE: &str = "Out of disk space. Free some space and try again.";

/// `x.ai/session/prompt_complete` payload key of a failed stop's typed error kind.
/// camelCase like its payload siblings (`stopReason`, `cancelTrigger`). Value: `SamplingErrorKind::as_str()`.
/// The durable twin carries the same value in [`SessionUpdate::TurnCompleted`]'s typed `error_kind` field.
pub const PROMPT_COMPLETE_ERROR_KIND_KEY: &str = "errorKind";

/// `RetryState::Failed.error_type` for a context-window/size overflow.
/// Frozen shell ↔ pager wire value — old pagers match the literal.
pub const CONTEXT_LENGTH_ERROR_TYPE: &str = "context_length";

/// State of a retry operation or error for visual feedback in the TUI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum RetryState {
    /// A retry is in progress
    Retrying {
        /// Current retry attempt number (1-indexed)
        attempt: u32,
        /// Maximum number of retries allowed
        max_retries: u32,
        /// Human-readable reason for the retry
        reason: String,
        /// Sampler error kind when known; absent on old shells and non-sampler paces.
        #[serde(default)]
        error_type: Option<String>,
    },
    /// All retries have been exhausted
    Exhausted {
        /// Total number of attempts made
        attempts: u32,
        /// Human-readable reason for the failure
        reason: String,
        /// True when the exhaustion was caused by an HTTP 429 rate limit.
        /// Clients use this to show a user-friendly upgrade message instead of the raw `reason` string.
        #[serde(default)]
        is_rate_limited: bool,
    },
    /// A non-retryable error occurred (e.g., auth error, invalid params)
    Failed {
        /// Category of the error (e.g., "auth", "invalid_params", "server")
        error_type: String,
        /// Human-readable error message
        message: String,
    },
}

/// Whether a terminal retry failure is a recoverable authentication error (expired/invalid credentials, 401). The user can fix those by signing in again; this drives the actionable re-auth banner.
/// `legacy_auth` is excluded: its message carries its own migration guidance (`grok update` / `grok logout` / `grok login`), shown verbatim.
/// `auth_transient` is excluded for the opposite reason: it is emitted only when the failure self-heals (`AuthManager::requires_manual_reauth`). Its message already says it recovers on its own, so no `/login` banner is shown.
pub fn is_reauthable_failure(error_type: Option<&str>, message: &str) -> bool {
    if matches!(error_type, Some("legacy_auth") | Some("auth_transient")) {
        return false;
    }
    error_type == Some("auth") || message.contains(UNAUTHORIZED_NEEDLE)
}

/// Text fallback for 401s that arrive without a typed `error_type`.
/// Messages lacking this exact literal rely on the typed `error_type == "auth"` alone.
pub const UNAUTHORIZED_NEEDLE: &str = "Unauthorized (401)";

/// Broader sub-needle of [`UNAUTHORIZED_NEEDLE`]; with a known-401 status the pager also matches banner-formatted text.
pub const HTTP_401_NEEDLE: &str = "(401)";

/// Status updates for relay sync (session sharing) feature.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum RelaySyncStatus {
    /// Successfully connected to relay, session is now shareable
    Connected {
        /// The URL where this session can be viewed
        share_url: String,
    },
    /// Disconnected from relay (will auto-reconnect)
    Disconnected,
    /// Reconnecting to relay
    Reconnecting {
        /// Current attempt number
        attempt: u32,
    },
}

/// A diff content item that serializes compatibly with `acp::ToolCallContent::Diff`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "type", rename = "diff")]
pub struct DiffContent {
    /// The diff details.
    #[serde(flatten)]
    pub diff: acp::Diff,
}

/// Notification requesting user feedback based on session heuristics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct FeedbackRequestNotification {
    /// Unique ID for this feedback request
    pub request_id: String,
    /// The tier that triggered this request
    pub tier: String,
    /// Human-readable prompt to show the user
    pub prompt: String,
    /// Whether this is a non-intrusive/dismissible request
    pub dismissible: bool,
    /// Trigger type identifier (e.g., "tier1_engagement", "tier2_complex_recovery")
    pub trigger_type: String,
    /// The specific condition that was met (e.g., "turns >= 10 AND tool_calls >= 5 AND ...")
    pub trigger_condition: String,
    /// Human-readable explanation of what triggered this request with actual values
    pub trigger_reason: String,
    pub stars: bool,
    pub thumbs: bool,
    pub text: bool,
}

// ── Compaction checkpoint types ────────────────────────────────────────

/// Metadata stored in `updates.jsonl` as a `CompactionCheckpoint` session update.
///
/// This is a lightweight reference; the full compacted conversation lives in a separate file (`compaction_checkpoints/{checkpoint_id}.json`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct CompactionCheckpointInfo {
    /// Unique checkpoint identifier (UUID).
    pub checkpoint_id: String,
    /// The prompt index at the time compaction completed.
    /// The next real user prompt will receive this index.
    pub prompt_index_at_compaction: usize,
    /// Relative path to the checkpoint file inside the session directory (e.g., `"compaction_checkpoints/<uuid>.json"`).
    pub checkpoint_file: String,
    /// If this compaction was triggered by auto-compact, contains the auto-continue prompt that was injected after compaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_continue: Option<AutoContinueInfo>,
    /// Schema version for forward compatibility.
    pub schema_version: u32,
    /// ISO 8601 timestamp of when the checkpoint was created.
    pub created_at: String,
}

/// Information about the auto-continue prompt injected after auto-compaction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct AutoContinueInfo {
    /// The exact text of the auto-continue prompt that was added as a user message.
    pub prompt_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalClassifierVerdict {
    Achieved,
    NotAchieved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingKind {
    /// `request_permission` for a tool action.
    Permission,
    /// `x.ai/ask_user_question`.
    Question,
    /// `x.ai/exit_plan_mode` plan approval.
    PlanApproval,
    McpElicitation,
}

/// Client-facing status for one background task in a list snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundTaskStatus {
    Running,
    Completed,
    Failed,
}

/// One background task in a durable list snapshot. No stdout — clients that
/// need output still use incrementals / `get_task_output`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BackgroundTaskRow {
    pub task_id: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub cwd: String,
    pub kind: TaskKind,
    pub status: BackgroundTaskStatus,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
}

impl BackgroundTaskRow {
    pub fn from_snapshot(snapshot: TaskSnapshot) -> Self {
        let status = background_task_status(&snapshot);
        let output_file = {
            let path = snapshot.output_file;
            if path.as_os_str().is_empty() {
                None
            } else {
                Some(path.to_string_lossy().into_owned())
            }
        };
        Self {
            task_id: snapshot.task_id,
            command: snapshot.command,
            display_command: snapshot.display_command,
            description: snapshot.description,
            cwd: snapshot.cwd,
            kind: snapshot.kind,
            status,
            started_at: rfc3339(snapshot.start_time),
            ended_at: snapshot.end_time.map(rfc3339),
            output_file,
            exit_code: snapshot.exit_code,
            signal: snapshot.signal,
        }
    }
}

pub fn background_task_status(snapshot: &TaskSnapshot) -> BackgroundTaskStatus {
    if !snapshot.completed {
        return BackgroundTaskStatus::Running;
    }
    if snapshot.exit_code == Some(0) || (snapshot.exit_code.is_none() && snapshot.signal.is_none())
    {
        BackgroundTaskStatus::Completed
    } else {
        BackgroundTaskStatus::Failed
    }
}

fn rfc3339(time: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}

#[cfg(test)]
mod native_wire_tests {
    use super::*;

    #[test]
    fn unknown_notifications_and_incomplete_usage_stay_fail_closed() {
        let update: SessionUpdate = serde_json::from_value(serde_json::json!({
            "sessionUpdate": "future_update", "payload": "ignored"
        }))
        .unwrap();
        assert!(matches!(update, SessionUpdate::Unknown));
        let reason: xai_tool_types::scheduled_task::ScheduledTaskRemovedReason =
            serde_json::from_str("\"future_reason\"").unwrap();
        assert_eq!(
            reason,
            xai_tool_types::scheduled_task::ScheduledTaskRemovedReason::Unknown
        );
        let mut usage = PromptUsage {
            totals: PromptUsageModel {
                cost_usd_ticks: Some(10),
                ..Default::default()
            },
            usage_is_incomplete: true,
            ..Default::default()
        };
        usage.scrub_untrustworthy_costs();
        assert_eq!(usage.totals.cost_usd_ticks, None);
        let mut result = serde_json::json!({});
        project_result_usage(&mut result, &usage);
        assert_eq!(result["usage_is_incomplete"], true);
        assert!(result.get("total_cost_usd").is_none());
    }
}

impl From<xai_tool_types::workflow::WorkflowUpdate> for SessionUpdate {
    fn from(update: xai_tool_types::workflow::WorkflowUpdate) -> Self {
        let xai_tool_types::workflow::WorkflowUpdate {
            run_id,
            revision,
            name,
            objective,
            status,
            foreground,
            phases,
            current_phase,
            agent_budget,
            agents_used,
            agents_reserved,
            agents_remaining,
            agent_usage_incomplete,
            elapsed_ms,
            active_agents,
            current_agent_label,
            agents,
            last_event,
            last_event_detail,
            last_event_timestamp,
            pause_message,
            result_summary,
        } = update;
        Self::WorkflowUpdated {
            run_id,
            revision,
            name,
            objective,
            status,
            foreground,
            phases,
            current_phase,
            agent_budget,
            agents_used,
            agents_reserved,
            agents_remaining,
            agent_usage_incomplete,
            elapsed_ms,
            active_agents,
            current_agent_label,
            agents,
            last_event,
            last_event_detail,
            last_event_timestamp,
            pause_message,
            result_summary,
        }
    }
}
