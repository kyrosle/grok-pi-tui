//! Consume installed-Pi production adapter output through the native ACP receiver.
use super::*;
use crate::scrollback::block::BlockContent;
use crate::scrollback::types::{BlockContext, DisplayMode};

fn child_text(agent: &AgentView, child_id: &str) -> String {
    let child = agent
        .subagent_views
        .get(child_id)
        .expect("real ACP built child view");
    (0..child.scrollback.len())
        .map(|index| {
            let output = child
                .scrollback
                .get(index)
                .unwrap()
                .block
                .output(&BlockContext {
                    mode: DisplayMode::Expanded,
                    is_running: false,
                    width: 120,
                    raw: false,
                    max_lines: None,
                    appearance: Default::default(),
                    is_selected: false,
                    cwd: None,
                });
            output
                .lines
                .iter()
                .map(|line| {
                    line.content
                        .spans
                        .iter()
                        .map(|span| span.content.as_ref())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
#[ignore = "requires PI_NATIVE_CAPTURE from the installed-Pi production ACP fixture"]
fn actual_pi_child_live_and_persisted_replay_survive_native_open_close() {
    let capture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("PI_NATIVE_CAPTURE").expect("actual Pi ACP capture required"))
            .unwrap(),
    )
    .unwrap();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["scenario"] == "subagent")
        .expect("actual child case required");
    let root_id = case["rootSessionId"].as_str().unwrap();
    let child_id = case["childSessionId"].as_str().unwrap();
    for phase in ["live", "replay"] {
        let mut app = make_app_with_agent(root_id);
        app.external_agent = true;
        app.agents
            .get_mut(&AgentId(0))
            .unwrap()
            .session
            .loading_replay = phase == "replay";
        for event in case[phase].as_array().unwrap() {
            if event["kind"] == "session" {
                let notification: acp::SessionNotification =
                    serde_json::from_value(event["notification"].clone()).unwrap();
                let (tx, _rx) = tokio::sync::oneshot::channel();
                handle(
                    AcpClientMessage::SessionNotification(xai_acp_lib::AcpArgs {
                        request: notification,
                        response_tx: tx,
                    }),
                    &mut app,
                );
            } else if event["kind"] == "ext" {
                let notification = acp::ExtNotification::new(
                    event["method"].as_str().unwrap().to_owned(),
                    std::sync::Arc::from(
                        serde_json::value::to_raw_value(&event["params"]).unwrap(),
                    ),
                );
                handle_ext_notification(&notification, &mut app);
            }
        }
        let agent = app.agents.get_mut(&AgentId(0)).unwrap();
        assert_eq!(
            agent.subagent_sessions.len(),
            1,
            "{phase}: actual lifecycle must not duplicate rows"
        );
        let info = &agent.subagent_sessions[child_id];
        assert!(
            info.is_finished(),
            "{phase}: terminal child must remain completed"
        );
        assert!(matches!(
            info.transcript,
            crate::app::subagent::ChildTranscript::AdapterManaged
        ));
        let before = child_text(agent, child_id);
        assert_eq!(
            before.matches("NATIVE_CHILD_BODY").count(),
            1,
            "{phase}: body must render once: {before}"
        );
        agent.open_subagent_fullscreen(child_id.to_owned());
        assert_eq!(
            child_text(agent, child_id),
            before,
            "{phase}: opening must preserve Pi ACP history"
        );
        agent.close_subagent_fullscreen();
        assert_eq!(
            child_text(agent, child_id),
            before,
            "{phase}: closing must retain child without Grok disk proof"
        );
        agent.open_subagent_fullscreen(child_id.to_owned());
        assert_eq!(
            child_text(agent, child_id),
            before,
            "{phase}: reopening must preserve Pi history"
        );
    }
}
