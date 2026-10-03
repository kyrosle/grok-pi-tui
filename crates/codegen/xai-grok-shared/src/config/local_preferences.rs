use toml::Value as TomlValue;
/// Returns `None` when `[cli] use_leader` is not set in the config, or `Some(true/false)` when explicitly configured.
/// The `None` lets callers fall through to a remote flag when the user hasn't expressed a local preference.
pub fn use_leader_from_toml_opt(root: &TomlValue) -> Option<bool> {
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(cli)) = table.get("cli")
    {
        cli.get("use_leader").and_then(|v| v.as_bool())
    } else {
        None
    }
}

/// When true, the agent will connect to a shared leader process instead of running the agent directly.
/// This allows multiple agent instances to share one backend.
pub fn use_leader_from_toml(root: &TomlValue) -> bool {
    use_leader_from_toml_opt(root).unwrap_or(false)
}

/// Returns `Some(true/false)` when `[cli] session_registry` is set in config.toml, `None` when absent (allowing remote settings fallback).
/// Local config takes precedence over remote settings.
pub fn session_registry_from_toml_opt(root: &TomlValue) -> Option<bool> {
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(cli)) = table.get("cli")
    {
        cli.get("session_registry").and_then(|v| v.as_bool())
    } else {
        None
    }
}

/// Overrides `[cli] session_registry`; usable before `~/.grok/config.toml` exists.
pub const SESSION_REGISTRY_ENV_VAR: &str = "GROK_SESSION_REGISTRY";

pub fn session_registry_from_env_opt() -> Option<bool> {
    xai_grok_config::env_bool(SESSION_REGISTRY_ENV_VAR)
}

/// Where a local session-registry override came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrySource {
    /// [`SESSION_REGISTRY_ENV_VAR`].
    Env,
    /// `[cli] session_registry` in config.toml.
    ConfigToml,
}

impl RegistrySource {
    /// The user-facing name of this source, for diagnostics.
    pub const fn label(self) -> &'static str {
        match self {
            RegistrySource::Env => SESSION_REGISTRY_ENV_VAR,
            RegistrySource::ConfigToml => "[cli] session_registry",
        }
    }
}

/// Env var, then `[cli] session_registry`; `None` defers to remote settings.
pub fn session_registry_local_override_sourced(
    root: Option<&TomlValue>,
) -> Option<(bool, RegistrySource)> {
    if let Some(v) = session_registry_from_env_opt() {
        return Some((v, RegistrySource::Env));
    }
    root.and_then(session_registry_from_toml_opt)
        .map(|v| (v, RegistrySource::ConfigToml))
}

pub fn session_registry_local_override(root: Option<&TomlValue>) -> Option<bool> {
    session_registry_local_override_sourced(root).map(|(v, _)| v)
}
