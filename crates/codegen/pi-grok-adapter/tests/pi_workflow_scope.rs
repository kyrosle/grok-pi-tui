//! Real installed Pi + production adapter workflow scope lifecycle; synthetic provider, temporary data only.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::{PiAgent, PiBootstrap, PiRpc, SpawnConfig};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use xai_acp_lib::{AcpClientMessage, acp_channels};

async fn wait_file(path: &Path) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("real child provider did not reach the fixture marker");
}
async fn launch(agent: &PiAgent, objective: &str) -> String {
    let params = json!({"name":"deep-research", "args":json!({"query":objective,"objective":objective}).to_string()});
    let response = agent
        .ext_method(acp::ExtRequest::new(
            "x.ai/workflow/launch",
            serde_json::value::to_raw_value(&params).unwrap().into(),
        ))
        .await
        .unwrap();
    let body: Value = serde_json::from_str(response.0.get()).unwrap();
    body["result"]["runId"]
        .as_str()
        .unwrap_or_else(|| panic!("workflow launch response has no runId: {body}"))
        .to_owned()
}
fn manifest(root: &Path, session: &str, run: &str) -> Value {
    serde_json::from_slice(
        &std::fs::read(
            root.join("workflow-sessions")
                .join(session)
                .join("workflows")
                .join(run)
                .join("state.json"),
        )
        .unwrap(),
    )
    .unwrap()
}
async fn scenario() {
    let dir = tempfile::tempdir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let sessions = dir.path().join("sessions");
    let guard = dir.path().join("cancel-switch");
    let started = dir.path().join("child-started");
    let drained = dir.path().join("child-drained");
    let mut args: Vec<String> = [
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
        "workflow-scope/local",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    args.extend([
        "--session-dir".into(),
        sessions.display().to_string(),
        "--extension".into(),
        root.join("extensions/pi-grok-workflows/index.ts")
            .display()
            .to_string(),
        "--extension".into(),
        root.join("crates/codegen/pi-grok-adapter/tests/fixtures/pi_workflow_scope.ts")
            .display()
            .to_string(),
    ]);
    let process = PiRpc::spawn(SpawnConfig {
        program: std::env::var("PI_BIN").unwrap_or_else(|_| "pi".into()),
        prefix_args: Vec::new(),
        cwd: dir.path().into(),
        pi_args: args,
        env: vec![
            (
                "PI_CODING_AGENT_DIR".into(),
                dir.path().join("pi").display().to_string(),
            ),
            ("PI_GROK_WORKFLOWS".into(), "1".into()),
            ("PI_TELEMETRY".into(), "0".into()),
            (
                "PI_WORKFLOW_SWITCH_GUARD".into(),
                guard.display().to_string(),
            ),
            (
                "PI_WORKFLOW_CHILD_STARTED".into(),
                started.display().to_string(),
            ),
            (
                "PI_WORKFLOW_CHILD_DRAINED".into(),
                drained.display().to_string(),
            ),
        ],
    })
    .await
    .unwrap();
    let rpc = process.rpc.clone();
    let bootstrap = PiBootstrap::load(&rpc).await.unwrap();
    let original = bootstrap.session_id().to_owned();
    let (mut client, channel) = acp_channels();
    let agent = Rc::new(
        PiAgent::new(
            process.rpc,
            channel.tx,
            bootstrap,
            sessions.clone(),
            None,
            None,
            None,
            None,
            None,
            true,
        )
        .unwrap(),
    );
    let consumer = tokio::task::spawn_local(async move {
        while let Some(message) = client.rx.recv().await {
            match message {
                AcpClientMessage::SessionNotification(args) => {
                    let _ = args.response_tx.send(Ok(()));
                }
                AcpClientMessage::ExtNotification(args) => {
                    let _ = args.response_tx.send(Ok(()));
                }
                other => panic!("unexpected client request: {other:?}"),
            }
        }
    });
    let event_agent = agent.clone();
    let events = tokio::task::spawn_local(async move {
        event_agent.run_events(process.events).await;
    });
    // Startup trust/catalog probes must not materialize a workflow persistence tree.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!sessions.join("workflow-sessions").exists());
    let run = launch(&agent, "scope-wait").await;
    wait_file(&started).await;
    assert_eq!(
        manifest(&sessions, &original, &run)["state"]["status"],
        "active"
    );
    std::fs::write(&guard, "cancel").unwrap();
    let cancelled = agent
        .new_session(acp::NewSessionRequest::new(dir.path().to_path_buf()))
        .await
        .unwrap();
    assert_eq!(cancelled.session_id.0.as_ref(), original);
    assert!(!drained.exists());
    assert_eq!(
        manifest(&sessions, &original, &run)["state"]["status"],
        "active"
    );
    std::fs::remove_file(&guard).unwrap();
    let next = agent
        .new_session(acp::NewSessionRequest::new(dir.path().to_path_buf()))
        .await
        .unwrap();
    let next_id = next.session_id.0.to_string();
    assert_ne!(next_id, original);
    assert!(
        drained.exists(),
        "scope replacement waits for real child abort and drain"
    );
    assert_eq!(
        manifest(&sessions, &original, &run)["state"]["status"],
        "cancelled"
    );
    let next_entries = rpc.request(json!({"type":"get_entries"})).await.unwrap();
    assert!(
        !next_entries.to_string().contains(&run),
        "outgoing workflow metadata entered the replacement Pi session"
    );
    assert!(!sessions.join("workflow-sessions").join(&next_id).exists());
    agent
        .prompt(acp::PromptRequest::new(
            next.session_id.clone(),
            vec![acp::ContentBlock::Text(acp::TextContent::new(
                "materialize target session",
            ))],
        ))
        .await
        .unwrap();
    let saved = rpc.request(json!({"type":"get_state"})).await.unwrap()["sessionFile"]
        .as_str()
        .unwrap()
        .to_owned();
    let third = agent
        .new_session(acp::NewSessionRequest::new(dir.path().to_path_buf()))
        .await
        .unwrap();
    let third_id = third.session_id.0.to_string();
    std::fs::remove_file(&started).unwrap();
    std::fs::remove_file(&drained).unwrap();
    let third_run = launch(&agent, "scope-wait").await;
    wait_file(&started).await;
    std::fs::write(&guard, "cancel").unwrap();
    assert!(
        agent
            .switch_session(Path::new(&saved), &next_id)
            .await
            .unwrap()
            .cancelled
    );
    assert!(!drained.exists());
    assert_eq!(
        manifest(&sessions, &third_id, &third_run)["state"]["status"],
        "active"
    );
    std::fs::remove_file(&guard).unwrap();
    assert!(
        !agent
            .switch_session(Path::new(&saved), &next_id)
            .await
            .unwrap()
            .cancelled
    );
    assert!(drained.exists());
    assert_eq!(
        manifest(&sessions, &third_id, &third_run)["state"]["status"],
        "cancelled"
    );
    let resumed_entries = rpc.request(json!({"type":"get_entries"})).await.unwrap();
    assert!(
        !resumed_entries.to_string().contains(&third_run),
        "outgoing workflow metadata entered the resumed Pi session"
    );
    assert!(!sessions.join("workflow-sessions").join(&next_id).exists());
    rpc.kill().await;
    events.abort();
    consumer.abort();
}

#[test]
#[ignore = "requires installed Pi 1.0; runs with isolated process environment"]
fn workflow_session_scope_real_pi() {
    // Isolate the parent adapter logger as well as Pi's own user/session roots.
    if std::env::var_os("PI_WORKFLOW_SCOPE_TEST_CHILD").is_none() {
        let dir = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "workflow_session_scope_real_pi",
                "--ignored",
                "--nocapture",
            ])
            .env("PI_WORKFLOW_SCOPE_TEST_CHILD", "1")
            .env("GROK_HOME", dir.path().join("grok"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(tokio::task::LocalSet::new().run_until(async {
        tokio::time::timeout(Duration::from_secs(75), scenario())
            .await
            .unwrap();
    }));
}
