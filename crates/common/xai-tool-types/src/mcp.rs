//! Canonical MCP icon, tool delta and server-status wire contracts.

use serde::{Deserialize, Serialize};

/// Max protocol icons kept per server/tool at ingest.
pub const MAX_MCP_ICONS_PER_ENTITY: usize = 8;

/// Max bytes for a single icon `src` (including data URIs) at ingest.
pub const MAX_MCP_ICON_SRC_BYTES: usize = 64 * 1024;

/// Max bytes for a single icon `mime_type` string at ingest.
pub const MAX_MCP_ICON_MIME_TYPE_BYTES: usize = 128;

/// Max size tokens kept per icon (`48x48`, `any`, …) at ingest.
pub const MAX_MCP_ICON_SIZES: usize = 8;

/// Max bytes for a single size token at ingest.
pub const MAX_MCP_ICON_SIZE_TOKEN_BYTES: usize = 32;

/// Wire theme for MCP protocol icons.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpIconTheme {
    Light,
    Dark,
    #[serde(other)]
    Unknown,
}

/// ACP-facing MCP protocol icon (SEP-973), mirrored from rmcp so clients never depend on the quarantined SDK types.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpIcon {
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<McpIconTheme>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerSource {
    Managed,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub icons: Vec<McpIcon>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpToolsChanged {
    /// Session that owns this push.
    /// The pager routes via `find_session_match` so a background-agent push does not land on the foregrounded agent's modal.
    pub session_id: String,
    /// MCP server whose tool list changed. Currently unread by the pager. The pager treats every `tools_changed` push as a trigger to schedule a debounced `mcp/list` refetch and re-reads the full catalog.
    /// The toggle-tool path therefore leaves this empty for forward-compat. A future field-aware pager optimization would need to special-case empty as "not scoped to one server"; no consumer reads that today.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub server_name: String,
    /// New tool entries for the named server. Currently unread by the pager for the same reason as `server_name` above. Empty on the toggle-tool path.
    /// Populated on the post-handshake and auth-recovery paths so future field-aware consumers can avoid the `mcp/list` round trip.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<McpToolEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpSessionStatus {
    Ready,
    Initializing,
    SetupRequired,
    Unavailable,
}

/// Method name for the ACP push.
pub const SERVER_STATUS_METHOD: &str = "x.ai/mcp/server_status";

/// JSON payload pushed over ACP. Fields written in camelCase per ACP convention.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatusPayload {
    pub session_id: String,
    /// MCP server name (`managed_gateway:linear`, `github`, ...).
    pub name: String,
    /// `managed` for gateway catalog ids (`managed_gateway:*`), else `local`.
    pub source: McpServerSource,
    pub status: McpServerStatus,
    pub reason: McpServerStatusReason,
    /// Optional human-readable detail.
    /// Passes the full handshake / transport error reason to the UI verbatim (no sanitization or truncation) so failures are easy to debug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Reserved for future use; always `null` today.
    /// It may later carry the post-restart tool list so the client can re-render without a follow-up `mcp/list` round-trip.
    pub tools: Option<serde_json::Value>,
}

/// Status enum sent on the wire. Lowercase serialization to match the existing pager `McpSessionStatus` family.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerStatus {
    /// Client is in [`xai_grok_mcp::servers::ClientStateKind::Ready`] and the transport is healthy.
    Ready,
    /// Per-server handshake is in flight, or a restart is being debounced.
    Initializing,
    /// Transport closed, handshake failed, or the server is disabled/unconfigured.
    Unavailable,
    /// OAuth required but not yet acquired.
    NeedsAuth,
}

/// Reason a status delta was emitted. Lowercase, snake_case serialization to keep the wire schema stable.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpServerStatusReason {
    TransportClosed,
    HandshakeFailed,
    ConfigAdded,
    ConfigRemoved,
    ConfigChanged,
    Disabled,
    AuthExpired,
    /// First-time successful handshake (a new server transitioned from `Initializing` to `Ready`).
    /// Every `McpClientEvent::Ready` maps to this reason.
    Initialized,
    /// A watcher fired `TransportClosed`, the auto-restart path re-handshook, and the new handshake succeeded.
    RestartSucceeded,
    /// The auto-restart path exhausted retries.
    RestartFailed,
    /// Old leaders still emit this after reactive reauth. Not produced anymore.
    ManagedTokenRefreshed,
}
