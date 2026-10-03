use super::*;
use crate::app::actions::PiControlMethod;
use serde_json::json;

#[test]
fn pi_control_effect_is_typed_and_bound_to_the_active_session() {
    let mut app = test_app_with_agent();
    app.external_agent = true;
    let effects = dispatch(
        Action::PiControlRequest {
            method: PiControlMethod::RuntimeControl,
            params: json!({"operation":"retry","enabled":false}),
        },
        &mut app,
    );
    assert!(
        matches!(&effects[..], [Effect::PiControlRequest { agent_id: AgentId(0), session_id, method:PiControlMethod::RuntimeControl, params }]
        if session_id.0.as_ref() == "test-session" && params["enabled"] == false && params["sessionId"] == "test-session")
    );
    app.external_agent = false;
    assert!(
        dispatch(
            Action::PiControlRequest {
                method: PiControlMethod::RuntimeControl,
                params: json!({})
            },
            &mut app
        )
        .is_empty()
    );
}

#[test]
fn pi_control_result_from_an_old_session_cannot_touch_the_new_view() {
    let mut app = test_app_with_agent();
    let before = app.agents[&AgentId(0)].scrollback.len();
    dispatch_task_result(
        TaskResult::PiControlComplete {
            agent_id: AgentId(0),
            session_id: "old-session".into(),
            method: PiControlMethod::RuntimeControl,
            result: Ok(json!({"message":"configured retry off"})),
        },
        &mut app,
    );
    assert_eq!(app.agents[&AgentId(0)].scrollback.len(), before);
    assert!(app.agents[&AgentId(0)].toast.is_none());
    dispatch_task_result(
        TaskResult::PiControlComplete {
            agent_id: AgentId(0),
            session_id: "test-session".into(),
            method: PiControlMethod::RuntimeControl,
            result: Ok(json!({"message":"configured retry off"})),
        },
        &mut app,
    );
    assert_eq!(app.agents[&AgentId(0)].scrollback.len(), before + 1);
}
