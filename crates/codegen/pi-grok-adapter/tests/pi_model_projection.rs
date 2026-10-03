//! Installed public Pi model/Codemode APIs → production ACP; no network or credentials.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::{PiAgent, PiBootstrap, PiRpc, SpawnConfig};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};
use xai_acp_lib::{AcpClientMessage, acp_channels};

async fn run() -> Value {
    let directory = tempfile::tempdir().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let process = PiRpc::spawn(SpawnConfig {
        program: std::env::var("PI_BIN").unwrap_or_else(|_| "pi".into()),
        prefix_args: Vec::new(),
        cwd: directory.path().to_path_buf(),
        pi_args: [
            "--mode",
            "rpc",
            "--offline",
            "-ne",
            "-ns",
            "-np",
            "-nc",
            "--no-themes",
            "--no-approve",
            "--model",
            "pi-router-fixture/auto",
            "--thinking",
            "high",
            "--tools",
            "codemode",
            "--extension",
            "builtin:codemode",
        ]
        .into_iter()
        .map(str::to_string)
        .chain([
            "--extension".into(),
            root.join("crates/codegen/pi-grok-adapter/tests/fixtures/pi_models.ts")
                .display()
                .to_string(),
            "--session-dir".into(),
            directory.path().join("sessions").display().to_string(),
        ])
        .collect(),
        env: vec![
            (
                "PI_CODING_AGENT_DIR".into(),
                directory.path().join("pi").display().to_string(),
            ),
            ("PI_OFFLINE".into(), "1".into()),
            ("PI_TELEMETRY".into(), "0".into()),
        ]
        .into_iter()
        .chain(std::env::vars().filter_map(|(key, _)| {
            (key.ends_with("_API_KEY") || key.ends_with("_TOKEN") || key.starts_with("AWS_"))
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
            directory.path().join("sessions"),
            None,
            None,
            None,
            None,
            None,
            false,
            false,
        )
        .unwrap(),
    );
    let captured = Rc::new(RefCell::new(Vec::new()));
    let sink = captured.clone();
    let consumer = tokio::task::spawn_local(async move {
        while let Some(message) = client.rx.recv().await {
            match message {
                AcpClientMessage::SessionNotification(args) => {
                    sink.borrow_mut().push(json!({"notification":args.request}));
                    let _ = args.response_tx.send(Ok(()));
                }
                AcpClientMessage::ExtNotification(args) => {
                    sink.borrow_mut().push(json!({"method":args.request.method,
                        "params":serde_json::from_str::<Value>(args.request.params.get()).unwrap()}));
                    let _ = args.response_tx.send(Ok(()));
                }
                other => panic!("unexpected model fixture UI request: {other:?}"),
            }
        }
    });
    let runner = agent.clone();
    let events = tokio::task::spawn_local(async move {
        runner.run_events(process.events).await;
    });
    tokio::time::timeout(
        Duration::from_secs(30),
        agent.prompt(acp::PromptRequest::new(
            session_id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(
                "codemode success",
            ))],
        )),
    )
    .await
    .unwrap()
    .unwrap();
    let live = captured.borrow().clone();
    let route = live
        .iter()
        .find(|event| {
            event["method"] == "pi/ui/status" && event["params"]["statusKey"] == "model_route"
        })
        .unwrap();
    assert_eq!(
        route["params"]["statusText"],
        "pi-router-fixture/auto · high → pi-model-fixture/small · medium"
    );
    assert!(
        live.iter()
            .any(|event| event["notification"]["_meta"]["totalContextTokens"] == 32768)
    );
    let state = rpc.request(json!({"type":"get_state"})).await.unwrap();
    assert_eq!(state["model"]["provider"], "pi-router-fixture");
    let info = agent
        .ext_method(acp::ExtRequest::new(
            "x.ai/session/info",
            serde_json::value::to_raw_value(&json!({})).unwrap().into(),
        ))
        .await
        .unwrap();
    let info: Value = serde_json::from_str(info.0.get()).unwrap();
    let rows = info["result"]["sessionStats"]["modelUsage"]
        .as_array()
        .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "physical chat and unattributed Codemode usage stay separate"
    );
    assert!(
        rows.iter()
            .any(|row| row["provider"] == "pi-model-fixture" && row["model"] == "small")
    );
    let breakdown: f64 = rows.iter().map(|row| row["cost"].as_f64().unwrap()).sum();
    assert!((breakdown - info["result"]["sessionStats"]["cost"].as_f64().unwrap()).abs() < 1e-12);
    captured.borrow_mut().clear();
    agent
        .load_session(acp::LoadSessionRequest::new(
            session_id,
            directory.path().to_path_buf(),
        ))
        .await
        .unwrap();
    let replay = captured.borrow().clone();
    let end = |items: &Vec<Value>| {
        items
            .iter()
            .filter_map(|event| event.get("notification")?.get("update").cloned())
            .find(|update| {
                update["sessionUpdate"] == "tool_call_update"
                    && update["toolCallId"] == "fixture-model-codemode"
                    && update["status"] == "completed"
            })
            .unwrap()
    };
    let live_end = end(&live);
    let replay_end = end(&replay);
    assert_eq!(live_end["content"], replay_end["content"]);
    assert!(
        live_end["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["content"]["type"] == "image")
    );
    assert!(replay.iter().any(|event| event["method"] == "pi/ui/status"
        && event["params"]["statusKey"] == "model_route"
        && event["params"]["statusText"] == route["params"]["statusText"]));
    rpc.kill().await;
    events.abort();
    consumer.abort();
    json!({"live":live, "replay":replay, "info":info})
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "installed Pi public model pipeline; isolated synthetic provider"]
async fn installed_pi_model_route_image_classifier_projection() {
    let home = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("GROK_HOME", home.path());
        std::env::set_var("GROK_PROJECT_DIR", home.path().join("project"));
        std::env::set_var("PI_GROK_RPC_WATCHDOG", "0");
    }
    let result = tokio::task::LocalSet::new().run_until(run()).await;
    if let Ok(path) = std::env::var("PI_MODEL_CAPTURE") {
        std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    }
    eprintln!(
        "PASS: public Pi virtual dispatch/context/cost + image/classifier Codemode live/replay ACP"
    );
}
