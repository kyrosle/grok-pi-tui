//! Existing plugin CTA dismissal preferences, using the product-scoped config store.

/// Locked read-modify-write of `~/.grok/config.toml`: the whole window runs under the config-init
/// flock and lands via atomic replace; unchanged configs skip the write.
pub fn update_config_toml_locked(
    grok_home: &std::path::Path,
    mutate: impl FnOnce(&mut toml::value::Table) -> Result<bool, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = grok_home.join("config.toml");
    let _flock = super::persist::acquire_init_lock(grok_home)?;
    let content = super::persist::read_to_string_or_empty(&config_path)?;
    let mut config: toml::Value = if content.is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(&content).map_err(|e| format!("failed to parse config.toml: {e}"))?
    };
    let table = config
        .as_table_mut()
        .ok_or("config.toml root is not a table")?;
    if !mutate(table)? {
        return Ok(());
    }
    super::persist::atomic_write_string(&config_path, &toml::to_string_pretty(&config)?)?;
    Ok(())
}
/// Run one `update_config_toml_locked` writer on the blocking pool: the flock poll blocks, so
/// LocalSet callers must hop here. Errors are stringified to cross the spawn boundary.
pub async fn config_write_blocking<F>(write: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
{
    tokio::task::spawn_blocking(move || write().map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("config write task failed: {e}"))?
}
/// Async [`add_dismissed_plugin_cta`] for UI callers (see [`config_write_blocking`]): the locked
/// write sleep-polls the config-init flock, so it must stay off the render path.
pub async fn run_add_dismissed_plugin_cta(plugin_id: String) -> Result<(), String> {
    config_write_blocking(move || add_dismissed_plugin_cta(&plugin_id)).await
}
/// Add a plugin to `[plugin_cta].dismissed` in `~/.grok/config.toml`.
/// Creates the `[plugin_cta]` section and `dismissed` array if they don't exist.
/// Deduplicates: if already present, this is a no-op.
pub fn add_dismissed_plugin_cta(plugin_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = xai_grok_config::grok_home().join("config.toml");
    add_dismissed_plugin_cta_to_file(plugin_id, &config_path)
}
/// Add a dismissed plugin CTA to a specific config file (path-parameterized for tests); runs
/// under the config-init flock with an atomic replace like every config.toml writer.
#[doc(hidden)]
pub fn add_dismissed_plugin_cta_to_file(
    plugin_id: &str,
    config_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let grok_home = config_path
        .parent()
        .ok_or("config.toml path has no parent directory")?;
    update_config_toml_locked(grok_home, |table| {
        let plugin_cta = table
            .entry("plugin_cta".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .ok_or("[plugin_cta] is not a table")?;
        let dismissed = plugin_cta
            .entry("dismissed".to_string())
            .or_insert_with(|| toml::Value::Array(vec![]))
            .as_array_mut()
            .ok_or("[plugin_cta].dismissed is not an array")?;
        if dismissed
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s == plugin_id))
        {
            return Ok(false);
        }
        dismissed.push(toml::Value::String(plugin_id.to_string()));
        Ok(true)
    })
}
/// All plugin ids listed in `[plugin_cta].dismissed` in `~/.grok/config.toml`.
///
/// Read once (e.g. on catalog load) and cached so the matched-debounce recompute doesn't parse the config from disk on the UI thread.
pub fn dismissed_plugin_ctas() -> std::collections::HashSet<String> {
    let config_path = xai_grok_config::grok_home().join("config.toml");
    dismissed_plugin_ctas_in_file(&config_path)
}
/// Read the dismissed plugin CTA set from a specific config file (for tests).
#[doc(hidden)]
pub fn dismissed_plugin_ctas_in_file(
    config_path: &std::path::Path,
) -> std::collections::HashSet<String> {
    let Ok(content) = std::fs::read_to_string(config_path) else {
        return std::collections::HashSet::new();
    };
    let Ok(config) = toml::from_str::<toml::Value>(&content) else {
        return std::collections::HashSet::new();
    };
    config
        .as_table()
        .and_then(|t| t.get("plugin_cta"))
        .and_then(|v| v.as_table())
        .and_then(|t| t.get("dismissed"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_dismissed_plugin_cta_creates_table() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        add_dismissed_plugin_cta_to_file("figma", &config_path).unwrap();
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[plugin_cta]"));
        assert!(content.contains("figma"));
        assert!(dismissed_plugin_ctas_in_file(&config_path).contains("figma"));
    }
    #[test]
    fn add_dismissed_plugin_cta_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        add_dismissed_plugin_cta_to_file("notion", &config_path).unwrap();
        add_dismissed_plugin_cta_to_file("notion", &config_path).unwrap();
        let config: toml::Value =
            toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        let dismissed = config
            .get("plugin_cta")
            .and_then(|v| v.get("dismissed"))
            .and_then(|v| v.as_array())
            .unwrap();
        let count = dismissed
            .iter()
            .filter(|v| v.as_str() == Some("notion"))
            .count();
        assert_eq!(count, 1);
    }
    #[test]
    fn dismissed_plugin_ctas_reflects_added_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        assert!(!dismissed_plugin_ctas_in_file(&config_path).contains("figma"));
        add_dismissed_plugin_cta_to_file("figma", &config_path).unwrap();
        let dismissed = dismissed_plugin_ctas_in_file(&config_path);
        assert!(dismissed.contains("figma"));
        assert!(!dismissed.contains("notion"));
    }
    #[test]
    fn add_dismissed_plugin_cta_preserves_other_config() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        std::fs::write(&config_path, "[plugins]\ndisabled = [\"keep-me\"]\n").unwrap();
        add_dismissed_plugin_cta_to_file("figma", &config_path).unwrap();
        let config: toml::Value =
            toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(
            config
                .get("plugins")
                .and_then(|v| v.get("disabled"))
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str()),
            Some("keep-me"),
        );
        assert!(dismissed_plugin_ctas_in_file(&config_path).contains("figma"));
    }
}
