//! Canonical ACP task kill/cancel wire contracts.

use crate::task_snapshot::{KillOutcome, KillSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTaskRequest {
    pub session_id: String,
    pub task_id: String,
    /// Single-task UI `[×]` omits this (defaults to [`TaskKillSource::ClientUi`]).
    /// Bulk teardown (dashboard stop-all, session delete, headless reap) must send [`TaskKillSource::Teardown`].
    #[serde(default)]
    pub source: TaskKillSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskKillSource {
    #[default]
    ClientUi,
    Teardown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTaskResponse {
    pub task_id: String,
    pub outcome: KillOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSubagentRequest {
    pub subagent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubagentCancelOutcomeDto {
    /// A live subagent was cancelled; a real `SubagentFinished` is coming.
    Cancelled,
    /// The subagent already finished, so no finish event is coming; `status` is the real terminal status.
    AlreadyFinished { status: String },
    /// The id is unknown (never existed, or evicted), so no finish event is coming.
    NotFound,
    /// Unknown future `kind` (`#[serde(other)]`): lets an old client still parse and fall back to the legacy bool.
    /// `From` never produces this variant.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSubagentResponse {
    pub subagent_id: String,
    /// Legacy wire-compat flag for older pagers; new clients prefer `outcome`.
    pub cancelled: bool,
    /// Typed outcome; `None` only from an older shell. This shell always sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<SubagentCancelOutcomeDto>,
}

impl From<TaskKillSource> for KillSource {
    fn from(source: TaskKillSource) -> Self {
        match source {
            TaskKillSource::ClientUi => Self::ClientUi,
            TaskKillSource::Teardown => Self::Teardown,
        }
    }
}

impl SubagentCancelOutcomeDto {
    /// Legacy bool for older pagers: true only when a live subagent was stopped.
    /// Already-finished and not-found map to false so an old pager finalizes the row.
    pub fn cancelled_bool(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}
