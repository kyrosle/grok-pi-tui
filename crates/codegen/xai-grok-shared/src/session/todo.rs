//! Canonical conversion of ACP plan rows into native Todo data.

use agent_client_protocol as acp;
use xai_tool_types::todo::{TodoItem, TodoPriority, TodoStatus};

/// ACP has no `Cancelled` status, so cancelled items are stored as `Completed` with `{"cancelled": true}` in meta.
pub fn todo_item_from_plan_entry(entry: acp::PlanEntry) -> TodoItem {
    let status = match entry.status {
        acp::PlanEntryStatus::Pending => TodoStatus::Pending,
        acp::PlanEntryStatus::InProgress => TodoStatus::InProgress,
        acp::PlanEntryStatus::Completed => {
            if entry
                .meta
                .as_ref()
                .and_then(|m| m.get("cancelled"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                TodoStatus::Cancelled
            } else {
                TodoStatus::Completed
            }
        }
        // TODO(acp-0.10): `PlanEntryStatus` is #[non_exhaustive].
        _ => TodoStatus::Pending,
    };
    TodoItem {
        content: entry.content,
        priority: match entry.priority {
            acp::PlanEntryPriority::High => TodoPriority::High,
            acp::PlanEntryPriority::Medium => TodoPriority::Medium,
            acp::PlanEntryPriority::Low => TodoPriority::Low,
            // TODO(acp-0.10): `PlanEntryPriority` is #[non_exhaustive].
            _ => TodoPriority::Medium,
        },
        status,
        meta: entry.meta.map(serde_json::Value::Object),
    }
}
