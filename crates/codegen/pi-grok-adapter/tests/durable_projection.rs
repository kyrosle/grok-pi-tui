//! Real published Durable SDK → headless host → production ACP. No paid inference.
use agent_client_protocol::{self as acp, Agent};
use pi_grok_adapter::durable::{DurableAgent, DurableRpc};
use serde_json::{Value, json};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};
use xai_acp_lib::{AcpClientMessage, acp_channels};

#[tokio::test(flavor = "current_thread")]
#[ignore = "Requires npm ci in runtime/pi-durable-host; run explicitly for Durable changes"]
async fn durable_sdk_to_acp_stream_tools_and_resume() {
    tokio::task::LocalSet::new().run_until(async {
        let dir = tempfile::tempdir().unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_path_buf();
        let options = json!({"home":dir.path(),"cwd":dir.path(),"agentDir":dir.path().join("pi"),"noContextFiles":true,"noSkills":true});
        std::fs::write(dir.path().join("fixture.txt"), "DURABLE_FILE_CONTENT").unwrap();
        let (rpc, events) = DurableRpc::spawn(&root.join("runtime/pi-durable-host/test/fixture.mjs"), dir.path(), options.clone()).await.unwrap();
        let boot = rpc.request("hello", json!({})).await.unwrap();
        assert_eq!(boot["sdkVersion"], "1.1.0");
        let locator = boot["session"].as_str().unwrap().to_string();
        let (mut client, channel) = acp_channels();
        let agent = Rc::new(DurableAgent::new(rpc.clone(), channel.tx, boot));
        assert!(agent.models().is_some());
        let captured = Rc::new(RefCell::new(Vec::new())); let sink = captured.clone();
        let consumer = tokio::task::spawn_local(async move {
            while let Some(message) = client.rx.recv().await { match message {
                AcpClientMessage::SessionNotification(args) => { sink.borrow_mut().push(json!({"notification":args.request})); let _ = args.response_tx.send(Ok(())); },
                AcpClientMessage::ExtNotification(args) => { sink.borrow_mut().push(json!({"method":args.request.method,"params":serde_json::from_str::<Value>(args.request.params.get()).unwrap()})); let _ = args.response_tx.send(Ok(())); },
                other => panic!("unexpected UI request: {other:?}"),
            } }
        });
        let event_agent = agent.clone(); let runner = tokio::task::spawn_local(async move { event_agent.run_events(events).await; });
        agent.load_session(acp::LoadSessionRequest::new(locator.clone(), dir.path().to_path_buf())).await.unwrap();
        let prompt = acp::PromptRequest::new(locator.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new("hello durable"))]);
        let response = tokio::time::timeout(Duration::from_secs(15), agent.prompt(prompt)).await.unwrap().unwrap();
        assert_eq!(response.stop_reason, acp::StopReason::EndTurn);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let text = captured.borrow().iter().filter(|row| row["notification"]["update"]["sessionUpdate"] == "agent_message_chunk").map(|row| row["notification"]["update"]["content"]["text"].as_str().unwrap_or_default().to_owned()).collect::<String>();
        assert_eq!(text, "DURABLE_REPLY hello durable");
        agent.prompt(acp::PromptRequest::new(locator.clone(), vec![acp::ContentBlock::Text(acp::TextContent::new("read fixture"))])).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let serialized = serde_json::to_string(&*captured.borrow()).unwrap();
        std::fs::write("/tmp/grok-pi-durable-acp.json", &serialized).unwrap();
        assert!(serialized.contains("DURABLE_FILE_CONTENT"));
        assert!(serialized.contains("tool_call_update") && serialized.contains("completed"));
        assert!(serialized.contains("pi/ui/durable/tasks"));
        let old_locator = locator.clone();
        let fresh = agent.new_session(acp::NewSessionRequest::new(dir.path().to_path_buf())).await.unwrap();
        let fresh_locator = fresh.session_id.0.to_string();
        agent.load_session(acp::LoadSessionRequest::new(fresh_locator.clone(), dir.path().to_path_buf())).await.unwrap();
        agent.prompt(acp::PromptRequest::new(fresh_locator, vec![acp::ContentBlock::Text(acp::TextContent::new("fresh conversation"))])).await.unwrap();
        captured.borrow_mut().clear();
        agent.load_session(acp::LoadSessionRequest::new(old_locator, dir.path().to_path_buf())).await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let replayed = serde_json::to_string(&*captured.borrow()).unwrap();
                if replayed.contains("DURABLE_REPLY hello durable") && replayed.contains("DURABLE_FILE_CONTENT") { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("switched-session snapshot must reach ACP");
        rpc.request("close", json!({})).await.unwrap(); runner.abort(); consumer.abort();
        let (rpc, _) = DurableRpc::spawn(&root.join("runtime/pi-durable-host/test/fixture.mjs"), dir.path(), json!({"session":locator,"home":dir.path(),"cwd":dir.path(),"agentDir":dir.path().join("pi")})).await.unwrap();
        let snapshot = rpc.request("hello", json!({})).await.unwrap();
        assert!(snapshot["snapshot"]["entries"].as_array().unwrap().len() >= 4);
        rpc.request("close", json!({})).await.unwrap();
    }).await;
}

#[tokio::test]
#[ignore = "Requires npm ci in runtime/pi-durable-host; run explicitly for Durable changes"]
async fn durable_background_owner_survives_transport_close() {
    let dir = tempfile::tempdir().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    let host = root.join("runtime/pi-durable-host/test/fixture.mjs");
    let options = json!({"backgroundOwner":true,"home":dir.path(),"cwd":dir.path(),"agentDir":dir.path().join("pi"),"noSkills":true,"noContextFiles":true});
    let (first, _) = DurableRpc::spawn(&host, dir.path(), options.clone())
        .await
        .unwrap();
    let boot = first.request("hello", json!({})).await.unwrap();
    assert_eq!(boot["backgroundOwner"], true);
    let input = first
        .request(
            "submit",
            json!({"text":"background durable","requestId":"background-rust-1"}),
        )
        .await
        .unwrap();
    first.request("close", json!({})).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut resume = options;
    resume["session"] = boot["session"].clone();
    let (second, _) = DurableRpc::spawn(&host, dir.path(), resume).await.unwrap();
    assert_eq!(
        second
            .request("wait", json!({"id":input["id"]}))
            .await
            .unwrap()["status"],
        "done"
    );
    second.request("stop", json!({})).await.unwrap();
}
