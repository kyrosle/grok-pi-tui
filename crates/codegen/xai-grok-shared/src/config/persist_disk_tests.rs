use super::*;
/// grok_home is a process-lifetime cache. Set each real temporary root before
/// the child test harness starts, instead of changing env underneath that cache.
fn run_isolated(test: &str) -> bool {
    const CHILD: &str = "PI_GROK_CONFIG_DISK_TEST_CHILD";
    if std::env::var(CHILD).ok().as_deref() == Some(test) {
        assert!(std::env::var_os("GROK_HOME").is_some());
        crate::host_features::HostFeatureManifest::from_json_sources(&[(
            "pi-grok-workflows/grok-pi.json",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../extensions/pi-grok-workflows/grok-pi.json"
            )),
        )])
        .expect("real product workflow manifest");
        return false;
    }
    let home = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("config::persist::disk_tests::{test}"),
            "--nocapture",
        ])
        .env(CHILD, test)
        .env("GROK_HOME", home.path())
        .current_dir(home.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated config test failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
        "child must execute exactly the real named test"
    );
    true
}

#[tokio::test]
async fn disk_writer_preserves_unknown_sections_and_serializes_concurrent_settings() {
    if run_isolated("disk_writer_preserves_unknown_sections_and_serializes_concurrent_settings") {
        return;
    }
    let path = user_config_path();
    std::fs::write(
        &path,
        r#"[ui]
future_ui = "preserved"
[ui.display_refresh]
future_knob = 19
[privacy]
future_privacy = true
[consent]
future_consent = true
[skills]
future_skills = true
[unmodeled]
value = "keep"
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    }
    let (a, b) = tokio::join!(
        update_config(|cfg| cfg.ui.pi_workflows = true),
        crate::config::set_show_thinking_blocks(false),
    );
    a.unwrap();
    b.unwrap();
    let root: TomlValue = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(root["ui"]["pi_workflows"].as_bool(), Some(true));
    assert_eq!(root["ui"]["show_thinking_blocks"].as_bool(), Some(false));
    assert_eq!(root["ui"]["future_ui"].as_str(), Some("preserved"));
    assert_eq!(
        root["ui"]["display_refresh"]["future_knob"].as_integer(),
        Some(19)
    );
    for section in ["privacy", "consent", "skills"] {
        assert_eq!(
            root[section]
                .get(format!("future_{section}").as_str())
                .and_then(TomlValue::as_bool),
            Some(true)
        );
    }
    assert_eq!(root["unmodeled"]["value"].as_str(), Some("keep"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
    assert!(crate::config::load_config().await.unwrap().ui.pi_workflows);
}

#[tokio::test]
async fn disk_load_and_update_propagate_hard_read_errors() {
    if run_isolated("disk_load_and_update_propagate_hard_read_errors") {
        return;
    }
    let path = user_config_path();
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("marker"), "unchanged").unwrap();
    assert!(crate::config::load_config().await.is_err());
    assert!(
        update_config(|_| panic!("hard read errors must stop before mutation"))
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(path.join("marker")).unwrap(),
        "unchanged"
    );
}

#[tokio::test]
async fn syntax_error_is_not_overwritten_by_settings_update() {
    if run_isolated("syntax_error_is_not_overwritten_by_settings_update") {
        return;
    }
    let path = user_config_path();
    let bad = "[ui\npi_workflows = true\n";
    std::fs::write(&path, bad).unwrap();
    assert!(
        update_config(|_| panic!("invalid TOML must stop before mutation"))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), bad);
}

#[tokio::test]
async fn missing_file_is_the_only_empty_load_case_and_flock_is_enforced() {
    if run_isolated("missing_file_is_the_only_empty_load_case_and_flock_is_enforced") {
        return;
    }
    let held = acquire_init_lock(user_config_path().parent().unwrap()).unwrap();
    let result = update_config(|cfg| cfg.ui.pi_workflows = true).await;
    assert!(
        result.is_err(),
        "flock-only writers must block settings mutation"
    );
    assert!(!user_config_path().exists());
    drop(held);
    update_config(|cfg| cfg.ui.pi_workflows = true)
        .await
        .unwrap();
    assert!(crate::config::load_config().await.unwrap().ui.pi_workflows);
}

#[test]
fn full_native_snapshot_retains_every_existing_section() {
    let root = toml::from_str(
        r#"
[cli]
session_picker_grouped = true
[models]
default = "pi-model"
default_reasoning_effort = "high"
[harness]
block_for_upload = true
[skills]
paths = ["skill-root"]
[compat.claude]
skills = false
[diagnostics]
crash_handler = true
[session]
load_envrc = false
[toolset.ask_user_question]
timeout_enabled = false
[privacy]
privacy_banner_acked = "time"
[consent.answers.notice]
version = 2
[telemetry]
trace_upload = false
[features]
feedback_trace_card = false
[endpoints]
management_api_key = "test-key"
[permission]
rules = []
"#,
    )
    .unwrap();
    let cfg = load_config_from_toml(&root);
    assert_eq!(cfg.cli.session_picker_grouped, Some(true));
    assert_eq!(cfg.models.default.as_deref(), Some("pi-model"));
    assert_eq!(
        cfg.models.default_reasoning_effort,
        Some(xai_grok_sampling_types::ReasoningEffort::High)
    );
    assert_eq!(cfg.harness.wait_for_uploads, Some(true));
    assert_eq!(cfg.harness.block_for_upload, None);
    assert_eq!(cfg.skills.paths, ["skill-root"]);
    assert_eq!(cfg.diagnostics.crash_handler, Some(true));
    assert_eq!(cfg.session.load_envrc, Some(false));
    assert_eq!(cfg.ask_user_question.timeout_enabled, Some(false));
    assert_eq!(cfg.privacy.privacy_banner_acked.as_deref(), Some("time"));
    assert_eq!(cfg.consent.answers["notice"].version, 2);
    assert_eq!(cfg.telemetry.trace_upload, Some(false));
    assert_eq!(cfg.features.feedback_trace_card, Some(false));
    assert_eq!(cfg.management_api_key.as_deref(), Some("test-key"));
    assert!(cfg.permission.is_some());
    assert_eq!(cfg.compat.claude.skills, Some(false));
}
