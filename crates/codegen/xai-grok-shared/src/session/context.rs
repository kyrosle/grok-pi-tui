//! Context snapshots and token usage rows shared by native clients and producers.

use super::count_detail;
use xai_grok_config::DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT;

/// Itemized token usage for one context category, shown as an informational row in `/context`, e.g. the skills listing or the MCP server listing.
/// Token counts come from rendering the current state (the skill set, the connected servers), never from parsing conversation text.
/// Once injected, these rows overlap [`ContextInfo::message_tokens`]; a fresh session can show rows before the reminders are injected.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TokenUsageCategory {
    /// Display label, e.g. `"Skills"` or `"MCP servers"`.
    pub label: String,
    /// Estimated tokens this category costs in context.
    pub tokens: u64,
    /// By convention a count followed by a noun, e.g. `"21 skills"`; the pager right-aligns the leading count across rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl TokenUsageCategory {
    /// `text` is the canonical render from `SkillManager::listing_snapshot`.
    pub fn skills_listing(text: &str, skill_count: usize) -> Self {
        Self {
            label: "Skills".to_string(),
            tokens: xai_token_estimation::estimate_tokens(text),
            detail: Some(count_detail(skill_count as u64, "skill")),
        }
    }

    /// `text` is the canonical model-facing catalog render.
    pub fn workflows_listing(text: &str, workflow_count: usize) -> Self {
        Self {
            label: "Workflows".to_string(),
            tokens: xai_token_estimation::estimate_tokens(text),
            detail: Some(count_detail(workflow_count as u64, "workflow")),
        }
    }

    /// `text` is the full reminder body for the current server set.
    pub fn mcp_servers(text: &str, server_count: usize) -> Self {
        Self {
            label: "MCP servers".to_string(),
            tokens: xai_token_estimation::estimate_tokens(text),
            detail: Some(count_detail(server_count as u64, "server")),
        }
    }

    /// `text` is the rendered section from `Agent::agents_md_section`.
    pub fn agents_md(text: &str, file_count: usize) -> Self {
        Self {
            label: "AGENTS.md".to_string(),
            tokens: xai_token_estimation::estimate_tokens(text),
            detail: Some(count_detail(file_count as u64, "file")),
        }
    }
}

/// Context usage breakdown for session info.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ContextInfo {
    pub used: u64,
    pub total: u64,
    pub system_prompt_tokens: u64,
    pub tool_definitions_count: u64,
    pub tool_definitions_tokens: u64,
    pub compaction_count: u64,
    pub turn_count: u64,
    pub tool_call_count: u64,
    /// Total conversation items (system + user + assistant + tool responses).
    pub message_count: u64,
    /// Bytes/4 estimate of all non-system conversation items.
    pub message_tokens: u64,
    pub free_tokens: u64,
    pub usage_pct: u8,
    /// The resolved auto-compact threshold percent (0-100) for the active model at the time this snapshot was captured.
    /// Comes from the 6-tier resolution (env > user per-model > user global > GB per-model > GB global > 85).
    /// Used by the TUI `/context` view so the displayed “Auto-compact at X%” matches the actual trigger (e.g. 65 for grok-build in remote settings).
    #[serde(default = "default_auto_compact_threshold")]
    pub auto_compact_threshold_percent: u8,
    /// Itemized usage rows (skills, workflows, MCP servers, AGENTS.md).
    /// Empty on partial snapshots.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub usage_categories: Vec<TokenUsageCategory>,
}

impl ContextInfo {
    /// Partial snapshot from a notification carrying only used and total.
    /// Breakdown fields default to zero until the next full ContextInfo update.
    pub fn from_notification(used: u64, total: u64) -> Self {
        Self {
            used,
            total,
            usage_pct: xai_token_estimation::usage_percentage_u8(used, total),
            free_tokens: xai_token_estimation::free_tokens(total, used),
            auto_compact_threshold_percent: DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT,
            ..Self::default()
        }
    }
}

/// Serde default for the threshold field (keeps old snapshots and partials deserializing without error and gives the historical default of 85).
fn default_auto_compact_threshold() -> u8 {
    DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT
}

#[cfg(test)]
mod tests {
    #[test]
    fn old_snapshot_and_notification_keep_the_existing_defaults() {
        let old: super::ContextInfo = serde_json::from_str(r#"{"used":1,"total":2}"#).unwrap();
        assert_eq!(old.auto_compact_threshold_percent, 85);
        let partial = super::ContextInfo::from_notification(50_000, 200_000);
        assert_eq!(partial.free_tokens, 150_000);
        assert_eq!(partial.usage_pct, 25);
        let empty = super::ContextInfo::from_notification(100, 0);
        assert_eq!(empty.free_tokens, 0);
        assert_eq!(empty.usage_pct, 0);
        let row = super::TokenUsageCategory::skills_listing("skill instructions", 2);
        assert_eq!(row.detail.as_deref(), Some("2 skills"));
        assert_eq!(
            row.tokens,
            xai_token_estimation::estimate_tokens("skill instructions")
        );
    }
}
