use super::*;

fn runtime_command(params: &Value) -> Result<Option<Value>, acp::Error> {
    match string(params, &["operation"]).unwrap_or("status") {
        "status" => Ok(None),
        "cancel-retry" => Ok(Some(json!({"type":"abort_retry"}))),
        operation @ ("retry" | "compaction") => {
            let enabled = params
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(|| acp::Error::invalid_params().data("enabled must be a boolean"))?;
            Ok(Some(json!({
                "type": if operation == "retry" {"set_auto_retry"} else {"set_auto_compaction"},
                "enabled": enabled,
            })))
        }
        _ => Err(acp::Error::invalid_params()
            .data("operation must be status, retry, compaction or cancel-retry")),
    }
}

impl PiAgent {
    pub(super) async fn runtime_control(&self, params: &Value) -> Result<Value, acp::Error> {
        let command = runtime_command(params)?;
        if let Some(command) = command {
            self.rpc.request(command).await.map_err(acp_internal)?;
        }
        let state = self
            .rpc
            .request(json!({"type":"get_state"}))
            .await
            .map_err(acp_internal)?;
        // get_state exposes live compaction but not the auto-retry policy.
        // The public SDK snapshot reports re-read effective configuration;
        // do not present that as a live AgentSession getter.
        let (snapshot, configuration_error) = match self.package_snapshot().await {
            Ok(snapshot) => (Some(snapshot), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let retry = snapshot
            .as_ref()
            .and_then(|value| value.get("retryConfiguredEnabled"))
            .and_then(Value::as_bool);
        let compaction = state.get("autoCompactionEnabled").and_then(Value::as_bool);
        let setting = |value: Option<bool>| match value {
            Some(true) => "on",
            Some(false) => "off",
            None => "unknown",
        };
        let message = format!(
            "Pi runtime · auto compaction: {} · retry policy (configured): {} · streaming: {} · compacting: {}",
            setting(compaction),
            setting(retry),
            state
                .get("isStreaming")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            state
                .get("isCompacting")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        );
        Ok(
            json!({"state":state,"retryConfiguredEnabled":retry,"configurationError":configuration_error,"message":message}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_controls_are_typed_pi_commands() {
        assert_eq!(runtime_command(&json!({})).unwrap(), None);
        assert_eq!(
            runtime_command(&json!({"operation":"retry","enabled":false})).unwrap(),
            Some(json!({"type":"set_auto_retry","enabled":false}))
        );
        assert_eq!(
            runtime_command(&json!({"operation":"compaction","enabled":true})).unwrap(),
            Some(json!({"type":"set_auto_compaction","enabled":true}))
        );
        assert_eq!(
            runtime_command(&json!({"operation":"cancel-retry"})).unwrap(),
            Some(json!({"type":"abort_retry"}))
        );
        for value in [
            json!({"operation":"retry","enabled":"false"}),
            json!({"operation":"compaction"}),
            json!({"operation":"remove"}),
        ] {
            assert!(runtime_command(&value).is_err());
        }
    }
}
