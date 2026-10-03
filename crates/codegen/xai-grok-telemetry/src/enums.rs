//! Telemetry payload structs in this crate reference these enums, so they live here.
//! `xai-grok-shell` re-exports them from their original paths (`session::mcp_servers`, `util::config`) to keep callers unchanged.

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpInitStrategy {
    /// Wait for MCP initialization before first LLM call
    #[default]
    Blocking,
    /// Start immediately, advertise tools as they become available
    Progressive,
}

impl<S: AsRef<str>> From<S> for McpInitStrategy {
    fn from(s: S) -> Self {
        match s.as_ref() {
            "progressive" => McpInitStrategy::Progressive,
            _ => McpInitStrategy::Blocking,
        }
    }
}

/// Shared between the shell's session signals (`turn_result.json`) and the `pr_created` telemetry event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrCreationSource {
    /// `gh pr create` via the bash tool.
    Bash,
    /// An MCP `create_pull_request` tool.
    Mcp,
}

pub use xai_grok_config_types::PermissionMode;
