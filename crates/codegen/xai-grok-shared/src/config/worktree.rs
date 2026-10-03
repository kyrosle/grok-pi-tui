use super::RemoteSettings;
use toml::Value as TomlValue;
pub use xai_grok_config_types::WorktreeType;
/// Returns `Some(type)` when `[cli] worktree_type` is set to a valid value in config.toml,
/// `None` when absent or the value type is wrong. Logs a warning for invalid strings.
pub fn worktree_type_from_toml_opt(root: &TomlValue) -> Option<WorktreeType> {
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(cli)) = table.get("cli")
        && let Some(toml_value) = cli.get("worktree_type")
    {
        if let Some(type_str) = toml_value.as_str() {
            return match type_str.parse::<WorktreeType>() {
                Ok(wt) => Some(wt),
                Err(()) => {
                    tracing::warn!("Invalid worktree_type value in config: {type_str}, ignoring");
                    None
                }
            };
        }
        tracing::warn!("Invalid worktree_type value in config: {toml_value:?}, ignoring");
    }
    None
}

pub fn worktree_type_from_toml(root: &TomlValue) -> WorktreeType {
    worktree_type_from_toml_opt(root).unwrap_or_default()
}

/// Resolve worktree type: local config > remote settings > default (`Linked`).
pub fn resolve_worktree_type(
    raw_config: &TomlValue,
    remote: Option<&RemoteSettings>,
) -> (WorktreeType, &'static str) {
    if let Some(wt) = worktree_type_from_toml_opt(raw_config) {
        return (wt, "local");
    }
    if let Some(s) = remote.and_then(|r| r.worktree_type.as_deref()) {
        match s.parse::<WorktreeType>() {
            Ok(wt) => return (wt, "remote"),
            Err(()) => {
                tracing::warn!("Invalid remote worktree_type: {s}, using default");
            }
        }
    }
    (WorktreeType::default(), "default")
}

pub fn worktree_type() -> WorktreeType {
    let root: TomlValue = match xai_grok_config::load_effective_config_disk_only() {
        Ok(r) => r,
        Err(_) => return WorktreeType::Linked,
    };
    worktree_type_from_toml(&root)
}

/// Env override for grove vs copy (`grove` | `grove-fuse` | `grove-nfs` | `nfs` | `copy`).
/// Distinct from [`WorktreeType`] (`linked` | `standalone` | `git`).
pub const ENV_WORKTREE_TYPE: &str = "GROK_WORKTREE_TYPE";

fn grove_from_str(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "grove" | "grove-fuse" | "grove-nfs" | "nfs" | "true" | "1" | "on" => Some(true),
        "copy" | "false" | "0" | "off" => Some(false),
        _ => None,
    }
}

fn grove_worktree_from_toml_opt(root: &TomlValue) -> Option<bool> {
    let cli = root.get("cli")?;
    for key in ["grove_worktree", "nfs_worktree"] {
        if let Some(v) = cli.get(key) {
            if let Some(b) = v.as_bool() {
                return Some(b);
            }
            if let Some(s) = v.as_str() {
                return grove_from_str(s);
            }
            tracing::warn!("Invalid [cli].{key} value: {v:?}, ignoring");
        }
    }
    if let Some(s) = cli.get("worktree_type").and_then(|v| v.as_str()) {
        match s {
            "grove" | "grove-fuse" | "grove-nfs" | "nfs" => return Some(true),
            "copy" => return Some(false),
            _ => {}
        }
    }
    None
}

/// The kill switch and a missing remote run **last** and fail **closed**.
/// `remote = None` forces copy; `grove_worktree = false` forces copy even when `desired`, env, or local asked for grove.
pub fn resolve_grove_worktree(
    raw_config: &TomlValue,
    remote: Option<&RemoteSettings>,
) -> (bool, &'static str) {
    gate_grove_worktree(None, raw_config, remote)
}

pub fn grove_worktree_env() -> Option<bool> {
    std::env::var(ENV_WORKTREE_TYPE)
        .ok()
        .and_then(|s| grove_from_str(&s))
}

/// Resolves in layer order: request, then env, then local, then remote (which only enables when true); the kill switch runs last.
/// Tests inject `env` instead of mutating process state.
pub fn gate_grove_worktree_layers(
    desired: Option<bool>,
    env: Option<bool>,
    raw_config: &TomlValue,
    remote: Option<&RemoteSettings>,
) -> (bool, &'static str) {
    let mut enabled = false;
    let mut src = "default";
    if let Some(v) = desired {
        enabled = v;
        src = "request";
    } else if let Some(v) = env {
        enabled = v;
        src = "env";
    } else if let Some(v) = grove_worktree_from_toml_opt(raw_config) {
        enabled = v;
        src = "local";
    } else if remote.and_then(|r| r.grove_worktree) == Some(true) {
        enabled = true;
        src = "remote";
    }
    let refusal = match remote {
        None => Some("remote_unavailable"),
        Some(r) if r.grove_worktree == Some(false) => Some("remote_kill"),
        _ => None,
    };
    match refusal {
        // A refusal is only the *reason* when a layer actually asked for grove;
        // otherwise callers would read `remote_unavailable` (returned on every
        // settings-fetch failure) as proof of a grove request that never happened.
        Some(reason) => (false, if enabled { reason } else { src }),
        None => (enabled, src),
    }
}

/// Single grove-vs-copy gate. `desired` is an explicit client/resume flag.
pub fn gate_grove_worktree(
    desired: Option<bool>,
    raw_config: &TomlValue,
    remote: Option<&RemoteSettings>,
) -> (bool, &'static str) {
    gate_grove_worktree_layers(desired, grove_worktree_env(), raw_config, remote)
}

pub fn grove_worktree_enabled(remote: Option<&RemoteSettings>) -> bool {
    let root: TomlValue = match xai_grok_config::load_effective_config_disk_only() {
        Ok(r) => r,
        Err(_) => TomlValue::Table(toml::map::Map::new()),
    };
    gate_grove_worktree(None, &root, remote).0
}

pub fn restore_code_from_toml(root: &TomlValue) -> Option<bool> {
    root.get("cli")
        .and_then(|c| c.get("restore_code"))
        .and_then(|v| v.as_bool())
}

/// Used when the client omits `restoreCode` on the wire.
pub fn resolve_restore_code(raw_config: &TomlValue, remote: Option<&RemoteSettings>) -> bool {
    restore_code_from_toml(raw_config)
        .or(remote.and_then(|r| r.restore_code))
        .unwrap_or(false)
}
