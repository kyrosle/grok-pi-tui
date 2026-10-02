//! Types are canonical in `xai-grok-tools`.
//! This module adds conversions between ACP plan entries and `TodoItem` since `xai-grok-tools` is protocol-agnostic.

pub use xai_grok_tools::implementations::grok_build::todo::TodoId;
pub use xai_grok_tools::implementations::grok_build::todo::TodoItem;
pub use xai_grok_tools::implementations::grok_build::todo::TodoPriority;
pub use xai_grok_tools::implementations::grok_build::todo::TodoState;
pub use xai_grok_tools::implementations::grok_build::todo::TodoStatus;

use agent_client_protocol as acp;

pub use xai_grok_shared::session::todo::todo_item_from_plan_entry;

/// Cancelled items become `Completed` with `{"cancelled": true}` in meta.
pub(crate) fn plan_entry_from_todo_item(item: TodoItem) -> acp::PlanEntry {
    let status = match item.status {
        TodoStatus::Pending => acp::PlanEntryStatus::Pending,
        TodoStatus::InProgress => acp::PlanEntryStatus::InProgress,
        TodoStatus::Completed => acp::PlanEntryStatus::Completed,
        TodoStatus::Cancelled => acp::PlanEntryStatus::Completed,
    };
    let mut meta = item.meta;
    if item.status == TodoStatus::Cancelled {
        let mut m = meta.unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = m.as_object_mut() {
            obj.insert("cancelled".into(), true.into());
        }
        meta = Some(m);
    }
    acp::PlanEntry::new(
        item.content,
        match item.priority {
            TodoPriority::High => acp::PlanEntryPriority::High,
            TodoPriority::Medium => acp::PlanEntryPriority::Medium,
            TodoPriority::Low => acp::PlanEntryPriority::Low,
        },
        status,
    )
    .meta(meta.and_then(|v| v.as_object().cloned()))
}
