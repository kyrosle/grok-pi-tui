//! Installed Pi disposition and official runtime controls through production ACP.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::{PiAgent, PiBootstrap, PiRpc, SpawnConfig};
use serde_json::{Value, json};
use std::{path::PathBuf, rc::Rc, time::Duration};
use xai_acp_lib::{AcpClientMessage, acp_channels};

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires installed Pi 1.0; isolated synthetic provider, no network"]
async fn actual_pi_disposition_and_runtime_controls() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let dir = tempfile::tempdir().unwrap();
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(3)
                .unwrap()
                .to_path_buf();
            let bridge = dir.path().join("bridge.ts");
            let source = std::fs::read_to_string(
                root.join("crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/tree_bridge.rs"),
            )
            .unwrap();
            std::fs::write(
                &bridge,
                source
                    .split_once("r#\"")
                    .unwrap()
                    .1
                    .split_once("\"#;")
                    .unwrap()
                    .0,
            )
            .unwrap();
            let args = [
                "--mode",
                "rpc",
                "--offline",
                "--no-session",
                "--no-approve",
                "-ne",
                "-ns",
                "-np",
                "-nc",
                "--no-themes",
                "--model",
                "pi-lifecycle/local",
            ]
            .into_iter()
            .map(str::to_string)
            .chain([
                "--extension".into(),
                bridge.display().to_string(),
                "--extension".into(),
                root.join("crates/codegen/pi-grok-adapter/tests/fixtures/pi_lifecycle.ts")
                    .display()
                    .to_string(),
            ])
            .collect();
            let process = PiRpc::spawn(SpawnConfig {
                program: std::env::var("PI_BIN").unwrap_or_else(|_| "pi".into()),
                prefix_args: vec![],
                cwd: dir.path().into(),
                pi_args: args,
                env: vec![
                    (
                        "PI_CODING_AGENT_DIR".into(),
                        dir.path().join("pi").display().to_string(),
                    ),
                    ("PI_OFFLINE".into(), "1".into()),
                    ("PI_TELEMETRY".into(), "0".into()),
                ]
                .into_iter()
                .chain(std::env::vars().filter_map(|(key, _)| {
                    (key.ends_with("_API_KEY")
                        || key.ends_with("_TOKEN")
                        || key.starts_with("AWS_"))
                    .then_some((key, String::new()))
                }))
                .collect(),
            })
            .await
            .unwrap();
            let rpc = process.rpc.clone();
            let bootstrap = PiBootstrap::load(&rpc).await.unwrap();
            let session_id = acp::SessionId::new(bootstrap.session_id().to_string());
            let (mut client, channel) = acp_channels();
            let agent = Rc::new(
                PiAgent::new(
                    process.rpc,
                    channel.tx,
                    bootstrap,
                    dir.path().join("sessions"),
                    None,
                    None,
                    None,
                    None,
                    None,
                    false,
                )
                .unwrap(),
            );
            let sink = tokio::task::spawn_local(async move {
                while let Some(message) = client.rx.recv().await {
                    match message {
                        AcpClientMessage::SessionNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        AcpClientMessage::ExtNotification(args) => {
                            let _ = args.response_tx.send(Ok(()));
                        }
                        other => panic!("unexpected request: {other:?}"),
                    }
                }
            });
            let event_agent = agent.clone();
            let events = tokio::task::spawn_local(async move {
                event_agent.run_events(process.events).await;
            });
            for text in ["fixture-handled", "/fixture-no-run", "fixture-model-run"] {
                let response = tokio::time::timeout(
                    Duration::from_secs(10),
                    agent.prompt(acp::PromptRequest::new(
                        session_id.clone(),
                        vec![acp::ContentBlock::Text(acp::TextContent::new(text))],
                    )),
                )
                .await
                .unwrap()
                .unwrap();
                assert_eq!(response.stop_reason, acp::StopReason::EndTurn, "{text}");
            }
            for params in [
                json!({"operation":"status"}),
                json!({"operation":"retry","enabled":false}),
                json!({"operation":"compaction","enabled":false}),
                json!({"operation":"cancel-retry"}),
            ] {
                let response = agent
                    .ext_method(acp::ExtRequest::new(
                        "pi/runtime/control",
                        serde_json::value::to_raw_value(&params).unwrap().into(),
                    ))
                    .await
                    .unwrap();
                let value: Value = serde_json::from_str(response.0.get()).unwrap();
                let result = value.get("result").unwrap_or(&value);
                assert!(result["message"].as_str().unwrap().contains("Pi runtime"));
                if params["operation"] == "retry" {
                    assert_eq!(result["retryConfiguredEnabled"], false, "{result}");
                }
                if params["operation"] == "compaction" {
                    assert_eq!(result["state"]["autoCompactionEnabled"], false);
                }
            }
            let stale = agent
                .ext_method(acp::ExtRequest::new(
                    "pi/packages/cancel",
                    serde_json::value::to_raw_value(&json!({"sessionId":"stale-session"}))
                        .unwrap()
                        .into(),
                ))
                .await;
            assert!(
                stale.is_err(),
                "late cancel cannot target a replacement session"
            );
            let reloaded = agent
                .ext_method(acp::ExtRequest::new(
                    "pi/session/reload",
                    serde_json::value::to_raw_value(&json!({})).unwrap().into(),
                ))
                .await
                .unwrap();
            let reloaded: Value = serde_json::from_str(reloaded.0.get()).unwrap();
            let reloaded = reloaded.get("result").unwrap_or(&reloaded);
            assert_eq!(reloaded["reloaded"], true);
            assert_eq!(reloaded["loaded"], Value::Null);
            assert!(reloaded["snapshot"]["commands"].is_array());
            let settings: Value = serde_json::from_str(
                &std::fs::read_to_string(dir.path().join("pi/settings.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(settings["retry"]["enabled"], false);
            assert_eq!(settings["compaction"]["enabled"], false);
            rpc.kill().await;
            events.abort();
            sink.abort();
        })
        .await;
}
