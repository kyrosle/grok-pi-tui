use xai_grok_config_types::{CompatConfigToml, PermissionConfig, SkillsConfig};
/// TUI/CLI settings. Composed from typed section configs defined in `agent::config`.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub cli: xai_grok_config_types::CliConfig,
    pub models: xai_grok_config_types::ModelsConfig,
    pub ui: crate::ui_config::UiConfig,
    pub harness: xai_grok_config_types::HarnessConfig,
    pub skills: SkillsConfig,
    /// `[compat]` vendor-compatibility config, round-tripped so the pager preserves per-vendor toggles when persisting other settings.
    pub compat: CompatConfigToml,
    /// Management API key from `[endpoints]`.
    pub management_api_key: Option<String>,
    /// Permission policy rules loaded from `[permission]` section in config.toml.
    pub permission: Option<PermissionConfig>,
    pub diagnostics: xai_grok_config_types::DiagnosticsConfig,
    /// `[session]` section, round-tripped through `merge_section` so pager setters can persist session fields (e.g. auto-compact threshold).
    pub session: xai_grok_config_types::SessionConfig,
    /// `[toolset.ask_user_question]` sub-table, the only `[toolset]` piece the settings modal writes.
    /// The rest of `[toolset]` never round-trips (it carries runtime-only structs whose defaults must not hit disk).
    pub ask_user_question: xai_grok_config_types::AskUserQuestionToolConfig,
    /// `[privacy]`: local banner ack (not auth-metadata).
    pub privacy: PrivacyConfig,
    pub consent: super::consent::ConsentConfig,
    /// `[telemetry]`: only the key the pager persists round-trips.
    pub telemetry: TelemetryPersistConfig,
    /// `[features]`: only the key the pager persists round-trips.
    pub features: FeaturesPersistConfig,
}

/// The `[telemetry]` slice the pager is allowed to write back.
/// Unmodeled keys under `[telemetry]` are preserved by the deep merge in `save_config_locked`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct TelemetryPersistConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_upload: Option<bool>,
}

/// The `[features]` slice the pager is allowed to write back.
/// Unmodeled keys under `[features]` are preserved by the deep merge in `save_config_locked`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FeaturesPersistConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_trace_card: Option<bool>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PrivacyConfig {
    /// Last banner dismiss (Accept/Customize), RFC 3339 UTC.
    /// When the remote `privacy_banner_reshow_days` is unset or 0, the banner never re-shows once this is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_banner_acked: Option<String>,
}
