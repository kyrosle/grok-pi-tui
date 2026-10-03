//! Scheduled-task wire reason and the original recurring expiry default.

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    schemars::JsonSchema,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ScheduledTaskRemovedReason {
    /// A one-shot task fired and completed.
    Completed,
    /// A recurring task reached `expires_at` (7 days after creation).
    Expired,
    /// Explicit `scheduler_delete` by the user or model.
    Deleted,
    /// Actor shutdown chip-cleanup. The task itself persists on disk and re-arms on session
    /// resume, so this must not read as a real removal.
    Shutdown,
    /// Absent on legacy payloads, or a variant this build doesn't know. Consumers must treat
    /// it as "no special handling".
    #[default]
    #[serde(other)]
    Unknown,
}

pub const RECURRING_TASK_TTL_DAYS: i64 = 7;
