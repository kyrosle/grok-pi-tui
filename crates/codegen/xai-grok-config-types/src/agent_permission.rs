//! The complete original role permission mode, distinct from the interactive UI mode.
use serde::Deserialize;

/// Only `BypassPermissions` is wired at spawn; others are forward-compat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, serde::Serialize, strum::EnumCount)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    #[default]
    Default,
    AcceptEdits,
    /// Background classifier reviews tool calls.
    Auto,
    /// Silently deny non-pre-approved tools.
    DontAsk,
    BypassPermissions,
    Plan,
}
impl PermissionMode {
    pub const VALID_VALUES: &[&str] = &[
        "default",
        "acceptEdits",
        "auto",
        "dontAsk",
        "bypassPermissions",
        "plan",
    ];
}
const _: () =
    assert!(PermissionMode::VALID_VALUES.len() == <PermissionMode as strum::EnumCount>::COUNT);
