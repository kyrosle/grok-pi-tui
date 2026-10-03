//! Reload RPC acknowledgement must prove the business action, not only command handling.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::{PiAgent, PiBootstrap, PiRpc, SpawnConfig};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};
use xai_acp_lib::{AcpClientMessage, acp_channels};

#[tokio::test(flavor = "current_thread")]
async fn handled_reload_with_false_missing_or_non_boolean_ack_never_reports_reloaded() {
    tokio::task::LocalSet::new().run_until(async {
        for mode in ["false", "missing", "not_boolean"] {
            let directory = tempfile::tempdir().unwrap();
            let trace = directory.path().join("rpc.jsonl");
            let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pi_reload_ack_rpc.py");
            let process = PiRpc::spawn(SpawnConfig {
                program: "python3".into(), prefix_args: vec![fixture.display().to_string()],
                cwd: directory.path().into(), pi_args: vec![],
                env: vec![("PI_RELOAD_ACK_MODE".into(), mode.into()), ("PI_RELOAD_ACK_TRACE".into(), trace.display().to_string())],
            }).await.unwrap();
            let rpc = process.rpc.clone();
            let bootstrap = PiBootstrap::load(&rpc).await.unwrap();
            let (mut client, channel) = acp_channels();
            let agent = PiAgent::new(rpc.clone(), channel.tx, bootstrap, directory.path().join("sessions"), None, None, None, None, None, false, false).unwrap();
            let notifications = Rc::new(RefCell::new(Vec::<String>::new()));
            let captured = notifications.clone();
            let sink = tokio::task::spawn_local(async move {
                while let Some(message) = client.rx.recv().await {
                    match message {
                        AcpClientMessage::SessionNotification(args) => { let _ = args.response_tx.send(Ok(())); }
                        AcpClientMessage::ExtNotification(args) => {
                            captured.borrow_mut().push(args.request.method.to_string());
                            let _ = args.response_tx.send(Ok(()));
                        }
                        other => panic!("unexpected client request: {other:?}"),
                    }
                }
            });
            let params = serde_json::value::to_raw_value(&json!({"sessionId":"reload-ack"})).unwrap();
            let result = tokio::time::timeout(Duration::from_secs(3), agent.ext_method(acp::ExtRequest::new("pi/session/reload", params.into()))).await.unwrap();
            assert!(result.is_err(), "handled {mode} reload ACK must not become reloaded:true");
            if mode == "false" { assert!(serde_json::to_string(&result.unwrap_err()).unwrap().contains("fixture ctx.reload rejected")); }
            let records: Vec<Value> = std::fs::read_to_string(&trace).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect();
            assert_eq!(records.iter().filter(|row| row["type"] == "get_commands").count(), 1,
                "failed business ACK must not refresh/publish the old registry as a successful reload");
            assert_eq!(records.iter().filter(|row| row["type"] == "prompt").count(), 1);
            assert!(!notifications.borrow().iter().any(|method| method == "pi/session/reloaded"));
            rpc.kill().await;
            sink.abort();
        }
    }).await;
}
