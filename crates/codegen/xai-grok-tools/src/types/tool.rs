//! Tool types and post-execution reminders.
//!
//! The tool runtime contract (`Tool` trait) lives in `xai_tool_runtime`.
//! Tool metadata (kind, namespace, fingerprinting, etc.) lives in
//! `crate::types::tool_metadata::ToolMetadata`.
//!
//! This module provides:
//! - `ToolNamespace`, `ToolKind` — classification enums
//! - `Reminder` — post-execution system reminders (per-tool + cross-cutting)
use crate::types::output::ToolOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::SharedResources;
pub use xai_tool_types::classification::{ToolKind, ToolNamespace};

/// System reminders that fire after a tool call completes. **Per-tool reminders** on tool structs
/// (e.g., `ReadFileTool`: empty file, offset past end). **Cross-cutting reminders** on standalone
/// structs (e.g., `SkillDiscoveryReminder`) that react to any tool call.
#[async_trait::async_trait]
pub trait Reminder {
    /// Requirements for this reminder to be active.
    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
    /// Collect reminders after a tool execution completes.
    async fn collect_reminders(
        &self,
        _resources: SharedResources,
        _tool_output: &ToolOutput,
    ) -> Vec<String> {
        vec![]
    }
}
