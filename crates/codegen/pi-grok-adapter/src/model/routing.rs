use super::{PiModel, string};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Actual request identity from Pi's assistant message, independent of selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiDispatchedModel {
    pub provider: String,
    pub model: String,
    pub thinking_level: Option<String>,
}

impl PiDispatchedModel {
    pub fn from_message(message: &Value) -> Option<Self> {
        if string(message, &["role"]) != Some("assistant") {
            return None;
        }
        Some(Self {
            provider: string(message, &["provider"])?.to_string(),
            model: string(message, &["model"])?.to_string(),
            thinking_level: string(message, &["thinkingLevel"]).map(str::to_string),
        })
    }
}

pub fn model_route_status(
    selected: Option<&PiModel>,
    selected_level: &str,
    dispatched: Option<&PiDispatchedModel>,
) -> Option<String> {
    let selected = selected?;
    let dispatched = dispatched?;
    if selected.provider == dispatched.provider
        && selected.id == dispatched.model
        && dispatched
            .thinking_level
            .as_deref()
            .is_none_or(|level| level == selected_level)
    {
        return None;
    }
    let physical_level = dispatched
        .thinking_level
        .as_deref()
        .map(|level| format!(" · {level}"))
        .unwrap_or_default();
    Some(format!(
        "{}/{} · {} → {}/{}{}",
        selected.provider,
        selected.id,
        selected_level,
        dispatched.provider,
        dispatched.model,
        physical_level,
    ))
}

/// Match Pi getSessionStats' whole-file accounting without inventing attribution.
/// Codemode tool-result and summary usage may lack model identity in public RPC.
pub fn session_model_usage(payload: &Value) -> Value {
    let mut totals: BTreeMap<(Option<String>, Option<String>), (Value, f64)> = BTreeMap::new();
    for entry in payload
        .get("entries")
        .unwrap_or(payload)
        .as_array()
        .into_iter()
        .flatten()
    {
        let source = match string(entry, &["type"]) {
            Some("message") => {
                let Some(message) = entry.get("message") else {
                    continue;
                };
                if !matches!(string(message, &["role"]), Some("assistant" | "toolResult")) {
                    continue;
                }
                message
            }
            Some("usage" | "compaction" | "branch_summary") => entry,
            _ => continue,
        };
        let Some(usage) = source.get("usage") else {
            continue;
        };
        let key = (
            string(source, &["provider"]).map(str::to_string),
            string(source, &["model"]).map(str::to_string),
        );
        let (tokens, cost) = totals.entry(key).or_insert_with(|| {
            (
                json!({"input":0, "output":0, "cacheRead":0, "cacheWrite":0, "total":0}),
                0.0,
            )
        });
        for field in ["input", "output", "cacheRead", "cacheWrite"] {
            let count = usage.get(field).and_then(Value::as_u64).unwrap_or(0);
            tokens[field] = json!(tokens[field].as_u64().unwrap_or(0).saturating_add(count));
        }
        *cost += usage
            .get("cost")
            .and_then(|value| value.get("total"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0);
    }
    Value::Array(
        totals
            .into_iter()
            .map(|((provider, model), (mut tokens, cost))| {
                tokens["total"] = json!(
                    ["input", "output", "cacheRead", "cacheWrite"]
                        .into_iter()
                        .map(|field| tokens[field].as_u64().unwrap_or(0))
                        .fold(0u64, u64::saturating_add)
                );
                json!({"provider":provider, "model":model, "tokens":tokens, "cost":cost})
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PiEntryReplayCache, parse_models};

    #[test]
    fn route_keeps_selected_and_physical_thinking_separate_on_active_branch() {
        let selected = PiModel {
            provider: "router".into(),
            id: "auto".into(),
            ..Default::default()
        };
        let mut cache = PiEntryReplayCache::default();
        cache.reset("s", &json!({"leafId":"a", "entries":[
            {"id":"a", "parentId":null, "type":"message", "message":{"role":"assistant", "provider":"local", "model":"small", "thinkingLevel":"medium"}},
            {"id":"sibling", "parentId":null, "type":"message", "message":{"role":"assistant", "provider":"local", "model":"large", "thinkingLevel":"high"}}
        ]}));
        let routed = cache.latest_dispatched_model().unwrap();
        assert_eq!(routed.model, "small");
        assert_eq!(
            model_route_status(Some(&selected), "high", Some(&routed)).as_deref(),
            Some("router/auto · high → local/small · medium")
        );
        cache.append(&json!({"leafId":null,"entries":[]}));
        assert!(cache.latest_dispatched_model().is_none());
    }

    #[test]
    fn usage_preserves_actual_models_and_unattributed_totals_without_double_counting() {
        let usage =
            json!({"input":2,"output":3,"cacheRead":4,"cacheWrite":1,"cost":{"total":0.25}});
        let rows = session_model_usage(&json!({"entries":[
            {"type":"message","message":{"role":"assistant","provider":"p","model":"physical","usage":usage}},
            {"type":"usage","provider":"p","model":"physical","usage":usage},
            {"type":"message","message":{"role":"toolResult","usage":usage}},
            {"type":"compaction","usage":usage},
            {"type":"message","message":{"role":"user","usage":usage}}
        ]}));
        assert_eq!(rows.as_array().unwrap().len(), 2);
        assert_eq!(rows[0]["model"], Value::Null);
        assert_eq!(rows[0]["cost"], 0.5);
        assert_eq!(rows[1]["model"], "physical");
        assert_eq!(rows[1]["tokens"]["total"], 20);
        assert_eq!(rows[1]["cost"], 0.5);
    }

    #[test]
    fn non_chat_models_never_become_chat_picker_entries() {
        assert!(
            parse_models(&json!({"models":[
                {"type":"image","provider":"p","id":"painter","name":"Painter"},
                {"type":"classifier","provider":"p","id":"classify","name":"Classifier"}
            ]}))
            .is_empty()
        );
    }
}
