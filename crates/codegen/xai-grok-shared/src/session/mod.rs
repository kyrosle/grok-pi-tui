use std::path::PathBuf;

mod context;
mod details;
pub use details::{
    AssistantUsageMetric, CacheSessionMetrics, CacheUsageTotals, FeedbackOutcome, FeedbackResponse,
    SessionInfoData, SessionInfoResponse, SessionTokenTotals, SessionUsageStats,
    model_display_name,
};
pub mod info;
pub use context::{ContextInfo, TokenUsageCategory};
pub mod todo;

pub use info::Info;

// Re-export shared feedback wire types used by downstream crates (e.g. xai-grok-pager-render).
pub use prod_mc_cli_chat_proxy_types::feedback_types::FeedbackTerminalInfo;

pub fn session_dir(info: &Info) -> PathBuf {
    xai_grok_config::sessions_cwd_dir(&info.cwd).join(info.id.to_string())
}

/// Formats a count with a naively pluralized noun: `"1 skill"`, `"21 skills"`.
pub fn count_detail(count: u64, noun: &str) -> String {
    let suffix = if count == 1 { "" } else { "s" };
    format!("{count} {noun}{suffix}")
}

pub mod notification;
pub mod prompt_origin;
pub mod chunk_meta;

pub mod sampling_error;

pub mod catalog;
pub mod title;
pub mod result;
pub mod prompt_meta;
pub mod usage;

pub mod feedback;

pub mod billing;

pub mod completion;

pub mod compact;

pub mod user_echo;

pub mod share;
