//! Native error copy and ACP wire helpers, without a sampling client.
use agent_client_protocol as acp;
pub use xai_grok_sampling_types::error::*;
pub use xai_grok_sampling_types::error_kind::SamplingErrorKind;

/// ACP error code for rate-limited requests (HTTP 429). Uses the JSON-RPC implementation-defined server error range (-32000 to -32099). Contract: set only for actual HTTP 429 responses from the sampling client.
/// Clients derive user-facing text via [`format_rate_limited_user_message`]. The desktop path (`prompt_complete_fields`) reports the stop reason with no detail.
pub const RATE_LIMITED_ERROR_CODE: i32 = -32003;

/// OAuth / session rate-limit copy (personal plan upgrade path).
pub const RATE_LIMITED_USER_MESSAGE_OAUTH: &str =
    "You\u{2019}ve hit the rate limit for your plan. Upgrade your account or try again later.";

/// API key / team rate-limit copy.
/// Personal grok.com upgrades do not raise API team limits; admins purchase credits or a higher spend-based tier.
/// See https://docs.x.ai/developers/rate-limits#rate-limit-tiers
pub const RATE_LIMITED_USER_MESSAGE_API_KEY: &str = "You\u{2019}ve hit your team\u{2019}s API rate limit. Ask a team admin to purchase more credits for higher limits, or try again later. See https://docs.x.ai/developers/rate-limits#rate-limit-tiers";

/// Well-known free-usage exhaustion code CCP returns on HTTP 429.
/// Matches `prod_util_well_known_errors::SUBSCRIPTION_FREE_USAGE_EXHAUSTED`.
/// sampling-types' `parse_error_bytes` prepends the flat `code` to the flattened message, so this reaches clients embedded in error detail.
pub const FREE_USAGE_EXHAUSTED_ERROR_CODE: &str = "subscription:free-usage-exhausted";

/// User-facing free-usage exhaustion copy (paywall).
/// Promises no reset duration; the backend config drives the quota window.
pub const FREE_USAGE_USER_MESSAGE: &str = "You\u{2019}ve reached your free Grok Build usage limit for now. Get SuperGrok for much higher limits, or try again later: https://grok.com/supergrok?referrer=grok-build";

/// Whether flattened server detail is free-usage-quota exhaustion (paywall), not transient throttling.
/// Sniffs the well-known code embedded by `parse_error_bytes`.
pub fn is_free_usage_exhausted_error(detail: &str) -> bool {
    detail.contains(FREE_USAGE_EXHAUSTED_ERROR_CODE)
}

/// User-facing text for an ACP -32003 rate-limit error. The free-usage code wins first (consumer-only; checked before the API-key rewrite).
/// An API-key caller whose detail pushes the personal SuperGrok upsell gets the team credits copy instead. Otherwise the body is shown after stripping the `API error (status …):` prefix (SamplingError Display).
/// An empty detail falls back to the OAuth or API-key message. Callers that show this in UI should still run their usual sanitizer (scrub/cap).
pub fn format_rate_limited_user_message(
    server_detail: Option<&str>,
    is_api_key_auth: bool,
) -> String {
    // Free-usage sniff works on the prefixed wire string (`contains` the code).
    if server_detail.is_some_and(is_free_usage_exhausted_error) {
        return FREE_USAGE_USER_MESSAGE.to_string();
    }
    if let Some(detail) = server_detail.map(str::trim).filter(|s| !s.is_empty()) {
        let detail = strip_sampling_api_error_prefix(detail);
        if is_api_key_auth && pushes_consumer_subscription_upsell(detail) {
            return RATE_LIMITED_USER_MESSAGE_API_KEY.to_string();
        }
        return detail.to_string();
    }
    if is_api_key_auth {
        RATE_LIMITED_USER_MESSAGE_API_KEY
    } else {
        RATE_LIMITED_USER_MESSAGE_OAUTH
    }
    .to_string()
}

/// Drop `SamplingError::Api`'s Display prefix so users see the IC body, not `API error (status 429 Too Many Requests): …`.
fn strip_sampling_api_error_prefix(detail: &str) -> &str {
    const PREFIX: &str = "API error (status ";
    const SEP: &str = "): ";
    if let Some(rest) = detail.strip_prefix(PREFIX)
        && let Some(idx) = rest.find(SEP)
    {
        return rest[idx + SEP.len()..].trim();
    }
    detail.trim()
}

/// IC sometimes reuses OAuth free-tier upsell copy on 429s ("upgrade to a Grok subscription" / grok.com/supergrok).
/// That is wrong for API-key / team auth: higher limits come from credits and spend-based rate-limit tiers, not a personal SuperGrok plan.
fn pushes_consumer_subscription_upsell(detail: &str) -> bool {
    let d = detail.to_ascii_lowercase();
    d.contains("grok.com/supergrok") || d.contains("upgrade to a grok subscription")
}

/// User-facing copy for capacity/overload failures (stream `overloaded_error`, HTTP 529, proxy-wrapped 5xx).
/// See [`SamplingError::is_overloaded`].
pub const OVERLOADED_USER_MESSAGE: &str = "Model is temporarily overloaded. Try again in a moment.";

pub fn error_data_with_status(message: String, http_status: Option<u16>) -> serde_json::Value {
    match http_status {
        Some(sc) => serde_json::json!({ "message": message, "http_status": sc }),
        None => serde_json::Value::String(message),
    }
}

pub fn local_error(code: &str, message: impl Into<String>) -> acp::Error {
    let mut data = serde_json::json!({ "message": message.into() });
    data[ERROR_CODE_DATA_KEY] = serde_json::json!(code);
    acp::Error::internal_error().data(data)
}

/// `acp::Error.data` key of the typed terminal-error kind marker (stamped by [`terminal_error_data`]).
/// Snake_case like its shipped `data` siblings (`http_status`); frozen wire format.
/// The notification paths carry the kind under their own keys/fields (see `extensions::notification::PROMPT_COMPLETE_ERROR_KIND_KEY`).
const ERROR_KIND_DATA_KEY: &str = "error_kind";

/// `acp::Error.data` key for a local failure's stable `code`; shared by [`local_error`], [`error_code_from_data`], and `session::persistence::io_error_to_acp`.
pub const ERROR_CODE_DATA_KEY: &str = "code";

/// `salvage_cause` values stamped on mid-salvage terminal errors and forwarded onto the `shell.turn.length_empty_continuation` event.
/// EMPTY covers every continuation that cannot be salvaged at the cap: nothing visible, or a truncated tool-call tail.
/// The sampler folds both into `MaxTokensTruncation`; OVERFLOW means the request no longer fit.
pub const SALVAGE_CAUSE_KEY: &str = "salvage_cause";
pub const SALVAGE_CAUSE_EMPTY: &str = "empty_continuation";
pub const SALVAGE_CAUSE_OVERFLOW: &str = "context_overflow";

/// Terminal-failure `acp::Error.data`.
/// Only max-tokens truncation opts into the object shape with an `error_kind` marker.
/// Every other kind keeps the legacy string/status shape because old clients render `data` via `Display` and would show the raw JSON object.
pub fn terminal_error_data(
    message: String,
    http_status: Option<u16>,
    kind: SamplingErrorKind,
) -> serde_json::Value {
    if kind != SamplingErrorKind::MaxTokensTruncation {
        return error_data_with_status(message, http_status);
    }
    let mut data = serde_json::json!({ "message": message });
    data[ERROR_KIND_DATA_KEY] = serde_json::json!(kind.as_ref());
    if let Some(sc) = http_status {
        data["http_status"] = serde_json::json!(sc);
    }
    data
}

/// The raw `error_kind` marker string from `acp::Error.data`, unparsed, for readers with their own vocabulary.
/// The pager maps an unknown kind to its `Other`, keeping it immune to text recovery.
pub fn error_kind_str_from_error(err: &acp::Error) -> Option<&str> {
    err.data.as_ref()?.get(ERROR_KIND_DATA_KEY)?.as_str()
}

pub fn error_code_from_data(err: &acp::Error) -> Option<&str> {
    err.data.as_ref()?.get(ERROR_CODE_DATA_KEY)?.as_str()
}

/// Typed view of [`error_kind_str_from_error`] for the shell's own classification, where an unknown kind degrading to `None` (generic) is correct.
pub fn error_kind_from_error(err: &acp::Error) -> Option<SamplingErrorKind> {
    error_kind_str_from_error(err)?.parse().ok()
}

/// Whether a mapped turn error carries the max-tokens truncation marker.
pub fn is_max_tokens_turn_error(err: &acp::Error) -> bool {
    error_kind_from_error(err) == Some(SamplingErrorKind::MaxTokensTruncation)
}

/// `turn_result.json` stop_reason for a failed turn: "MaxTokens" when the marker is present, else "Error".
/// Matches the success path's `acp::StopReason` names.
pub fn stop_reason_for_turn_error(err: &acp::Error) -> &'static str {
    if is_max_tokens_turn_error(err) {
        "MaxTokens"
    } else {
        "Error"
    }
}

pub fn error_message_from_data(data: &serde_json::Value) -> serde_json::Value {
    data.get("message").cloned().unwrap_or_else(|| data.clone())
}

/// Internal service names that upstream error bodies echo, rewritten to distinct sentence-friendly backend labels before display. The labels stay distinct so a user paste keeps the failing hop.
/// Shared by shell and pager so the redaction cannot drift; apply via [`rewrite_service_names`] (case-insensitive, no cased variants here). No replacement value may re-match a pattern (pinned by test).
pub const SERVICE_NAME_REWRITES: &[(&str, &str)] = &[
    ("cli-chat-proxy", "build backend"),
    ("cli_chat_proxy", "build backend"),
    ("inference-api", "inference backend"),
    ("inference_api", "inference backend"),
    ("research-api", "research backend"),
    ("research_api", "research backend"),
    ("grok-code-backend", "code backend"),
    ("grok_code_backend", "code backend"),
];

/// Scrub every [`SERVICE_NAME_REWRITES`] entry out of `text`, ASCII-case-insensitively (upstream bodies title-case service names).
/// Each replacement keeps its own casing.
pub fn rewrite_service_names(text: &str) -> String {
    let mut result = text.to_owned();
    for (pattern, replacement) in SERVICE_NAME_REWRITES {
        result = replace_ascii_case_insensitive(&result, pattern, replacement);
    }
    result
}

/// ASCII-case-insensitive `replace`.
/// Indices found on the lowercased copy map 1:1 onto `text`: `to_ascii_lowercase` never changes byte lengths.
fn replace_ascii_case_insensitive(text: &str, pattern: &str, replacement: &str) -> String {
    // An empty pattern would never advance `idx`; fail safe in release too.
    if pattern.is_empty() {
        return text.to_owned();
    }
    let lower_text = text.to_ascii_lowercase();
    let lower_pattern = pattern.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut idx = 0;
    while let Some(pos) = lower_text[idx..].find(&lower_pattern) {
        let start = idx + pos;
        out.push_str(&text[idx..start]);
        out.push_str(replacement);
        idx = start + pattern.len();
    }
    out.push_str(&text[idx..]);
    out
}

pub fn error_detail_from_data(data: &serde_json::Value) -> Option<String> {
    if let Some(m) = data.get("message").and_then(|v| v.as_str()) {
        return Some(m.to_owned());
    }
    if let Some(s) = data.as_str() {
        return Some(s.to_owned());
    }
    data.get("detail")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

/// Detail an ACP error carries: `data` via [`error_detail_from_data`], else the JSON-RPC `message`, so foreign shapes classify as the safe `Other`.
pub fn acp_error_message(err: &acp::Error) -> String {
    err.data
        .as_ref()
        .and_then(error_detail_from_data)
        .unwrap_or_else(|| err.message.clone())
}

pub fn http_status_from_error(err: &acp::Error) -> Option<u16> {
    err.data
        .as_ref()?
        .get("http_status")?
        .as_u64()
        .map(|s| s as u16)
}

const PROMPT_USAGE_DATA_KEY: &str = "promptUsage";

pub fn attach_prompt_usage(
    err: acp::Error,
    usage: Option<super::notification::PromptUsage>,
) -> acp::Error {
    let Some(usage) = usage else {
        return err;
    };
    let Ok(usage_val) = serde_json::to_value(&usage) else {
        tracing::warn!(
            "attach_prompt_usage: failed to serialize PromptUsage; leaving error unchanged"
        );
        return err;
    };
    let mut map = match err.data.clone() {
        Some(serde_json::Value::Object(map)) => map,
        Some(serde_json::Value::String(message)) => {
            let mut m = serde_json::Map::new();
            m.insert("message".into(), serde_json::Value::String(message));
            m
        }
        Some(other) => {
            let mut m = serde_json::Map::new();
            m.insert("message".into(), other);
            m
        }
        None => {
            let mut m = serde_json::Map::new();
            m.insert(
                "message".into(),
                serde_json::Value::String(err.message.clone()),
            );
            m
        }
    };
    map.insert(PROMPT_USAGE_DATA_KEY.into(), usage_val);
    err.data(serde_json::Value::Object(map))
}

pub fn prompt_usage_from_error(err: &acp::Error) -> Option<super::notification::PromptUsage> {
    let data = err.data.as_ref()?;
    let raw = data.get(PROMPT_USAGE_DATA_KEY)?;
    serde_json::from_value(raw.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_attachment_preserves_foreign_error_metadata() {
        let usage = super::super::notification::PromptUsage {
            totals: Default::default(),
            model_usage: Default::default(),
            num_turns: 1,
            usage_is_incomplete: true,
        };
        let error = acp::Error::internal_error().data(serde_json::json!({
            "message": "provider failure", "error_kind": "future_kind", "foreign": 42,
        }));
        let error = attach_prompt_usage(error, Some(usage));
        assert_eq!(error_kind_str_from_error(&error), Some("future_kind"));
        assert_eq!(error_kind_from_error(&error), None);
        assert_eq!(error.data.as_ref().unwrap()["foreign"], 42);
        assert_eq!(acp_error_message(&error), "provider failure");
        assert!(prompt_usage_from_error(&error).unwrap().usage_is_incomplete);
    }
}
