//! Authoritative disposition vs delayed/event-first RPC lifecycle.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::{PiAgent, PiBootstrap, PiRpc, SpawnConfig};
use serde_json::Value;
use std::{path::PathBuf, rc::Rc, time::Duration};
use xai_acp_lib::{AcpClientMessage, acp_channels};

#[tokio::test(flavor = "current_thread")]
async fn disposition_controls_completion_without_a_speculative_idle_probe() {
    tokio::task::LocalSet::new()
        .run_until(async {
            for case in ["handled", "slow_started", "queued", "event_first"] {
                let dir = tempfile::tempdir().unwrap();
                let trace = dir.path().join("trace.jsonl");
                let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/pi_disposition_rpc.py");
                let process = PiRpc::spawn(SpawnConfig {
                    program: "python3".into(),
                    prefix_args: vec![fixture.display().to_string()],
                    cwd: dir.path().into(),
                    pi_args: vec![],
                    env: vec![("PI_DISPOSITION_TRACE".into(), trace.display().to_string())],
                })
                .await
                .unwrap();
                let rpc = process.rpc.clone();
                let bootstrap = PiBootstrap::load(&rpc).await.unwrap();
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
                        false,
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
                let events_agent = agent.clone();
                let events = tokio::task::spawn_local(async move {
                    events_agent.run_events(process.events).await;
                });
                let mut meta = acp::Meta::new();
                meta.insert("promptId".into(), serde_json::json!(case));
                let response = tokio::time::timeout(
                    Duration::from_secs(3),
                    agent.prompt(
                        acp::PromptRequest::new(
                            "disposition",
                            vec![acp::ContentBlock::Text(acp::TextContent::new(case))],
                        )
                        .meta(Some(meta)),
                    ),
                )
                .await
                .expect("prompt must complete")
                .unwrap();
                assert_eq!(response.stop_reason, acp::StopReason::EndTurn, "{case}");
                let records: Vec<Value> = std::fs::read_to_string(&trace)
                    .unwrap()
                    .lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect();
                assert_eq!(
                    records
                        .iter()
                        .filter(|row| row["command"] == "get_state")
                        .count(),
                    1,
                    "Pi 1.0 disposition must not schedule an idle probe: {case}"
                );
                assert_eq!(
                    records
                        .iter()
                        .filter(|row| row["event"] == "agent_settled")
                        .count(),
                    usize::from(case != "handled"),
                    "completion must follow actual work: {case}"
                );
                rpc.kill().await;
                events.abort();
                consumer.abort();
            }
        })
        .await;
}
