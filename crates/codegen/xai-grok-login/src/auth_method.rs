//! First-party API-key environment primitives.
//!
//! Only the env-key checks the auth subsystem itself needs live here. The ACP
//! `auth_methods` list-building surface (`build_auth_methods`,
//! `AuthMethodsBuildInputs`, `should_advertise_xai_api_key`, ...) stays in
//! `xai_grok_shell::agent::auth_method`, which depends on shell's `ModelEntry`.

/// Env var that, when set, advertises `xai.api_key` as a viable auth method.
///
/// Kept as a constant so test code and the production check stay in sync.
pub const XAI_API_KEY_ENV_VAR: &str = "XAI_API_KEY";

/// Legacy env var name.
/// Checked as a fallback when `XAI_API_KEY` is not set, so existing deployments that use the old name keep working.
pub const LEGACY_XAI_API_KEY_ENV_VAR: &str = "GROK_CODE_XAI_API_KEY";

/// Read the API key from the environment.
///
/// Checks `XAI_API_KEY` first, then falls back to the legacy `GROK_CODE_XAI_API_KEY` for backward compatibility.
pub fn read_xai_api_key_env() -> Result<String, std::env::VarError> {
    std::env::var(XAI_API_KEY_ENV_VAR).or_else(|_| std::env::var(LEGACY_XAI_API_KEY_ENV_VAR))
}

/// Returns `true` if either `XAI_API_KEY` or `GROK_CODE_XAI_API_KEY` is set.
pub fn has_xai_api_key_env() -> bool {
    read_xai_api_key_env().is_ok()
}

use agent_client_protocol as acp;

pub const XAI_API_KEY_METHOD_ID: &str = "xai.api_key";
pub const CACHED_TOKEN_AUTH_METHOD_ID: &str = "cached_token";
pub const GROK_COM_METHOD_ID: &str = "grok.com";
pub const OIDC_METHOD_ID: &str = "oidc";

/// ACP session auth method. Use `is_session_based_method` for classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethodKind {
    XaiApiKey,
    CachedToken,
    GrokCom,
    Oidc,
    Unknown,
}

impl AuthMethodKind {
    pub fn from_id(id: &acp::AuthMethodId) -> Self {
        match id.0.as_ref() {
            XAI_API_KEY_METHOD_ID => Self::XaiApiKey,
            CACHED_TOKEN_AUTH_METHOD_ID => Self::CachedToken,
            GROK_COM_METHOD_ID => Self::GrokCom,
            OIDC_METHOD_ID => Self::Oidc,
            _ => Self::Unknown,
        }
    }

    /// API key auth: no auth.json, no refresh, no user interaction.
    pub fn is_api_key(self) -> bool {
        matches!(self, Self::XaiApiKey)
    }

    /// `true` for session-based methods (cached_token, grok.com, oidc).
    pub fn is_session_based(self) -> bool {
        matches!(self, Self::CachedToken | Self::GrokCom | Self::Oidc)
    }

    /// Requires user interaction (browser, OIDC redirect, or external auth command).
    pub fn needs_interactive_login(self) -> bool {
        matches!(self, Self::GrokCom | Self::Oidc)
    }
}
