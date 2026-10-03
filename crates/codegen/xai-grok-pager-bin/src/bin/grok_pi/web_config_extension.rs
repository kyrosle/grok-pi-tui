use anyhow::{Context, Result};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

/// Materialized web-config extension bundle. `_source_dir` must stay alive for
/// the Pi process lifetime so relative imports between the TypeScript modules
/// keep resolving.
pub(super) struct WebConfigExtension {
    _source_dir: TempDir,
    source_path: PathBuf,
    ui_path: PathBuf,
    catalog_path: PathBuf,
}

impl WebConfigExtension {
    pub(super) fn source_path(&self) -> &Path {
        &self.source_path
    }

    /// Absolute path of the single-page UI handed to `PI_GROK_WEB_CONFIG_UI`.
    pub(super) fn ui_path(&self) -> &Path {
        &self.ui_path
    }

    /// Absolute path of the baked F2 settings catalog handed to
    /// `PI_GROK_WEB_CONFIG_CATALOG`.
    pub(super) fn catalog_path(&self) -> &Path {
        &self.catalog_path
    }
}

fn write_source_file(dir: &Path, name: &str, source: &str) -> Result<PathBuf> {
    let path = dir.join(name);
    let mut file = File::create(&path)
        .with_context(|| format!("create Pi web config extension module {name}"))?;
    file.write_all(source.as_bytes())
        .with_context(|| format!("write Pi web config extension module {name}"))?;
    file.flush()
        .with_context(|| format!("flush Pi web config extension module {name}"))?;
    file.sync_all().ok();
    Ok(path)
}

/// Materialize the `/pi-config web` + `/pi-models web` browser surface.
///
/// Every authored module must be materialized here — the injector owns the
/// transitive closure of `index.ts`'s relative imports (see AGENTS.md
/// "Diagnosing Pi RPC bootstrap / extension failures").
pub(super) fn write_web_config_extension() -> Result<WebConfigExtension> {
    let source_dir = tempfile::Builder::new()
        .prefix("pi-grok-web-config-")
        .tempdir()
        .context("create Pi web config extension source directory")?;
    let source_path = write_source_file(
        source_dir.path(),
        "index.ts",
        include_str!("../../../../../../extensions/pi-grok-web-config/index.ts"),
    )?;
    write_source_file(
        source_dir.path(),
        "shared.ts",
        include_str!("../../../../../../extensions/pi-grok-web-config/shared.ts"),
    )?;
    write_source_file(
        source_dir.path(),
        "config-store.ts",
        include_str!("../../../../../../extensions/pi-grok-web-config/config-store.ts"),
    )?;
    write_source_file(
        source_dir.path(),
        "server.ts",
        include_str!("../../../../../../extensions/pi-grok-web-config/server.ts"),
    )?;
    let ui_path = write_source_file(
        source_dir.path(),
        "ui.html",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/index.html"),
    )?;
    write_source_file(
        source_dir.path(),
        "styles.css",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/styles.css"),
    )?;
    write_source_file(
        source_dir.path(),
        "app.js",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/app.js"),
    )?;
    write_source_file(
        source_dir.path(),
        "ui-config.json",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/ui-config.json"),
    )?;
    write_source_file(
        source_dir.path(),
        "i18n.json",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/i18n.json"),
    )?;
    write_source_file(
        source_dir.path(),
        "models.js",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/models.js"),
    )?;
    write_source_file(
        source_dir.path(),
        "resources.js",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/resources.js"),
    )?;
    write_source_file(
        source_dir.path(),
        "host.js",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/host.js"),
    )?;
    write_source_file(
        source_dir.path(),
        "settings.js",
        include_str!("../../../../../../extensions/pi-grok-web-config/web/settings.js"),
    )?;
    let catalog_path =
        write_source_file(source_dir.path(), "host-catalog.json", &host_catalog_json())?;
    Ok(WebConfigExtension {
        _source_dir: source_dir,
        source_path,
        ui_path,
        catalog_path,
    })
}

/// Serialize the same `grok-pi.json` manifests build.rs bakes into
/// `BUNDLED_HOST_UI_SOURCES`, so the web F2 surface maps the registered
/// extension settings one-to-one with the native F2 modal.
fn host_catalog_json() -> String {
    use xai_grok_pager::settings::{SettingKind, SettingsRegistry};

    let registry = SettingsRegistry::defaults_with_host_features(
        &xai_grok_shared::host_features::HostFeatureManifest::default(),
    );
    // Keep the existing Web scalar-editor subset. Native metadata and support
    // come from the same registry as F2/palette; groups and Pi's model settings
    // continue using their existing dedicated surfaces.
    let ui: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../extensions/pi-grok-web-config/web/ui-config.json"
    ))
    .expect("valid baked Web UI config");
    let native = ui["host"]["catalog"]
        .as_array()
        .expect("Web native catalog")
        .iter()
        .filter_map(|entry| {
            let key = entry["key"].as_str()?;
            let meta = registry.all().iter().find(|meta| meta.key == key)?;
            let mut setting = entry.clone();
            setting["label"] = meta.label.into();
            setting["description"] = meta.description.into();
            setting["restartRequired"] = meta.restart_required.into();
            setting["f2"] = serde_json::json!({
                "category": meta.category.tab_label(),
                "section": entry["section"],
            });
            setting["default"] = match &meta.kind {
                SettingKind::Bool { default } => (*default).into(),
                SettingKind::String { default, .. }
                | SettingKind::Enum { default, .. }
                | SettingKind::DynamicEnum { default, .. } => (*default).into(),
                SettingKind::Int { default, .. } => (*default).into(),
                _ => return None,
            };
            Some(setting)
        })
        .collect::<Vec<_>>();
    let mut items = crate::bundled_host_ui::BUNDLED_HOST_UI_SOURCES
        .iter()
        .map(|(source, json)| {
            serde_json::json!({
                "source": source,
                "manifest": serde_json::from_str::<serde_json::Value>(json)
                    .expect("valid baked host manifest"),
            })
        })
        .collect::<Vec<_>>();
    items.push(serde_json::json!({
        "source": "native/Pi settings registry",
        "manifest": { "settings": native },
    }));
    serde_json::to_string_pretty(&items).expect("serializable host catalog")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bundle {
        index: String,
        shared: String,
        config_store: String,
        server: String,
        ui: String,
        styles: String,
        app: String,
        ui_config: String,
        i18n: String,
        models_ui: String,
        resources_ui: String,
        host_ui: String,
        settings_ui: String,
        catalog: String,
    }

    fn write_bundle() -> (WebConfigExtension, Bundle) {
        let extension = write_web_config_extension().expect("write extension");
        let dir = extension.source_path().parent().expect("source dir");
        let read = |name: &str| {
            std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
        };
        let bundle = Bundle {
            index: read("index.ts"),
            shared: read("shared.ts"),
            config_store: read("config-store.ts"),
            server: read("server.ts"),
            ui: read("ui.html"),
            styles: read("styles.css"),
            app: read("app.js"),
            ui_config: read("ui-config.json"),
            i18n: read("i18n.json"),
            models_ui: read("models.js"),
            resources_ui: read("resources.js"),
            host_ui: read("host.js"),
            settings_ui: read("settings.js"),
            catalog: read("host-catalog.json"),
        };
        (extension, bundle)
    }

    #[test]
    fn web_config_extension_materializes_every_module_of_the_entry_closure() {
        let (extension, bundle) = write_bundle();
        assert!(bundle.index.contains("from \"./server.ts\""));
        assert!(bundle.index.contains("from \"./config-store.ts\""));
        assert!(bundle.index.contains("from \"./shared.ts\""));
        assert!(bundle.config_store.contains("from \"./shared.ts\""));
        // Entry registers both web commands.
        assert!(bundle.index.contains("registerCommand(\"pi-config-web\""));
        assert!(bundle.index.contains("registerCommand(\"pi-models-web\""));
        // Per-module load-bearing symbols.
        assert!(bundle.shared.contains("PI_GROK_WEB_CONFIG_UI"));
        assert!(bundle.server.contains("startWebConfigServer"));
        assert!(bundle.server.contains("x-pi-token"));
        assert!(bundle.server.contains("styles.css"));
        assert!(bundle.server.contains("app.js"));
        assert!(bundle.server.contains("ui-config.json"));
        assert!(bundle.server.contains("i18n.json"));
        assert!(bundle.server.contains("models.js"));
        assert!(bundle.server.contains("resources.js"));
        assert!(bundle.server.contains("host.js"));
        assert!(bundle.server.contains("settings.js"));
        assert!(bundle.config_store.contains("collectState"));
        assert!(bundle.ui.contains("__PI_GROK_WEB_CONFIG_STYLES__"));
        assert!(bundle.ui.contains("__PI_GROK_WEB_CONFIG_APP__"));
        assert!(bundle.styles.contains(".app-shell"));
        assert!(bundle.app.contains("__PI_GROK_WEB_CONFIG_TOKEN__"));
        assert!(bundle.app.contains("__PI_GROK_WEB_CONFIG_UI_CONFIG__"));
        assert!(bundle.app.contains("__PI_GROK_WEB_CONFIG_I18N__"));
        assert!(bundle.app.contains("__PI_GROK_WEB_CONFIG_MODELS__"));
        assert!(bundle.app.contains("__PI_GROK_WEB_CONFIG_RESOURCES__"));
        assert!(bundle.app.contains("__PI_GROK_WEB_CONFIG_HOST__"));
        assert!(bundle.app.contains("__PI_GROK_WEB_CONFIG_SETTINGS__"));
        let ui_config: serde_json::Value =
            serde_json::from_str(&bundle.ui_config).expect("UI config is valid JSON");
        assert!(ui_config["settings"]["quickToggles"].is_array());
        assert!(ui_config["theme"]["modes"].is_array());
        let i18n: serde_json::Value =
            serde_json::from_str(&bundle.i18n).expect("i18n is valid JSON");
        assert!(i18n["en"].is_object());
        assert!(i18n["zh"].is_object());
        assert!(bundle.models_ui.contains("renderModels"));
        assert!(bundle.resources_ui.contains("renderResources"));
        assert!(bundle.host_ui.contains("renderHost"));
        assert!(bundle.settings_ui.contains("renderSettings"));
        // The F2 catalog mirrors the baked grok-pi.json manifests.
        let parsed: serde_json::Value =
            serde_json::from_str(&bundle.catalog).expect("host catalog is valid JSON");
        let sources = parsed.as_array().expect("catalog array");
        assert!(
            sources.iter().any(|item| item["source"]
                .as_str()
                .is_some_and(|value| value.contains("pi-grok-loop"))),
            "catalog must include the pi-grok-loop manifest"
        );
        assert!(
            sources
                .iter()
                .all(|item| item["manifest"]["settings"].is_array()),
            "every catalog entry carries a settings array"
        );
        let native = sources
            .iter()
            .find(|item| item["source"] == "native/Pi settings registry")
            .expect("shared native settings catalog")["manifest"]["settings"]
            .as_array()
            .unwrap();
        assert!(native.iter().any(|entry| entry["key"] == "pi_bash"));
        assert!(native.iter().any(|entry| entry["key"] == "session_recap"));
        for entry in native {
            assert!(xai_grok_pager::settings::external_setting_supported(
                entry["key"].as_str().unwrap()
            ));
        }
        for key in [
            "coding_data_sharing",
            "voice_keybind_enabled",
            "permission_mode",
        ] {
            assert!(!native.iter().any(|entry| entry["key"] == key));
        }
        assert_eq!(
            extension
                .source_path()
                .extension()
                .and_then(|value| value.to_str()),
            Some("ts")
        );
        assert_eq!(
            extension
                .ui_path()
                .extension()
                .and_then(|value| value.to_str()),
            Some("html")
        );
        assert!(extension.ui_path().exists());
        assert!(extension.catalog_path().exists());
    }
}
