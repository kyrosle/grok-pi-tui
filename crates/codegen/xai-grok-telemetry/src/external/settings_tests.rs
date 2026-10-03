//! Original resolver acceptance tests migrated with the actual implementation.
use super::*;
fn ext_env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
    let map: std::collections::HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |name: &str| map.get(name).cloned()
}
fn ext_client() -> crate::external::config::ExternalClientInfo {
    crate::external::config::ExternalClientInfo::default()
}
#[test]
fn external_otel_default_off_and_double_opt_in() {
    assert!(
        resolve_external_otel_config_with(None, None, ext_env(&[]), ext_client(), false).is_none()
    );
    assert!(
        resolve_external_otel_config_with(
            None,
            None,
            ext_env(&[("GROK_EXTERNAL_OTEL", "1")]),
            ext_client(),
            false,
        )
        .is_none()
    );
    assert!(
        resolve_external_otel_config_with(
            None,
            None,
            ext_env(&[
                ("GROK_EXTERNAL_OTEL", "1"),
                ("OTEL_METRICS_EXPORTER", "otlp"),
            ]),
            ext_client(),
            false,
        )
        .is_some()
    );
}
#[test]
fn external_otel_file_table_layered_under_env() {
    let effective: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "https://collector.corp.example:4318"
            otel_protocol = "grpc"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        None,
        ext_env(&[]),
        ext_client(),
        false,
    )
    .expect("file table must activate");
    assert_eq!(cfg.logs_transport.as_protocol_str(), "grpc");
    assert_eq!(cfg.metrics_transport.as_protocol_str(), "grpc");
    assert_eq!(cfg.logs_endpoint, "https://collector.corp.example:4318");
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        None,
        ext_env(&[("OTEL_EXPORTER_OTLP_PROTOCOL", "http/protobuf")]),
        ext_client(),
        false,
    )
    .expect("env protocol must override file protocol");
    assert_eq!(cfg.logs_transport.as_protocol_str(), "http/protobuf");
    assert_eq!(cfg.metrics_transport.as_protocol_str(), "http/protobuf");
    assert_eq!(
        cfg.logs_endpoint,
        "https://collector.corp.example:4318/v1/logs"
    );
    assert!(
        resolve_external_otel_config_with(
            Some(&effective),
            None,
            ext_env(&[("GROK_EXTERNAL_OTEL", "0")]),
            ext_client(),
            false,
        )
        .is_none()
    );
}
#[test]
fn external_otel_file_table_carries_mtls_paths() {
    let effective: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_metrics_exporter = "otlp"
            otel_endpoint = "https://collector.corp.example:4318"
            otel_protocol = "grpc"
            otel_certificate = "/etc/ssl/corp-ca.pem"
            otel_client_certificate = "/etc/ssl/client.crt"
            otel_client_key = "/etc/ssl/client.key"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        None,
        ext_env(&[]),
        ext_client(),
        false,
    )
    .expect("managed paths alone must activate");
    assert_eq!(
        cfg.logs_ca_certificate.as_deref(),
        Some("/etc/ssl/corp-ca.pem")
    );
    assert_eq!(
        cfg.logs_client_certificate.as_deref(),
        Some("/etc/ssl/client.crt")
    );
    assert_eq!(cfg.logs_client_key.as_deref(), Some("/etc/ssl/client.key"));
    assert_eq!(
        cfg.metrics_client_certificate.as_deref(),
        Some("/etc/ssl/client.crt")
    );
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        None,
        ext_env(&[
            ("OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE", "/env/client.crt"),
            ("OTEL_EXPORTER_OTLP_CLIENT_KEY", "/env/client.key"),
        ]),
        ext_client(),
        false,
    )
    .expect("env override must resolve");
    assert_eq!(
        cfg.logs_client_certificate.as_deref(),
        Some("/env/client.crt")
    );
    assert_eq!(cfg.logs_client_key.as_deref(), Some("/env/client.key"));
}
#[test]
fn external_otel_requirements_pin_wins_over_env() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = false
            "#,
    )
    .unwrap();
    assert!(
        resolve_external_otel_config_with(
            None,
            Some(&req),
            ext_env(&[("GROK_EXTERNAL_OTEL", "1"), ("OTEL_LOGS_EXPORTER", "otlp"),]),
            ext_client(),
            false,
        )
        .is_none()
    );
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_log_user_prompts = false
            otel_log_tool_details = false
            otel_log_tool_content = false
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[
            ("GROK_EXTERNAL_OTEL", "1"),
            ("OTEL_LOGS_EXPORTER", "otlp"),
            ("OTEL_LOG_USER_PROMPTS", "1"),
            ("OTEL_LOG_TOOL_DETAILS", "1"),
            ("OTEL_LOG_TOOL_CONTENT", "1"),
        ]),
        ext_client(),
        false,
    )
    .expect("stream still active; only gates pinned");
    assert!(!cfg.gates.log_user_prompts, "requirement pin must win");
    assert!(!cfg.gates.log_tool_details, "requirement pin must win");
    assert!(!cfg.gates.log_tool_content, "requirement pin must win");
}
#[test]
fn external_otel_requirements_pin_endpoint_beats_env() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:9")]),
        ext_client(),
        false,
    )
    .expect("pin must keep the stream active");
    assert!(
        cfg.logs_endpoint.starts_with("http://corp:4318"),
        "listed endpoint must beat env: {}",
        cfg.logs_endpoint
    );
}
#[test]
fn external_otel_unset_endpoint_still_env_overridable() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4318")]),
        ext_client(),
        false,
    )
    .expect("unset endpoint stays developer-settable");
    assert!(
        cfg.logs_endpoint.contains("127.0.0.1:4318"),
        "{}",
        cfg.logs_endpoint
    );
}
#[test]
fn external_otel_pin_endpoint_strips_per_signal_env() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[(
            "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            "http://127.0.0.1:9/v1/logs",
        )]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert!(
        !cfg.logs_endpoint.contains("127.0.0.1:9"),
        "unlisted per-signal endpoint env must not win: {}",
        cfg.logs_endpoint
    );
    assert!(cfg.logs_endpoint.contains("corp:4318"));
}
#[test]
fn external_otel_pin_endpoint_hides_unlisted_file_siblings() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            "#,
    )
    .unwrap();
    let effective: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_logs_endpoint = "http://127.0.0.1:9/v1/logs"
            otel_metrics_endpoint = "http://127.0.0.1:9/v1/metrics"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        Some(&req),
        ext_env(&[]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert!(
        !cfg.logs_endpoint.contains("127.0.0.1:9"),
        "unlisted file sibling must not retarget: {}",
        cfg.logs_endpoint
    );
    assert!(
        cfg.logs_endpoint.contains("corp:4318"),
        "{}",
        cfg.logs_endpoint
    );
}
#[test]
fn external_otel_pin_protocol_hides_unlisted_file_siblings() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            otel_protocol = "http/protobuf"
            "#,
    )
    .unwrap();
    let effective: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_logs_protocol = "grpc"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        Some(&req),
        ext_env(&[]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert_eq!(
        cfg.logs_transport,
        crate::external::config::OtlpTransport::HttpProtobuf,
        "unlisted file protocol sibling must not win"
    );
}
#[test]
fn external_otel_pin_ca_keeps_file_endpoint() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_certificate = "/etc/ssl/corp-ca.pem"
            "#,
    )
    .unwrap();
    let effective: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_logs_endpoint = "http://logs:4318/v1/logs"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        Some(&req),
        ext_env(&[]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert_eq!(
        cfg.logs_endpoint, "http://logs:4318/v1/logs",
        "CA pin must not lock destination"
    );
}
#[test]
fn external_otel_pin_client_cert_hides_file_endpoints() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_client_certificate = "/etc/ssl/client.crt"
            otel_client_key = "/etc/ssl/client.key"
            "#,
    )
    .unwrap();
    let effective: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_logs_endpoint = "http://127.0.0.1:9/v1/logs"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        Some(&effective),
        Some(&req),
        ext_env(&[]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert!(
        !cfg.logs_endpoint.contains("127.0.0.1:9"),
        "client-identity pin must hide unlisted file endpoints: {}",
        cfg.logs_endpoint
    );
}
#[test]
fn external_otel_pin_logs_endpoint_keeps_that_signal() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            otel_logs_endpoint = "http://logs:4318/v1/logs"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[(
            "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
            "http://127.0.0.1:9/v1/logs",
        )]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert_eq!(cfg.logs_endpoint, "http://logs:4318/v1/logs");
}
#[test]
fn external_otel_pin_exporter_beats_none() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[("OTEL_LOGS_EXPORTER", "none")]),
        ext_client(),
        false,
    )
    .expect("pinned exporter must beat OTEL_LOGS_EXPORTER=none");
    assert_eq!(
        cfg.logs_exporter,
        crate::external::config::ExporterSelection::Otlp
    );
}
#[test]
fn external_otel_omit_exporter_still_allows_none() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_endpoint = "http://corp:4318"
            "#,
    )
    .unwrap();
    assert!(
        resolve_external_otel_config_with(
            None,
            Some(&req),
            ext_env(&[
                ("OTEL_LOGS_EXPORTER", "none"),
                ("OTEL_METRICS_EXPORTER", "none")
            ]),
            ext_client(),
            false,
        )
        .is_none(),
        "omitted exporter stays env-overridable to none"
    );
}
#[test]
fn external_otel_pin_client_cert_strips_developer_endpoints() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            otel_client_certificate = "/etc/ssl/client.crt"
            otel_client_key = "/etc/ssl/client.key"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:9")]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert!(
        cfg.logs_endpoint.starts_with("http://corp:4318"),
        "{}",
        cfg.logs_endpoint
    );
}
#[test]
fn external_otel_pin_ca_does_not_strip_endpoints() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_certificate = "/etc/ssl/corp-ca.pem"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4318")]),
        ext_client(),
        false,
    )
    .expect("CA-only pin must not lock destination");
    assert!(
        cfg.logs_endpoint.contains("127.0.0.1:4318"),
        "{}",
        cfg.logs_endpoint
    );
}
#[test]
fn external_otel_pin_ca_hides_unlisted_per_signal_cert_env() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_endpoint = "http://corp:4318"
            otel_certificate = "/etc/ssl/corp-ca.pem"
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[(
            "OTEL_EXPORTER_OTLP_LOGS_CERTIFICATE",
            "/tmp/decoy-logs-ca.pem",
        )]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert_eq!(
        cfg.logs_ca_certificate.as_deref(),
        Some("/etc/ssl/corp-ca.pem"),
        "generic CA pin must hide OTEL_EXPORTER_OTLP_LOGS_CERTIFICATE"
    );
}
#[test]
fn external_otel_pin_assistant_beats_env() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_log_assistant_responses = false
            "#,
    )
    .unwrap();
    let cfg = resolve_external_otel_config_with(
        None,
        Some(&req),
        ext_env(&[("OTEL_LOG_ASSISTANT_RESPONSES", "1")]),
        ext_client(),
        false,
    )
    .expect("stream active");
    assert!(!cfg.gates.log_assistant_responses);
}
#[test]
fn external_otel_pin_prompts_true_omitted_assistant_stays_off() {
    let req: toml::Value = toml::from_str(
        r#"
            [telemetry]
            otel_enabled = true
            otel_logs_exporter = "otlp"
            otel_log_user_prompts = true
            "#,
    )
    .unwrap();
    let cfg =
        resolve_external_otel_config_with(None, Some(&req), ext_env(&[]), ext_client(), false)
            .expect("stream active");
    assert!(cfg.gates.log_user_prompts);
    assert!(
        !cfg.gates.log_assistant_responses,
        "omitted sibling gate must default off across the requirements boundary"
    );
    assert!(!cfg.gates.log_tool_details);
    assert!(
        !cfg.gates.log_tool_content,
        "omitted CONTENT sibling must default off across the requirements boundary"
    );
}
#[test]
fn external_otel_carries_internal_consumed_flag() {
    let cfg = resolve_external_otel_config_with(
        None,
        None,
        ext_env(&[("GROK_EXTERNAL_OTEL", "1"), ("OTEL_LOGS_EXPORTER", "otlp")]),
        ext_client(),
        true,
    )
    .expect("resolution itself still succeeds");
    assert!(cfg.internal_pipeline_consumed_otel_vars);
}
