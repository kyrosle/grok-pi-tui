use super::*;
use acp::Agent;

const RPC: &str = r#"
import json, os, sys, time
for line in sys.stdin:
    request = json.loads(line)
    kind = request['type']
    data = {}
    if kind == 'get_state':
        with open(os.environ['PI_FOCUS_STATE_MARK'], 'w') as mark:
            mark.write('requested')
        time.sleep(float(os.environ.get('PI_FOCUS_STATE_DELAY', '0')))
        data = {'sessionId':'handled-review','isStreaming':False,'isCompacting':False}
    elif kind == 'get_session_stats':
        data = {'contextUsage':{'tokens':0,'contextWindow':32768}}
    elif kind == 'get_available_models':
        data = {'models':[]}
    if request.get('id'):
        print(json.dumps({'type':'response','id':request['id'],'command':kind,'success':True,'data':data}), flush=True)
"#;

struct Fixture {
    directory: tempfile::TempDir,
    agent: PiAgent,
    completions: Rc<RefCell<Vec<Value>>>,
    sink: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new(state_delay: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let process = PiRpc::spawn(crate::pi_rpc::SpawnConfig {
            program: "python3".into(),
            prefix_args: vec!["-u".into(), "-c".into(), RPC.into()],
            cwd: directory.path().into(),
            pi_args: vec![],
            env: vec![
                (
                    "PI_FOCUS_STATE_MARK".into(),
                    directory
                        .path()
                        .join("state-requested")
                        .display()
                        .to_string(),
                ),
                ("PI_FOCUS_STATE_DELAY".into(), state_delay.into()),
            ],
        })
        .await
        .unwrap();
        let (mut client, channel) = xai_acp_lib::acp_channels();
        let agent = PiAgent::new(
            process.rpc,
            channel.tx,
            PiBootstrap {
                state: PiState {
                    session_id: "handled-review".into(),
                    ..Default::default()
                },
                models: vec![],
                commands: vec![],
            },
            directory.path().join("sessions"),
            None,
            None,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        let completions = Rc::new(RefCell::new(Vec::new()));
        let captured = completions.clone();
        let sink = tokio::task::spawn_local(async move {
            while let Some(message) = client.rx.recv().await {
                match message {
                    AcpClientMessage::SessionNotification(args) => {
                        let _ = args.response_tx.send(Ok(()));
                    }
                    AcpClientMessage::ExtNotification(args) => {
                        if args.request.method.as_ref() == "x.ai/session/prompt_complete" {
                            captured
                                .borrow_mut()
                                .push(serde_json::from_str(args.request.params.get()).unwrap());
                        }
                        let _ = args.response_tx.send(Ok(()));
                    }
                    other => panic!("unexpected client request: {other:?}"),
                }
            }
        });
        Self {
            directory,
            agent,
            completions,
            sink,
        }
    }

    fn running(&self, entry: QueueEntry) {
        let mut state = self.agent.state.borrow_mut();
        state.agent_running = true;
        state.live_prompt_id = Some(entry.id.clone());
        state.queue_mirror.set_running(entry);
    }

    async fn close(self) {
        self.agent.rpc.kill().await;
        self.sink.abort();
    }
}

fn entry(id: &str, origin: QueueOrigin) -> QueueEntry {
    QueueEntry {
        id: id.into(),
        execution_text: format!("execute-{id}"),
        display_text: format!("display-{id}"),
        images: vec![],
        version: 0,
        lane: QueueLane::FollowUp,
        origin,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn pi_side_aborted_settle_cancels_waiters_without_dispatching_a_successor() {
    tokio::task::LocalSet::new().run_until(async {
        let fixture = Fixture::new("0").await;
        fixture.running(entry("A", QueueOrigin::Extension));
        let (completion, mut waiter) = oneshot::channel();
        {
            let mut state = fixture.agent.state.borrow_mut();
            state.active_prompts.push(ActivePrompt {
                id: 1, client_prompt_id: Some("A".into()), completion,
                agent_started: true, cancelled: false,
            });
            state.queue_mirror.reserve("B".into(), "execute-B".into(), "display-B".into(), vec![], QueueLane::FollowUp, QueueOrigin::Client);
        }
        fixture.agent.handle_event(json!({"type":"agent_settled","aborted":true})).await.unwrap();
        assert_eq!(waiter.try_recv().unwrap().reason, acp::StopReason::Cancelled);
        assert!(!fixture.agent.state.borrow().agent_running);
        assert!(fixture.agent.state.borrow().queue_mirror.running().is_none());
        assert_eq!(fixture.completions.borrow()[0]["stopReason"], "cancelled");
        fixture.close().await;
    }).await;
}

#[tokio::test(flavor = "current_thread")]
async fn handled_after_cancel_does_not_complete_the_server_entry_twice() {
    tokio::task::LocalSet::new()
        .run_until(async {
            for origin in [QueueOrigin::Extension, QueueOrigin::Pi] {
                let fixture = Fixture::new("0").await;
                let original = entry("A", origin);
                fixture.running(original.clone());
                fixture
                    .agent
                    .cancel(acp::CancelNotification::new(fixture.agent.session_id()))
                    .await
                    .unwrap();
                fixture
                    .agent
                    .apply_prompt_response(
                        &json!({"disposition":"handled"}),
                        None,
                        Some(&original),
                        true,
                    )
                    .await;
                let completions = fixture.completions.borrow().clone();
                assert_eq!(
                    completions.len(),
                    1,
                    "late handled must not replace cancellation: {completions:?}"
                );
                assert_eq!(completions[0]["promptId"], "A");
                assert_eq!(completions[0]["stopReason"], "cancelled");
                fixture.close().await;
            }
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn handled_after_settled_does_not_finish_the_successor_waiter_or_server_entry() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let fixture = Fixture::new("0").await;
            let original = entry("A", QueueOrigin::Extension);
            fixture.running(original.clone());
            fixture
                .agent
                .handle_event(json!({"type":"agent_settled"}))
                .await
                .unwrap();
            let successor = entry("B", QueueOrigin::Client);
            fixture.running(successor);
            let (completion, mut waiter) = oneshot::channel();
            fixture
                .agent
                .state
                .borrow_mut()
                .active_prompts
                .push(ActivePrompt {
                    id: 2,
                    client_prompt_id: Some("B".into()),
                    completion,
                    agent_started: true,
                    cancelled: false,
                });
            fixture.agent.state.borrow_mut().turn_start_ms = Some(utc_now_ms());
            fixture
                .agent
                .apply_prompt_response(
                    &json!({"disposition":"handled"}),
                    None,
                    Some(&original),
                    true,
                )
                .await;
            assert!(matches!(
                waiter.try_recv(),
                Err(oneshot::error::TryRecvError::Empty)
            ));
            let state = fixture.agent.state.borrow();
            assert!(state.agent_running);
            assert_eq!(state.queue_mirror.running().unwrap().id, "B");
            assert_eq!(state.active_prompts.len(), 1);
            drop(state);
            let completions = fixture.completions.borrow().clone();
            assert_eq!(
                completions.len(),
                1,
                "settle already completed A: {completions:?}"
            );
            assert_eq!(completions[0]["promptId"], "A");
            assert_eq!(completions[0]["stopReason"], "end_turn");
            fixture.close().await;
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn handled_running_a_preserves_b_reservation_identity_and_waiter() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let fixture = Fixture::new("0").await;
            let original = entry("A", QueueOrigin::Extension);
            fixture.running(original.clone());
            let (completion, mut waiter) = oneshot::channel();
            {
                let mut state = fixture.agent.state.borrow_mut();
                state.active_prompts.push(ActivePrompt {
                    id: 2,
                    client_prompt_id: Some("B".into()),
                    completion,
                    agent_started: false,
                    cancelled: false,
                });
                state.queue_mirror.reserve(
                    "B".into(),
                    "execute-B".into(),
                    "display-B".into(),
                    vec![],
                    QueueLane::Steering,
                    QueueOrigin::Client,
                );
            }
            fixture
                .agent
                .apply_prompt_response(
                    &json!({"disposition":"handled"}),
                    None,
                    Some(&original),
                    true,
                )
                .await;
            assert!(matches!(
                waiter.try_recv(),
                Err(oneshot::error::TryRecvError::Empty)
            ));
            let mut state = fixture.agent.state.borrow_mut();
            assert!(state.agent_running, "B still owns an unresolved preflight");
            state
                .queue_mirror
                .apply_queue_update(&["execute-B".into()], &[]);
            let snapshot = state.queue_mirror.snapshot();
            assert_eq!(snapshot.entries.len(), 1);
            assert_eq!(
                snapshot.entries[0]["id"], "B",
                "Pi must retain the reserved client id"
            );
            assert_eq!(snapshot.entries[0]["text"], "display-B");
            drop(state);
            assert_eq!(fixture.completions.borrow().len(), 1);
            fixture.close().await;
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn legacy_a_probe_cannot_finish_b_after_its_get_state_reply_becomes_stale() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let fixture = Fixture::new("0.12").await;
            fixture.running(entry("A", QueueOrigin::Client));
            let (completion_a, _waiter_a) = oneshot::channel();
            fixture
                .agent
                .state
                .borrow_mut()
                .active_prompts
                .push(ActivePrompt {
                    id: 1,
                    client_prompt_id: Some("A".into()),
                    completion: completion_a,
                    agent_started: false,
                    cancelled: false,
                });
            fixture
                .agent
                .apply_prompt_response(&json!({}), Some(1), None, true)
                .await;
            let marker = fixture.directory.path().join("state-requested");
            tokio::time::timeout(Duration::from_secs(2), async {
                while !marker.exists() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("legacy probe must have started its asynchronous get_state");
            let (completion_b, mut waiter_b) = oneshot::channel();
            {
                let mut state = fixture.agent.state.borrow_mut();
                state.active_prompts.clear();
                state.active_prompts.push(ActivePrompt {
                    id: 2,
                    client_prompt_id: Some("B".into()),
                    completion: completion_b,
                    agent_started: false,
                    cancelled: false,
                });
                state
                    .queue_mirror
                    .set_running(entry("B", QueueOrigin::Client));
                state.live_prompt_id = Some("B".into());
            }
            fixture
                .agent
                .rpc
                .request(json!({"type":"get_available_models"}))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            assert!(matches!(
                waiter_b.try_recv(),
                Err(oneshot::error::TryRecvError::Empty)
            ));
            let state = fixture.agent.state.borrow();
            assert!(state.agent_running);
            assert_eq!(state.queue_mirror.running().unwrap().id, "B");
            assert_eq!(state.live_prompt_id.as_deref(), Some("B"));
            assert_eq!(state.active_prompts[0].id, 2);
            drop(state);
            fixture.close().await;
        })
        .await;
}
