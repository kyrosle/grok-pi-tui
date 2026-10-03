//! Package lifecycle delegates to the configured official Pi CLI. Pi owns
//! source identity, storage, filters, pins, and settings writes.

use super::*;
use crate::pi_rpc::{SpawnConfig, looks_like_js_cli, spawn_command_for_program};
use anyhow::Context;
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::sync::CancellationToken;

const SNAPSHOT_COMMAND: &str = "__pi_package_snapshot";
const OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageAction {
    operation: String,
    scope: String,
    source: Option<String>,
}

impl PackageAction {
    fn configured_cli_source(&self, snapshot: &Value) -> Result<Option<String>> {
        if !matches!(self.operation.as_str(), "remove" | "update") {
            return Ok(None);
        }
        let source = self.source.as_deref().unwrap_or("").trim();
        let package = snapshot
            .get("packages")
            .and_then(Value::as_array)
            .and_then(|packages| {
                packages.iter().find(|package| {
                    package["source"].as_str() == Some(source)
                        && package["scope"].as_str() == Some(self.scope.as_str())
                })
            })
            .context("Package declaration changed; refresh the resource list before operating")?;
        // Qualified npm/git/URL sources retain their identity and version/ref.
        // Local declarations are relative to settings, unlike CLI input (cwd).
        let qualified = [
            "npm:", "git:", "github:", "http:", "https:", "ssh:", "builtin:", "file:",
        ]
        .iter()
        .any(|prefix| source.starts_with(prefix));
        if Path::new(source).is_absolute() || source.starts_with('~') || qualified {
            return Ok(Some(source.to_owned()));
        }
        if let Some(installed) = package.get("installedPath").and_then(Value::as_str) {
            if !Path::new(installed).is_absolute() {
                bail!("Pi returned a non-absolute installed package path");
            }
            return Ok(Some(installed.to_owned()));
        }
        let base = snapshot
            .get("packageSettingsBases")
            .and_then(|bases| bases.get(self.scope.as_str()))
            .and_then(Value::as_str)
            .context("Pi package settings base is unavailable")?;
        if !Path::new(base).is_absolute() {
            bail!("Pi returned a non-absolute package settings base");
        }
        Ok(Some(
            Path::new(base).join(source).to_string_lossy().into_owned(),
        ))
    }

    fn cli_args(&self, project_trusted: bool) -> Result<Vec<String>> {
        if !matches!(self.scope.as_str(), "user" | "project") {
            bail!("Package scope must be user or project");
        }
        if self.scope == "project" && !project_trusted {
            bail!("Pi project is not trusted; package changes are unavailable");
        }
        let mut args = match self.operation.as_str() {
            "install" | "remove" | "update" => {
                let source = self.source.as_deref().unwrap_or("").trim();
                if source.is_empty()
                    || source.starts_with('-')
                    || source.chars().any(char::is_control)
                {
                    bail!("A package source is required (npm:, git:, or a local path)");
                }
                vec![self.operation.clone(), source.to_owned()]
            }
            "update_all" => vec!["update".into(), "--extensions".into()],
            _ => bail!("Unknown Pi package operation"),
        };
        // Pi updates by package identity across all configured scopes. Only
        // install/remove expose --local; never imply scope-local updates.
        if self.scope == "project" && matches!(self.operation.as_str(), "install" | "remove") {
            args.push("--local".into());
        }
        args.push(
            if project_trusted {
                "--approve"
            } else {
                "--no-approve"
            }
            .into(),
        );
        Ok(args)
    }
}

impl PiAgent {
    pub(super) async fn package_snapshot(&self) -> Result<Value, acp::Error> {
        let response_dir = tempfile::tempdir().map_err(acp_internal)?;
        let response_path = response_dir.path().join("snapshot.json");
        self.run_bridge_command(
            SNAPSHOT_COMMAND,
            &json!({"responsePath": response_path}).to_string(),
        )
        .await?;
        let bytes = std::fs::read(&response_path).map_err(acp_internal)?;
        let snapshot: Value = serde_json::from_slice(&bytes).map_err(acp_internal)?;
        let session_id = self.state.borrow().bootstrap.state.session_id.clone();
        let config = self.rpc.spawn_config();
        let expected_cwd = config.cwd.canonicalize().map_err(acp_internal)?;
        let actual_cwd = snapshot
            .get("cwd")
            .and_then(Value::as_str)
            .ok_or_else(|| acp::Error::internal_error().data("Package snapshot cwd is missing"))?;
        let actual_cwd = Path::new(actual_cwd).canonicalize().map_err(acp_internal)?;
        if snapshot.get("sessionId").and_then(Value::as_str) != Some(session_id.as_str())
            || actual_cwd != expected_cwd
        {
            return Err(acp::Error::internal_error()
                .data("Package snapshot does not belong to the active Pi session"));
        }
        Ok(snapshot)
    }

    pub(super) fn cancel_package_action(&self) -> Value {
        let active = self.package_cancel.borrow();
        if let Some(cancel) = active.as_ref() {
            cancel.cancel();
        }
        json!({"cancelRequested": active.is_some()})
    }

    pub(super) async fn package_action(&self, params: &Value) -> Result<Value, acp::Error> {
        let action: PackageAction = serde_json::from_value(params.clone()).map_err(acp_internal)?;
        // One shared exclusion with /reload avoids replacing the extension
        // runtime while an official package command is changing its resources.
        if !reserve_reload_request(&mut self.state.borrow_mut().reload_in_flight) {
            return Err(acp::Error::internal_error()
                .data("A package operation or reload is already in progress"));
        }
        let cancel = CancellationToken::new();
        *self.package_cancel.borrow_mut() = Some(cancel.clone());
        let result = self.package_action_inner(&action, &cancel).await;
        self.package_cancel.borrow_mut().take();
        self.state.borrow_mut().reload_in_flight = false;
        self.dispatch_next_queued().await;
        result
    }

    async fn package_action_inner(
        &self,
        action: &PackageAction,
        cancel: &CancellationToken,
    ) -> Result<Value, acp::Error> {
        let state = parse_state(
            &self
                .rpc
                .request(json!({"type": "get_state"}))
                .await
                .map_err(acp_internal)?,
        );
        if state.is_streaming || state.is_compacting {
            return Err(acp::Error::internal_error()
                .data("Wait for the current Pi response or compaction before changing packages"));
        }
        let before = self.package_snapshot().await?;
        if before
            .get("settingsErrors")
            .and_then(Value::as_array)
            .is_some_and(|errors| !errors.is_empty())
        {
            return Err(acp::Error::internal_error()
                .data("Pi settings contain errors; fix them before changing packages"));
        }
        let trusted = before
            .get("projectTrusted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut args = action.cli_args(trusted).map_err(acp_internal)?;
        if let Some(source) = action
            .configured_cli_source(&before)
            .map_err(acp_internal)?
        {
            args[1] = source;
        }
        if cancel.is_cancelled() {
            return Ok(
                json!({"operation":action.operation,"status":"cancelled","changed":false,"reloaded":false}),
            );
        }
        let config = self.rpc.spawn_config().clone();
        let output = run_package_cli(&config, &args, cancel)
            .await
            .map_err(acp_internal)?;
        // Re-read through the official manager even on failure/cancel: a
        // package tool can change storage before it exits unsuccessfully.
        let snapshot = self.package_snapshot().await;
        if output.cancelled || !output.success {
            return Ok(json!({
                "operation": action.operation,
                "status": if output.cancelled { "cancelled" } else { "failed" },
                "changed": true, "reloaded": false,
                "error": if output.cancelled { "Cancelled; declarations were reread. Reload to apply any completed changes.".to_owned() } else { output.text.clone() },
                "output": output.text,
                "snapshot": snapshot.ok(),
            }));
        }
        if let Err(error) = snapshot {
            return Ok(
                json!({"operation":action.operation,"status":"saved","changed":true,"reloaded":false,"error":format!("Pi command succeeded; refreshing declarations failed: {error}")}),
            );
        }
        if cancel.is_cancelled() {
            return Ok(
                json!({"operation":action.operation,"status":"saved","changed":true,"reloaded":false,"error":"Pi command succeeded; reload was cancelled","snapshot":snapshot.ok()}),
            );
        }
        let checkpoint = self.rpc.stderr_checkpoint();
        let reload = match self.reload_session_resources_inner().await {
            Ok(value) => value,
            Err(error) => {
                return Ok(
                    json!({"operation":action.operation,"status":"saved","changed":true,"reloaded":false,"error":format!("Pi command succeeded; reload failed: {error}"),"snapshot":snapshot.ok()}),
                );
            }
        };
        // Stderr is independently consumed. This bounded grace captures visible
        // diagnostics only; Pi 1.0 keeps loader.errors outside official RPC.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let checkpoint = if reload.get("restarted").and_then(Value::as_bool) == Some(true) {
            // The complete new-generation startup log belongs to this restart.
            (self.rpc.generation(), 0)
        } else {
            checkpoint
        };
        let diagnostics = match self.rpc.stderr_since(checkpoint) {
            Ok(lines) => lines,
            Err(error) => {
                return Ok(
                    json!({"operation":action.operation,"status":"saved","changed":true,"reloaded":false,"loaded":null,"loadStatus":"unverified","error":format!("Pi reloaded; diagnostics verification failed: {error}")}),
                );
            }
        };
        match self.package_snapshot().await {
            Ok(snapshot) => Ok(
                json!({"operation":action.operation,"status":"registry_refreshed","changed":true,"reloaded":true,"loaded":null,"loadStatus":"unverified","snapshot":snapshot,"output":output.text,"diagnostics":diagnostics}),
            ),
            Err(error) => Ok(
                json!({"operation":action.operation,"status":"saved","changed":true,"reloaded":false,"error":format!("Pi reloaded; live registry verification failed: {error}")}),
            ),
        }
    }
}

struct CliOutput {
    success: bool,
    cancelled: bool,
    text: String,
}

async fn capture_output(mut reader: impl AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            return Ok(output);
        }
        output.extend_from_slice(&chunk[..count]);
        if output.len() > OUTPUT_LIMIT {
            output.drain(..output.len() - OUTPUT_LIMIT);
        }
    }
}

async fn run_package_cli(
    config: &SpawnConfig,
    args: &[String],
    cancel: &CancellationToken,
) -> Result<CliOutput> {
    let program = Path::new(&config.program);
    let mut command = spawn_command_for_program(program);
    if looks_like_js_cli(program) {
        command.arg(program);
    }
    command
        .args(&config.prefix_args)
        .args(args)
        .current_dir(&config.cwd)
        .envs(config.env.iter().cloned())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if config
        .pi_args
        .iter()
        .any(|argument| argument == "--offline")
    {
        command.env("PI_OFFLINE", "1");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    let mut child = command.spawn()?;
    let pid = child.id();
    let mut stdout = tokio::spawn(capture_output(
        child
            .stdout
            .take()
            .context("Pi package stdout unavailable")?,
    ));
    let mut stderr = tokio::spawn(capture_output(
        child
            .stderr
            .take()
            .context("Pi package stderr unavailable")?,
    ));
    let (status, cancelled) = tokio::select! {
        status = child.wait() => (status?, false),
        _ = cancel.cancelled() => {
            if let Some(pid) = pid {
                #[cfg(unix)]
                let _ = tokio::process::Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().await;
                #[cfg(windows)]
                let _ = tokio::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().await;
            }
            let _ = child.start_kill();
            (child.wait().await?, true)
        }
    };
    let collected = tokio::time::timeout(Duration::from_secs(3), async {
        ((&mut stdout).await, (&mut stderr).await)
    })
    .await;
    let text = match collected {
        Ok((Ok(Ok(stdout)), Ok(Ok(stderr)))) => {
            let mut text = String::from_utf8_lossy(&stdout).into_owned();
            if !stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&String::from_utf8_lossy(&stderr));
            }
            if text.trim().is_empty() && !status.success() {
                format!("Pi package command exited with {status}")
            } else {
                text
            }
        }
        _ => {
            stdout.abort();
            stderr.abort();
            format!("Pi package command exited with {status}; output collection did not complete")
        }
    };
    Ok(CliOutput {
        success: status.success(),
        cancelled,
        text: text
            .chars()
            .filter(|character| *character == '\n' || !character.is_control())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_local_operations_use_sdk_scope_base_while_pins_and_new_installs_stay_literal() {
        let snapshot = json!({"agentDir":"/fixture/agent", "cwd":"/fixture/project", "packageSettingsBases":{"user":"/fixture/agent","project":"/fixture/project/.pi"}, "packages":[
            {"source":"../local-package", "scope":"user", "installedPath":"/fixture/local-package"},
            {"source":"tools/local", "scope":"project"},
            {"source":"npm:@demo/tools@1.0.0", "scope":"user", "installedPath":"/fixture/npm/tools"},
            {"source":"git:https://example.invalid/tools@abc", "scope":"user", "installedPath":"/fixture/git/tools"}
        ]});
        let action = |operation: &str, scope: &str, source: &str| PackageAction {
            operation: operation.into(),
            scope: scope.into(),
            source: Some(source.into()),
        };
        assert_eq!(
            action("remove", "user", "../local-package")
                .configured_cli_source(&snapshot)
                .unwrap()
                .as_deref(),
            Some("/fixture/local-package")
        );
        assert_eq!(
            action("update", "project", "tools/local")
                .configured_cli_source(&snapshot)
                .unwrap()
                .as_deref(),
            Some("/fixture/project/.pi/tools/local")
        );
        for source in [
            "npm:@demo/tools@1.0.0",
            "git:https://example.invalid/tools@abc",
        ] {
            assert_eq!(
                action("update", "user", source)
                    .configured_cli_source(&snapshot)
                    .unwrap()
                    .as_deref(),
                Some(source)
            );
        }
        assert!(
            action("remove", "project", "../local-package")
                .configured_cli_source(&snapshot)
                .is_err()
        );
        assert!(
            action("remove", "user", "missing")
                .configured_cli_source(&snapshot)
                .is_err()
        );
        assert_eq!(
            action("install", "user", "../local-package")
                .configured_cli_source(&snapshot)
                .unwrap(),
            None
        );
    }

    #[test]
    fn package_args_preserve_sources_and_keep_update_away_from_pi_self() {
        let action = |operation: &str, scope: &str| PackageAction {
            operation: operation.into(),
            scope: scope.into(),
            source: Some("npm:@demo/tools@1.0.0".into()),
        };
        assert_eq!(
            action("install", "project").cli_args(true).unwrap(),
            ["install", "npm:@demo/tools@1.0.0", "--local", "--approve"]
        );
        assert_eq!(
            action("remove", "user").cli_args(false).unwrap(),
            ["remove", "npm:@demo/tools@1.0.0", "--no-approve"]
        );
        assert_eq!(
            action("update", "project").cli_args(true).unwrap(),
            ["update", "npm:@demo/tools@1.0.0", "--approve"]
        );
        assert_eq!(
            action("update_all", "user").cli_args(false).unwrap(),
            ["update", "--extensions", "--no-approve"]
        );
        assert!(action("install", "project").cli_args(false).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cli_uses_selected_launcher_cwd_env_and_cancels_its_process_group() {
        use std::os::unix::fs::PermissionsExt;
        let fixture = tempfile::tempdir().unwrap();
        let launcher = fixture.path().join("pi-fixture");
        std::fs::write(&launcher, "#!/bin/sh\n[ \"$1\" = wrapper ] || exit 8\nshift\nprintf '%s\\n' \"$PWD\" \"$PI_FIXTURE\" \"offline=$PI_OFFLINE\" \"$@\"\nif [ \"$1\" = fail ]; then echo fixture-failure >&2; exit 7; fi\nif [ \"$1\" = wait ]; then sleep 30; fi\n").unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700)).unwrap();
        let config = SpawnConfig {
            program: launcher.to_string_lossy().into_owned(),
            prefix_args: vec!["wrapper".into()],
            cwd: fixture.path().into(),
            pi_args: vec!["--mode".into(), "rpc".into(), "--offline".into()],
            env: vec![("PI_FIXTURE".into(), "selected-host".into())],
        };
        let output = run_package_cli(
            &config,
            &["update".into(), "--extensions".into()],
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert!(output.success);
        assert!(
            output
                .text
                .contains("selected-host\noffline=1\nupdate\n--extensions")
        );
        assert!(output.text.contains(fixture.path().to_str().unwrap()));
        assert!(!output.text.contains("rpc"));
        let failed = run_package_cli(&config, &["fail".into()], &CancellationToken::new())
            .await
            .unwrap();
        assert!(!failed.success);
        assert!(!failed.cancelled);
        assert!(failed.text.contains("fixture-failure"));
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            token.cancel();
        });
        let output = tokio::time::timeout(
            Duration::from_secs(5),
            run_package_cli(&config, &["wait".into()], &cancel),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(output.cancelled);
    }
}
