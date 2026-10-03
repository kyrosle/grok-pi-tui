//! Concrete task presentation snapshots and kill outcomes shared by hosts and native UI.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartialOutput<'a> {
    text: &'a str,
    total_bytes: usize,
}

impl<'a> PartialOutput<'a> {
    pub fn whole(text: &'a str) -> Self {
        Self {
            text,
            total_bytes: text.len(),
        }
    }

    /// Part of an output of `total_bytes`.
    pub fn part_of(text: &'a str, total_bytes: usize) -> Self {
        Self {
            text,
            total_bytes: total_bytes.max(text.len()),
        }
    }

    pub fn text(&self) -> &'a str {
        self.text
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

/// Distinguishes different types of background tasks.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    /// Regular bash command.
    #[default]
    Bash,
    /// Monitor tool — streams stdout events with rate limiting.
    Monitor,
}

/// Full snapshot of a task's state.
/// Used by both local and ACP backends.
#[derive(
    Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct TaskSnapshot {
    pub task_id: String,
    /// The actual command that was executed (may be isolation-wrapped).
    pub command: String,
    /// The original user command before isolation wrapping. When set, model/user-facing output
    /// should prefer this over `command` to avoid exposing internal isolation mechanics
    /// (unshare/mount wrapper).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_command: Option<String>,
    pub cwd: String,
    pub start_time: std::time::SystemTime,
    pub end_time: Option<std::time::SystemTime>,
    pub output: String,
    pub output_file: PathBuf,
    /// `output` may not be the whole output: read `output_file` for the rest.
    /// Says the copy is partial, not how it came to be.
    pub truncated: bool,
    /// Total bytes the task has written, when the source tracks it. `output`
    /// may hold only part of that; zero means unknown.
    #[serde(default)]
    pub output_total_bytes: usize,
    pub exit_code: Option<i32>,
    pub signal: Option<String>,
    pub completed: bool,
    /// Task kind: bash (default) or monitor.
    #[serde(default)]
    pub kind: TaskKind,
    /// Whether a block-waiter (`block=true`) consumed this task's result.
    /// When set, the notification bridge skips auto-wake synthetic prompts
    /// because the blocking caller already received the result directly.
    #[serde(default)]
    pub block_waited: bool,
    /// Whether this task was explicitly killed (kill tool, UI Stop, or
    /// teardown) rather than exiting on its own. Display/tombstone flag;
    /// auto-wake uses [`Self::is_auto_wake_suppressed`].
    #[serde(default)]
    pub explicitly_killed: bool,
    /// Model already got a kill/wait tool result, or the kill was teardown.
    /// False for UI/Stop with no live waiter. Missing on the wire is false.
    #[serde(default)]
    pub kill_result_delivered: bool,

    /// Session that owns this task. Used for scoped kill operations so
    /// subagent teardown only kills the subagent's own tasks, not
    /// the parent's or sibling's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_session_id: Option<String>,
    /// Model-supplied label for task UI / snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// True after explicit/user/auto backgrounding; false for pure foreground runs.
    #[serde(default)]
    pub is_backgrounded: bool,
}

impl TaskSnapshot {
    pub fn is_completed_background(&self) -> bool {
        self.completed && self.is_backgrounded
    }

    /// Calculate duration in seconds.
    /// If task is still running, returns time since start.
    pub fn duration_secs(&self) -> f64 {
        let end = self.end_time.unwrap_or_else(std::time::SystemTime::now);
        end.duration_since(self.start_time)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    /// True iff the task has NOT yet completed — covers bash AND monitor task kinds (the `kind`
    /// field doesn't change this predicate; the runtime turn-end TodoGate counts both as backing
    /// work).
    pub fn is_outstanding(&self) -> bool {
        !self.completed
    }

    /// The output on hand, and the size of the output it came from. Readers
    /// take the size from here so the "not tracked" case is handled once.
    pub fn output_view(&self) -> PartialOutput<'_> {
        PartialOutput::part_of(&self.output, self.output_total_bytes)
    }

    /// Incomplete and backgrounded — tray/`tasks_snapshot` predicate (not FG in-flight).
    pub fn is_outstanding_background(&self) -> bool {
        !self.completed && self.is_backgrounded
    }

    /// True when a TaskCompleted auto-wake would be redundant.
    pub fn is_auto_wake_suppressed(&self) -> bool {
        self.block_waited || (self.explicitly_killed && self.kill_result_delivered)
    }
}

/// Result of killing a terminal task. Serialized over the wire in the `x.ai/task/kill` ext response
/// (`xai-grok-shell::extensions::task::KillTaskResponse`) and deserialized by clients
/// (xai-grok-pager), so it derives both serde directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillOutcome {
    Killed,
    AlreadyExited,
    NotFound,
}

/// Who initiated a background-task kill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillSource {
    /// Model `kill_task` / `kill_command_or_subagent` tool.
    ModelTool,
    /// Single-task client UI kill (task-pane `[×]`).
    ClientUi,
    /// Bulk teardown: owner sweep, dashboard stop-all, session delete, headless reap.
    Teardown,
}

impl KillSource {
    /// Model-tool and teardown kills count as delivered; a client/UI kill
    /// counts only when a waiter actually received the result.
    pub fn marks_result_delivered(self, waiter_delivered: bool) -> bool {
        matches!(self, Self::ModelTool | Self::Teardown) || waiter_delivered
    }
}

pub fn is_task_tool_id(name: &str) -> bool {
    matches!(
        name,
        crate::tool_names::TASK_TOOL_NAME | "Task" | "spawn_subagent"
    )
}
