//! Workflow result data shared by independent child-session producers.
use std::sync::Arc;

/// Result shape expected by `xai_workflow` host completion mapping.
#[derive(Debug, Clone)]
pub struct WorkflowAgentSpawnResult {
    pub success: bool,
    pub output: Arc<str>,
    pub error: Option<String>,
    pub cancelled: bool,
    pub child_session_id: String,
    pub total_tokens_used: u64,
    pub duration_ms: u64,
    pub backgrounded: bool,
}

/// Control the workflow tool applies to a run the calling session owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowControl {
    Pause,
    Stop,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowPhaseInfo {
    pub title: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowAgentInfo {
    pub agent_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub state: String,
    #[serde(default)]
    pub tokens_used: u64,
    #[serde(default)]
    pub duration_ms: u64,
}

/// Native workflow update wire payload. Field names and omissions match stock ACP.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "workflow_updated", tag = "sessionUpdate")]
pub struct WorkflowUpdate {
    pub run_id: String,
    #[serde(default)]
    pub revision: u64,
    pub name: String,
    pub objective: String,
    pub status: String,
    #[serde(default)]
    pub foreground: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<WorkflowPhaseInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_budget: Option<u64>,
    #[serde(default)]
    pub agents_used: u64,
    #[serde(default)]
    pub agents_reserved: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents_remaining: Option<u64>,
    #[serde(default)]
    pub agent_usage_incomplete: bool,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub active_agents: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_agent_label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<WorkflowAgentInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
}
