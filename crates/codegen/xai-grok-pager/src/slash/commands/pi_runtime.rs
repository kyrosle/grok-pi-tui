//! Native controls for Pi-owned retry and compaction settings.
use crate::app::actions::{Action, PiControlMethod};
use crate::slash::command::{AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand};
use serde_json::{Value, json};

pub struct PiRuntimeCommand;

fn runtime_params(args: &str) -> Option<Value> {
    match args.split_whitespace().collect::<Vec<_>>().as_slice() {
        [] => Some(json!({"operation":"status"})),
        ["cancel-retry"] => Some(json!({"operation":"cancel-retry"})),
        [
            operation @ ("retry" | "compaction"),
            enabled @ ("on" | "off"),
        ] => Some(json!({"operation":operation,"enabled":*enabled == "on"})),
        _ => None,
    }
}

impl SlashCommand for PiRuntimeCommand {
    fn name(&self) -> &str {
        "pi-runtime"
    }
    fn description(&self) -> &str {
        "Inspect Pi runtime; configure automatic retry/compaction or cancel retry"
    }
    fn usage(&self) -> &str {
        "/pi-runtime [retry on|off | compaction on|off | cancel-retry]"
    }
    fn takes_args(&self) -> bool {
        true
    }
    fn suggest_args(&self, _ctx: &AppCtx, _args_query: &str) -> Option<Vec<ArgItem>> {
        Some(
            [
                ("retry on", "Enable automatic retry"),
                ("retry off", "Disable automatic retry"),
                ("compaction on", "Enable automatic compaction"),
                ("compaction off", "Disable automatic compaction"),
                ("cancel-retry", "Cancel the current retry delay"),
            ]
            .into_iter()
            .map(|(text, description)| ArgItem {
                display: text.into(),
                match_text: text.into(),
                insert_text: text.into(),
                description: description.into(),
            })
            .collect(),
        )
    }
    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        match runtime_params(args) {
            Some(params) => CommandResult::Action(Action::PiControlRequest {
                method: PiControlMethod::RuntimeControl,
                params,
            }),
            None => CommandResult::Error(format!("usage: {}", self.usage())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_arguments_allow_only_typed_fixed_operations() {
        assert_eq!(runtime_params(""), Some(json!({"operation":"status"})));
        assert_eq!(
            runtime_params("retry on"),
            Some(json!({"operation":"retry","enabled":true}))
        );
        assert_eq!(
            runtime_params("compaction off"),
            Some(json!({"operation":"compaction","enabled":false}))
        );
        assert_eq!(
            runtime_params("cancel-retry"),
            Some(json!({"operation":"cancel-retry"}))
        );
        for invalid in [
            "retry true",
            "compaction 1",
            "retry on extra",
            "arbitrary/method",
            "status ignored",
        ] {
            assert!(runtime_params(invalid).is_none());
        }
    }
}
