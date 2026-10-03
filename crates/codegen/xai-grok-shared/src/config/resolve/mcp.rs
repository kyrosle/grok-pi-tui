use toml::Value as TomlValue;

/// Resolve `mcp.liveness_watchers` for a session.
/// Thin wrapper around the canonical [`xai_grok_config_types::resolve_mcp_liveness_watchers`].
/// Pulls each layer from its appropriate TOML / runtime source: | Layer | Source | |--------------|-----------------------------------------------------------------| | requirement | `[features] mcp_liveness_watchers` in `requirements.toml` | | cli | (none — no CLI flag) | | env | `GROK_MCP_LIVENESS_WATCHERS` (handled by `BoolFlag::env`) | | config | `[features] mcp_liveness_watchers` in `~/.grok/config.toml` | | managed | `[features] mcp_liveness_watchers` in `managed_config.toml` | | feature_flag | (none yet — remote settings plumbing TBD) | | default | `true` |
pub fn resolve_mcp_liveness_watchers(
    requirements: Option<&TomlValue>,
    user: Option<&TomlValue>,
    managed: Option<&TomlValue>,
) -> bool {
    fn from_toml(v: Option<&TomlValue>) -> Option<bool> {
        v?.get("features")?.get("mcp_liveness_watchers")?.as_bool()
    }
    xai_grok_config_types::resolve_mcp_liveness_watchers(
        from_toml(requirements),
        /* cli          */ None,
        from_toml(user),
        from_toml(managed),
        /* feature_flag */ None,
    )
    .value
}

/// Resolve `mcp.auto_restart` for a session.
/// Thin wrapper around the canonical [`xai_grok_config_types::resolve_mcp_auto_restart`].
/// Mirrors [`resolve_mcp_liveness_watchers`].
pub fn resolve_mcp_auto_restart(
    requirements: Option<&TomlValue>,
    user: Option<&TomlValue>,
    managed: Option<&TomlValue>,
) -> bool {
    fn from_toml(v: Option<&TomlValue>) -> Option<bool> {
        v?.get("features")?.get("mcp_auto_restart")?.as_bool()
    }
    xai_grok_config_types::resolve_mcp_auto_restart(
        from_toml(requirements),
        /* cli          */ None,
        from_toml(user),
        from_toml(managed),
        /* feature_flag */ None,
    )
    .value
}

/// Resolve `mcp.push_server_status` for a session.
/// Thin wrapper around the canonical [`xai_grok_config_types::resolve_mcp_push_server_status`] that mirrors [`resolve_mcp_liveness_watchers`].
/// Pulls each layer from its TOML / runtime source: | Layer | Source | |--------------|-----------------------------------------------------------------| | requirement | `[features] mcp_push_server_status` in `requirements.toml` | | cli | (none — no CLI flag) | | env | `GROK_MCP_PUSH_SERVER_STATUS` (handled by `BoolFlag::env`) | | config | `[features] mcp_push_server_status` in `~/.grok/config.toml` | | managed | `[features] mcp_push_server_status` in `managed_config.toml` | | feature_flag | (none yet — remote settings plumbing TBD) | | default | `true` |
pub fn resolve_mcp_push_server_status(
    requirements: Option<&TomlValue>,
    user: Option<&TomlValue>,
    managed: Option<&TomlValue>,
) -> bool {
    fn from_toml(v: Option<&TomlValue>) -> Option<bool> {
        v?.get("features")?.get("mcp_push_server_status")?.as_bool()
    }
    xai_grok_config_types::resolve_mcp_push_server_status(
        from_toml(requirements),
        /* cli          */ None,
        from_toml(user),
        from_toml(managed),
        /* feature_flag */ None,
    )
    .value
}

/// Resolve `mcp.recursive_config_watch` for the leader's `ConfigFileWatcher` spawn path.
/// Thin wrapper around the canonical [`xai_grok_config_types::resolve_mcp_recursive_config_watch`].
/// Pulls each layer from its TOML / runtime source: | Layer | Source | |--------------|---------------------------------------------------------------------| | requirement | `[features] mcp_recursive_config_watch` in `requirements.toml` | | cli | (none — no CLI flag) | | env | `GROK_MCP_RECURSIVE_CONFIG_WATCH` (handled by `BoolFlag::env`) | | config | `[features] mcp_recursive_config_watch` in `~/.grok/config.toml` | | managed | `[features] mcp_recursive_config_watch` in `managed_config.toml` | | feature_flag | (none yet — remote settings plumbing TBD) | | default | `true` |
pub fn resolve_mcp_recursive_config_watch(
    requirements: Option<&TomlValue>,
    user: Option<&TomlValue>,
    managed: Option<&TomlValue>,
) -> bool {
    fn from_toml(v: Option<&TomlValue>) -> Option<bool> {
        v?.get("features")?
            .get("mcp_recursive_config_watch")?
            .as_bool()
    }
    xai_grok_config_types::resolve_mcp_recursive_config_watch(
        from_toml(requirements),
        /* cli          */ None,
        from_toml(user),
        from_toml(managed),
        /* feature_flag */ None,
    )
    .value
}
