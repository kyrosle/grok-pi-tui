//! Installed Pi → production adapter ACP. Synthetic provider; no model or credentials.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::{PiAgent, PiBootstrap, PiRpc, SpawnConfig, SubagentEventTransport};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};
use tokio::sync::oneshot;
use xai_acp_lib::{AcpClientMessage, AcpResult, acp_channels};

fn updates<'a>(events: &'a [Value], tag: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter_map(|event| event.get("notification")?.get("update"))
        .filter(|update| update.get("sessionUpdate").and_then(Value::as_str) == Some(tag))
        .collect()
}

fn completed<'a>(events: &'a [Value], id: &str) -> &'a Value {
    let items: Vec<_> = updates(events, "tool_call_update")
        .into_iter()
        .filter(|update| {
            update.get("toolCallId").and_then(Value::as_str) == Some(id)
                && matches!(
                    update.get("status").and_then(Value::as_str),
                    Some("completed" | "failed")
                )
        })
        .collect();
    assert_eq!(
        items.len(),
        1,
        "each tool has exactly one terminal update: {id}"
    );
    assert_eq!(items[0]["status"], "completed", "tool failed: {}", items[0]);
    items[0]
}

async fn scenario(name: &str, eval_only: bool) -> Value {
    let directory = tempfile::tempdir().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let child_case = name == "subagent";
    let transport = child_case.then(|| SubagentEventTransport::bind().unwrap());
    let fixture = root.join(if child_case {
        "crates/codegen/pi-grok-adapter/tests/fixtures/pi_subagent_provider.ts"
    } else {
        "crates/codegen/pi-grok-adapter/tests/fixtures/pi_render_tools.ts"
    });
    let mut args: Vec<String> = [
        "--mode",
        "rpc",
        "--offline",
        "-ne",
        "-ns",
        "-np",
        "-nc",
        "--no-themes",
        "--no-session",
        "--no-approve",
        "--model",
        "pi-render/local",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    if child_case {
        args.retain(|argument| argument != "--no-session");
        let model = args
            .iter()
            .position(|argument| argument == "--model")
            .unwrap()
            + 1;
        args[model] = "pi-child-fixture/local".into();
        args.extend([
            "--session-dir".into(),
            directory.path().join("sessions").display().to_string(),
            "--extension".into(),
            root.join("extensions/pi-grok-subagents/index.ts")
                .display()
                .to_string(),
            "--tools".into(),
            "spawn_subagent".into(),
        ]);
    }
    args.extend([
        "--extension".into(),
        root.join("extensions/pi-grok-bash/index.ts")
            .display()
            .to_string(),
        "--extension".into(),
        fixture.display().to_string(),
    ]);
    if name == "codemode" {
        args.extend([
            "--extension".into(),
            "builtin:codemode".into(),
            "--tools".into(),
            "codemode,fixture_note".into(),
        ]);
    }
    let process = PiRpc::spawn(SpawnConfig {
        program: std::env::var("PI_BIN").unwrap_or_else(|_| "pi".into()),
        prefix_args: Vec::new(),
        cwd: directory.path().to_path_buf(),
        pi_args: args,
        env: vec![
            (
                "PI_CODING_AGENT_DIR".into(),
                directory.path().join("pi").display().to_string(),
            ),
            (
                "GROK_HOME".into(),
                directory.path().join("grok").display().to_string(),
            ),
            ("PI_OFFLINE".into(), "1".into()),
            ("PI_TELEMETRY".into(), "0".into()),
            ("PI_GROK_EVAL_VERSION".into(), "v2".into()),
            (
                "PI_GROK_EVAL_V2_ONLY".into(),
                if eval_only { "1" } else { "0" }.into(),
            ),
            ("PI_GROK_EVAL_MCP".into(), "0".into()),
            (
                "PI_GROK_SUBAGENTS".into(),
                if child_case { "1" } else { "0" }.into(),
            ),
            (
                "PI_GROK_SUBAGENT_SOCKET".into(),
                transport
                    .as_ref()
                    .map(|transport| transport.endpoint().to_string())
                    .unwrap_or_default(),
            ),
        ]
        .into_iter()
        .chain(std::env::vars().filter_map(|(key, _)| {
            (key.ends_with("_API_KEY")
                || key.ends_with("_TOKEN")
                || key == "AWS_ACCESS_KEY_ID"
                || key == "AWS_SECRET_ACCESS_KEY")
                .then_some((key, String::new()))
        }))
        .collect(),
    })
    .await
    .unwrap();
    let rpc = process.rpc.clone();
    let bootstrap = PiBootstrap::load(&rpc).await.unwrap();
    let session_id = acp::SessionId::new(bootstrap.session_id().to_string());
    let (mut client, agent_channel) = acp_channels();
    let agent = Rc::new(
        PiAgent::new(
            process.rpc,
            agent_channel.tx,
            bootstrap,
            directory.path().join("sessions"),
            None,
            None,
            None,
            None,
            transport,
            false,
            eval_only,
        )
        .unwrap(),
    );
    let captured = Rc::new(RefCell::new(Vec::new()));
    // Hold the real reverse request unresolved, as an open native dialog does.
    let questions = Rc::new(RefCell::new(Vec::<
        oneshot::Sender<AcpResult<acp::ExtResponse>>,
    >::new()));
    let sink = captured.clone();
    let held = questions.clone();
    let consumer = tokio::task::spawn_local(async move {
        while let Some(message) = client.rx.recv().await {
            match message {
                AcpClientMessage::SessionNotification(args) => {
                    sink.borrow_mut()
                        .push(json!({"kind":"session", "notification":args.request}));
                    let _ = args.response_tx.send(Ok(()));
                }
                AcpClientMessage::ExtNotification(args) => {
                    sink.borrow_mut().push(json!({"kind":"ext", "method":args.request.method,
                        "params":serde_json::from_str::<Value>(args.request.params.get()).unwrap()}));
                    let _ = args.response_tx.send(Ok(()));
                }
                AcpClientMessage::ExtMethod(args) => {
                    assert_eq!(args.request.method.as_ref(), "x.ai/ask_user_question");
                    sink.borrow_mut().push(json!({"kind":"question", "params":serde_json::from_str::<Value>(args.request.params.get()).unwrap()}));
                    held.borrow_mut().push(args.response_tx);
                }
                other => panic!("unexpected native client request: {other:?}"),
            }
        }
    });
    let events_agent = agent.clone();
    let events = tokio::task::spawn_local(async move {
        events_agent.run_events(process.events).await;
    });
    let result = if matches!(name, "signal" | "timeout" | "eof") {
        let _ = rpc
            .request(json!({"type":"prompt", "message":format!("/fixture-dialog {name}")}))
            .await;
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if captured.borrow().iter().any(|event| {
                    event.get("method").and_then(Value::as_str) == Some("pi/ui/cancel_interaction")
                }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("scoped auth dialog was not retracted");
        let snapshot = captured.borrow().clone();
        let opened: Vec<_> = snapshot
            .iter()
            .filter(|event| event["kind"] == "question")
            .collect();
        let cancelled: Vec<_> = snapshot
            .iter()
            .filter(|event| event["method"] == "pi/ui/cancel_interaction")
            .collect();
        assert_eq!(opened.len(), 1);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(
            opened[0]["params"]["toolCallId"],
            cancelled[0]["params"]["toolCallId"]
        );
        let question = format!(
            "NATIVE_AUTH_{}{}",
            name.to_uppercase(),
            if name == "signal" {
                ""
            } else {
                "\n\nFixture input"
            }
        );
        assert_eq!(opened[0]["params"]["questions"][0]["question"], question);
        json!({"scenario":name,"events":snapshot})
    } else {
        tokio::time::timeout(
            Duration::from_secs(30),
            agent.prompt(acp::PromptRequest::new(
                session_id.clone(),
                vec![acp::ContentBlock::Text(acp::TextContent::new(format!(
                    "render-{name}"
                )))],
            )),
        )
        .await
        .unwrap()
        .unwrap();
        if child_case {
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    let ready = {
                        let snapshot = captured.borrow();
                        snapshot.iter().any(|event| {
                            event["params"]["update"]["sessionUpdate"] == "subagent_finished"
                        }) && snapshot.iter().any(|event| {
                            event["notification"]["update"]["content"]["text"]
                                == "NATIVE_CHILD_BODY"
                        })
                    };
                    if ready {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("live child ACP/socket stream did not drain");
        }
        let live = captured.borrow().clone();
        captured.borrow_mut().clear();
        agent
            .load_session(acp::LoadSessionRequest::new(
                session_id,
                directory.path().to_path_buf(),
            ))
            .await
            .unwrap();
        let replay = captured.borrow().clone();
        if child_case {
            let spawn = live
                .iter()
                .find(|event| event["params"]["update"]["sessionUpdate"] == "subagent_spawned")
                .expect("actual production extension emitted native child lifecycle");
            let child_id = spawn["params"]["update"]["child_session_id"]
                .as_str()
                .unwrap();
            for (phase, notifications) in [("live", &live), ("replay", &replay)] {
                let bodies: Vec<_> = notifications
                    .iter()
                    .filter(|event| {
                        event["notification"]["sessionId"] == child_id
                            && event["notification"]["update"]["sessionUpdate"]
                                == "agent_message_chunk"
                            && event["notification"]["update"]["content"]["text"]
                                == "NATIVE_CHILD_BODY"
                    })
                    .collect();
                assert_eq!(
                    bodies.len(),
                    1,
                    "{phase}: actual child body must reach ACP once"
                );
                let finished: Vec<_> = notifications
                    .iter()
                    .filter(|event| {
                        event["params"]["update"]["sessionUpdate"] == "subagent_finished"
                            && event["params"]["update"]["child_session_id"] == child_id
                    })
                    .collect();
                assert_eq!(
                    finished.len(),
                    1,
                    "{phase}: actual child has one terminal lifecycle"
                );
                assert_eq!(finished[0]["params"]["update"]["status"], "completed");
            }
            json!({"scenario":name,"rootSessionId":spawn["params"]["sessionId"],"childSessionId":child_id,"live":live,"replay":replay})
        } else {
            let starts = updates(&live, "tool_call");
            let title = if name == "codemode" {
                "codemode"
            } else {
                "fixture_note"
            };
            let matching: Vec<_> = starts
                .into_iter()
                .filter(|update| update["title"] == title)
                .collect();
            assert_eq!(
                matching.len(),
                1,
                "live bridge snapshots must not duplicate the real nested call"
            );
            let id = matching[0]["toolCallId"].as_str().unwrap();
            let replay_starts: Vec<_> = updates(&replay, "tool_call")
                .into_iter()
                .filter(|update| update["toolCallId"] == id)
                .collect();
            assert_eq!(replay_starts.len(), 1);
            let live_end = completed(&live, id);
            let replay_end = completed(&replay, id);
            assert_eq!(
                live_end["rawOutput"], replay_end["rawOutput"],
                "live/replay complete envelopes differ"
            );
            assert_eq!(
                live_end["content"], replay_end["content"],
                "live/replay typed content differs"
            );
            assert!(
                live_end["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["content"]["type"] == "image")
            );
            if name == "codemode" {
                assert_eq!(live_end["rawOutput"]["calls"].as_array().unwrap().len(), 1);
                assert!(
                    updates(&live, "tool_call")
                        .iter()
                        .all(|update| update["title"] != "fixture_note")
                );
            } else {
                assert_eq!(live_end["rawOutput"]["structuredContent"]["value"], "eval");
                assert_eq!(
                    live_end["rawOutput"]["details"]["resource"]["text"],
                    "NATIVE_RESOURCE"
                );
            }
            json!({"scenario":name,"evalOnly":eval_only,"live":live,"replay":replay})
        }
    };
    rpc.kill().await;
    events.abort();
    consumer.abort();
    result
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "installed Pi 1.0 runtime fixture; run explicitly with isolated capture output"]
async fn installed_pi_live_replay_and_dialog_teardown() {
    let home = tempfile::tempdir().unwrap();
    // This integration binary has one test; its process-scoped config stays isolated.
    unsafe {
        std::env::set_var("GROK_HOME", home.path());
        std::env::set_var("GROK_PROJECT_DIR", home.path().join("project"));
        std::env::set_var("PI_GROK_RPC_WATCHDOG", "0");
    }
    let mut cases = Vec::new();
    for (name, eval_only) in [
        ("eval", false),
        ("eval", true),
        ("codemode", false),
        ("subagent", false),
        ("signal", false),
        ("timeout", false),
        ("eof", false),
    ] {
        cases.push(
            tokio::task::LocalSet::new()
                .run_until(scenario(name, eval_only))
                .await,
        );
    }
    if let Ok(path) = std::env::var("PI_NATIVE_CAPTURE") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({"cases":cases})).unwrap(),
        )
        .unwrap();
    }
    eprintln!(
        "PASS: 7 actual Pi live/replay ACP cases including registered-provider child and scoped signal/timeout/EOF teardown"
    );
}
