//! Full explicit external OTEL settings resolution, independent of the stock agent.
//! Requirements destination/content pins and env precedence remain in one actual parser.

fn telemetry_otel_str(t: &toml::Value, key: &str) -> Option<String> {
    t.get(key).and_then(toml::Value::as_str).map(str::to_owned)
}
fn telemetry_otel_ms(t: &toml::Value, key: &str) -> Option<String> {
    t.get(key).and_then(|v| {
        v.as_integer()
            .map(|i| i.to_string())
            .or_else(|| v.as_str().map(str::to_owned))
    })
}
fn telemetry_otel_file_config(t: &toml::Value) -> super::ExternalOtelFileConfig {
    super::ExternalOtelFileConfig {
        enabled: t.get("otel_enabled").and_then(toml::Value::as_bool),
        metrics_exporter: telemetry_otel_str(t, "otel_metrics_exporter"),
        logs_exporter: telemetry_otel_str(t, "otel_logs_exporter"),
        endpoint: telemetry_otel_str(t, "otel_endpoint"),
        protocol: telemetry_otel_str(t, "otel_protocol")
            .or_else(|| telemetry_otel_str(t, "otel_transport")),
        certificate: telemetry_otel_str(t, "otel_certificate"),
        client_certificate: telemetry_otel_str(t, "otel_client_certificate"),
        client_key: telemetry_otel_str(t, "otel_client_key"),
        log_user_prompts: t
            .get("otel_log_user_prompts")
            .and_then(toml::Value::as_bool),
        log_tool_details: t
            .get("otel_log_tool_details")
            .and_then(toml::Value::as_bool),
        log_assistant_responses: t
            .get("otel_log_assistant_responses")
            .and_then(toml::Value::as_bool),
        log_tool_content: t
            .get("otel_log_tool_content")
            .and_then(toml::Value::as_bool),
        timeout: telemetry_otel_ms(t, "otel_timeout"),
        metric_export_interval: telemetry_otel_ms(t, "otel_metric_export_interval"),
        logs_endpoint: telemetry_otel_str(t, "otel_logs_endpoint"),
        metrics_endpoint: telemetry_otel_str(t, "otel_metrics_endpoint"),
        logs_protocol: telemetry_otel_str(t, "otel_logs_protocol"),
        metrics_protocol: telemetry_otel_str(t, "otel_metrics_protocol"),
        logs_certificate: telemetry_otel_str(t, "otel_logs_certificate"),
        metrics_certificate: telemetry_otel_str(t, "otel_metrics_certificate"),
        logs_client_certificate: telemetry_otel_str(t, "otel_logs_client_certificate"),
        logs_client_key: telemetry_otel_str(t, "otel_logs_client_key"),
        metrics_client_certificate: telemetry_otel_str(t, "otel_metrics_client_certificate"),
        metrics_client_key: telemetry_otel_str(t, "otel_metrics_client_key"),
        include_session_id: t
            .get("otel_metrics_include_session_id")
            .and_then(toml::Value::as_bool),
    }
}
/// Native process startup has no internal stock OTEL pipeline. Resolve the user's
/// complete explicit exporter configuration and deployment pins from product disk layers.
pub fn resolve_external_otel_config(
    client: super::config::ExternalClientInfo,
) -> Option<super::ExternalOtelConfig> {
    let requirements = xai_grok_config::load_merged_requirements();
    resolve_external_otel_config_with(
        xai_grok_config::load_effective_config_disk_only()
            .ok()
            .as_ref(),
        requirements.as_ref(),
        |name| std::env::var(name).ok(),
        client,
        false,
    )
}

/// Testable core of [`resolve_external_otel_config`]: all inputs injected so tests don't race on process env / disk.
pub fn resolve_external_otel_config_with(
    effective_config: Option<&toml::Value>,
    requirements: Option<&toml::Value>,
    getenv: impl Fn(&str) -> Option<String>,
    client: super::config::ExternalClientInfo,
    internal_pipeline_consumed_otel_vars: bool,
) -> Option<super::ExternalOtelConfig> {
    let pins = super::requirements_pin::RequirementOtelPins::from_requirements(requirements);
    let file_cfg: Option<super::ExternalOtelFileConfig> = effective_config
        .and_then(|cfg| cfg.get("telemetry"))
        .cloned()
        .map(|mut telemetry| {
            if let Some(table) = telemetry.as_table_mut() {
                pins.hide_unlisted_file_siblings(table);
            }
            telemetry_otel_file_config(&telemetry)
        });
    let getenv_pinned = super::requirements_pin::getenv_with_pins(&pins, getenv);
    let mut resolved = super::ExternalOtelConfig::resolve_with(getenv_pinned, file_cfg.as_ref())?;
    resolved.client = client;
    resolved.internal_pipeline_consumed_otel_vars = internal_pipeline_consumed_otel_vars;
    Some(resolved)
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
