use agent_client_protocol as acp;

/// Stable id for the "enable always-approve mode" option prepended for TUI / Pager / Desktop clients.
/// Shell maps it to [`PromptOutcome::AllowOnce`] and persists nothing; the pager separately fires `set_yolo_mode(true)`.
/// Keeps the wire plain ACP. An unrecognized client still gets `AllowOnce`; worst case the current call is granted but the toggle does not flip.
pub const ENABLE_ALWAYS_APPROVE_OPTION_ID: &str = "enable-always-approve";

/// Canonical check for the "enable always-approve mode" option; match on this, not the label or position 0.
pub fn is_enable_always_approve_option(opt: &acp::PermissionOption) -> bool {
    opt.option_id.0.as_ref() == ENABLE_ALWAYS_APPROVE_OPTION_ID
}

pub mod bash_command_splitting;
pub mod bash_patterns;
pub mod bash_scope;
pub mod exec_risk;
mod mcp;
pub use bash_patterns::{
    bash_glob_is_catchall, bash_pattern_is_broad, bash_pattern_matches_command,
};
pub use bash_scope::{
    always_allow_scope_persists, default_always_allow_scope, default_always_deny_scope,
    minimum_always_allow_scope,
};
pub use mcp::*;
pub const ALLOW_EDITS_SESSION_OPTION_ID: &str = "allow-edits-session";
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookAsk {
    pub hook_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
pub const HOOK_ASK_META_KEY: &str = "hookAsk";
const HOOK_ASK_SEPARATOR: &str = " — ";
impl HookAsk {
    pub fn ask_line(&self) -> String {
        let hook_name = &self.hook_name;
        let reason = self.reason.as_deref().unwrap_or_default();
        let reason = reason.split_whitespace().collect::<Vec<_>>().join(" ");
        if reason.is_empty() {
            format!("hook '{hook_name}' asks for confirmation")
        } else {
            format!("hook '{hook_name}' asks: {reason}")
        }
    }
    pub fn prompt_header(&self, action: &str) -> String {
        format!("{action}{HOOK_ASK_SEPARATOR}{}", self.ask_line())
    }
    pub fn strip_prompt_header<'a>(&self, title: &'a str) -> &'a str {
        title
            .strip_suffix(self.ask_line().as_str())
            .and_then(|action| action.strip_suffix(HOOK_ASK_SEPARATOR))
            .unwrap_or(title)
    }
}
/// Why acceptEdits must still prompt for this edit target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectedEditReason {
    HookRoot,
    GitHooks,
    Ssh,
    StartupFile,
    Etc,
    GrokConfig,
    GrokSandbox,
    ClaudeSettings,
    CursorHooks,
    /// Fail-closed / unclassified sensitive path; no user copy yet.
    Sensitive,
}

impl ProtectedEditReason {
    pub fn kind(self) -> &'static str {
        match self {
            Self::HookRoot => "hook_root",
            Self::GitHooks => "git_hooks",
            Self::Ssh => "ssh",
            Self::StartupFile => "startup_file",
            Self::Etc => "etc",
            Self::GrokConfig => "grok_config",
            Self::GrokSandbox => "grok_sandbox",
            Self::ClaudeSettings => "claude_settings",
            Self::CursorHooks => "cursor_hooks",
            Self::Sensitive => "sensitive",
        }
    }

    pub fn description(self) -> Option<&'static str> {
        match self {
            Self::HookRoot => Some(
                "Note: This edit contains changes to hooks, which can be executed as code on later sessions without a separate execution approval.",
            ),
            Self::GitHooks => Some(
                "Note: This edit contains changes to Git hooks, which can run automatically on commit, push, or other Git actions without a separate execution approval.",
            ),
            Self::Ssh => Some(
                "Note: This edit contains changes under `.ssh`, which can affect credentials and authentication for future sessions.",
            ),
            Self::StartupFile => Some(
                "Note: This edit contains changes to a shell startup file, which can run automatically in future terminals without a separate execution approval.",
            ),
            Self::Etc => Some(
                "Note: This edit contains changes under `/etc`, which is system configuration and can affect this machine beyond the current project.",
            ),
            Self::GrokConfig => Some(
                "Note: This edit contains changes to Grok config, which can alter permissions, tools, and other behavior in later sessions.",
            ),
            Self::GrokSandbox => Some(
                "Note: This edit contains changes to the Grok sandbox config, which can loosen filesystem and network restrictions on commands.",
            ),
            Self::ClaudeSettings => Some(
                "Note: This edit contains changes to Claude-compatible settings, which can install hooks or change permission mode without a separate execution approval.",
            ),
            Self::CursorHooks => Some(
                "Note: This edit contains changes to Cursor hooks, which can run automatically in later sessions without a separate execution approval.",
            ),
            Self::Sensitive => None,
        }
    }
}

/// ACP `_meta` payload for protected-edit prompts (pager reads this for description).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectedEditPermission {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl ProtectedEditPermission {
    pub fn from_reason(reason: ProtectedEditReason) -> Self {
        Self {
            kind: reason.kind().to_owned(),
            description: reason.description().map(str::to_owned),
        }
    }
}
