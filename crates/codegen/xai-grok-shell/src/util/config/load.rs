use super::mcp::*;
use toml::Value as TomlValue;
/// Resolve a bool from an optional env var, then config.toml `[section] key`, then false.
fn toml_bool_sync(env_var: Option<&str>, section: &str, key: &str) -> bool {
    if let Some(var) = env_var
        && let Some(val) = crate::agent::config::env_bool(var)
    {
        return val;
    }
    let root: TomlValue = match crate::config::load_effective_config() {
        Ok(r) => r,
        Err(_) => return false,
    };
    if let TomlValue::Table(table) = root
        && let Some(TomlValue::Table(s)) = table.get(section)
    {
        s.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
    } else {
        false
    }
}
pub(crate) fn load_relay_sync_enabled_sync() -> bool {
    toml_bool_sync(Some("GROK_RELAY_SYNC_ENABLED"), "relay", "enabled")
}
const DEFAULT_FLUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
pub(crate) fn load_upload_wait_config_sync() -> (bool, std::time::Duration) {
    let root: TomlValue = match crate::config::load_effective_config() {
        Ok(r) => r,
        Err(_) => return (false, DEFAULT_FLUSH_TIMEOUT),
    };
    let harness = match &root {
        TomlValue::Table(table) => table.get("harness"),
        _ => None,
    };
    let wait_for_uploads = harness
        .and_then(|h| {
            h.get("wait_for_uploads")
                .or_else(|| h.get("block_for_upload"))
        })
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let flush_timeout = harness
        .and_then(|h| h.get("upload_flush_timeout_secs"))
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or(DEFAULT_FLUSH_TIMEOUT);
    (wait_for_uploads, flush_timeout)
}
pub async fn load_config() -> Config {
    let root: TomlValue = match crate::config::load_effective_config() {
        Ok(v) => v,
        Err(_) => return Config::default(),
    };
    load_config_from_toml(&root)
}
pub use xai_grok_shared::config::load_config_from_toml;
#[cfg(test)]
mod tests {
    use super::*;
    use toml::Value as TomlValue;
    #[test]
    fn test_models_default_parsing() {
        let toml_str = r#"
[models]
default = "grok-code-fast-1"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        if let TomlValue::Table(table) = root
            && let Some(TomlValue::Table(models)) = table.get("models")
        {
            let default = models
                .get("default")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            assert_eq!(default.as_deref(), Some("grok-code-fast-1"));
        } else {
            panic!("Expected models table");
        }
    }
    #[test]
    fn test_relay_sync_enabled_true() {
        let toml_str = r#"
[relay]
enabled = true
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        if let TomlValue::Table(table) = root
            && let Some(TomlValue::Table(relay)) = table.get("relay")
        {
            let enabled = relay
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            assert!(enabled);
        } else {
            panic!("Expected relay table");
        }
    }
    #[test]
    fn test_relay_sync_enabled_false() {
        let toml_str = r#"
[relay]
enabled = false
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        if let TomlValue::Table(table) = root
            && let Some(TomlValue::Table(relay)) = table.get("relay")
        {
            let enabled = relay
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            assert!(!enabled);
        } else {
            panic!("Expected relay table");
        }
    }
    #[test]
    fn test_relay_sync_default_false() {
        let toml_str = r#"
[relay]
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        if let TomlValue::Table(table) = root
            && let Some(TomlValue::Table(relay)) = table.get("relay")
        {
            let enabled = relay
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            assert!(!enabled);
        }
    }
    #[test]
    fn test_relay_sync_no_section() {
        let toml_str = r#"
[models]
default = "grok-code-fast-1"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        if let TomlValue::Table(table) = root {
            let has_relay = table.get("relay").is_some();
            assert!(!has_relay);
        }
    }
    #[test]
    fn test_relay_sync_config_struct() {
        let config = RelaySyncConfig {
            enabled: Some(true),
        };
        assert_eq!(config.enabled, Some(true));
        let config_disabled = RelaySyncConfig {
            enabled: Some(false),
        };
        assert_eq!(config_disabled.enabled, Some(false));
        let config_default = RelaySyncConfig::default();
        assert_eq!(config_default.enabled, None);
    }
}
