pub mod acp_types;
pub mod announcement_state;
pub(crate) mod auto_mode;
pub mod commands;
pub(crate) mod compaction_config;
pub(crate) mod doom_loop_telemetry;
pub(crate) mod fork_status;
pub mod handle;
pub(crate) mod memory_state;
pub mod merge;
pub(crate) mod message_delivery;
pub mod notifications;
pub mod pending_interaction;
pub mod prompt_queue;
pub(crate) mod resume_status;
pub mod two_pass;
pub mod user_echo;
pub mod visibility;
pub use self::acp_session::*;
pub use self::acp_types::*;
pub use self::commands::*;
pub use self::fork::{ForkSessionRequest, ForkSessionResponse, fork_session};
pub use self::handle::*;
pub use self::persistence::{
    LocalFeedbackEntry, UserFeedbackEntry, find_local_child_for_remote, resolve_local_session,
    resolve_local_session_any_cwd, resolve_local_session_ids_any_cwd, session_exists_for_cwd,
};
pub use self::result::{Empty, ExtMethodResult};
pub use self::share::{ShareSessionRequest, ShareSessionResponse};
pub use self::user_echo::{CLIENT_USER_MESSAGE_ECHO_META, USER_MESSAGE_ECHO_CAPABILITY};
pub use prod_mc_cli_chat_proxy_types::feedback_types::{
    ClientType, FeedbackImage, FeedbackTerminalInfo, MAX_FEEDBACK_IMAGE_BYTES,
    MAX_FEEDBACK_IMAGE_TOTAL_BYTES, MAX_FEEDBACK_IMAGES, RatingType, feedback_image_extension,
    validate_feedback_images,
};
pub use xai_fsnotify::{FsConfig, FsEvent, FsEventKind, FsEventSource, FsNotifyError, GitMetaKind};
/// `false` twin: this template is not compiled into this build, so no template matches.
/// Keeps ungated call sites compiling in both configurations.
pub(crate) fn is_cursor_user_template(
    _template: &xai_grok_agent::prompt::user_message::UserMessageTemplate,
) -> bool {
    false
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CompactionPins {
    pub mode: xai_chat_state::CompactionMode,
    pub two_pass: bool,
}
pub(crate) fn cursor_compaction_pins(
    resolved_mode: xai_chat_state::CompactionMode,
    resolved_two_pass: bool,
    is_cursor: bool,
) -> CompactionPins {
    if is_cursor {
        CompactionPins {
            mode: xai_chat_state::CompactionMode::Summary,
            two_pass: false,
        }
    } else {
        CompactionPins {
            mode: resolved_mode,
            two_pass: resolved_two_pass,
        }
    }
}
/// `false` twin of [`is_cursor_system_template`]; see [`is_cursor_user_template`].
pub(crate) fn is_cursor_system_template(
    _template: &xai_grok_agent::prompt::context::TemplateOverride,
) -> bool {
    false
}
/// The single spelling of "only Image blocks are carried structurally" (interject parse and queue-interject harvest).
pub(crate) fn image_blocks(
    blocks: impl IntoIterator<Item = agent_client_protocol::ContentBlock>,
) -> Vec<agent_client_protocol::ImageContent> {
    blocks
        .into_iter()
        .filter_map(|block| match block {
            agent_client_protocol::ContentBlock::Image(img) => Some(img),
            _ => None,
        })
        .collect()
}
pub use xai_agent_lifecycle::{
    AnalyticsClass, CompactionClass, InputAuthority, InputPolicy, QueuePolicy, ShutdownPolicy,
    SlashAuthority, TurnBoundary,
};
pub use xai_grok_shared::session::prompt_origin::PromptOrigin;

/// Determines whether the session sends an initial file index to the client or just streams raw file events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub(crate) enum ClientFsMode {
    #[default]
    Events,
    Index,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct ClientFsConfig {
    pub fs: FsConfig,
    pub mode: ClientFsMode,
}
pub use xai_grok_shared::session::share;
/// Proxy config for the session registry client.
/// Shared between `acp_session` (slash commands) and `persistence` (title generation).
#[derive(Clone)]
pub(crate) struct RegistryConfig {
    pub base_url: String,
    pub user_token: String,
    pub deployment_key: Option<String>,
    pub alpha_test_key: Option<String>,
}
pub mod acp_conversion;
pub(crate) mod acp_mcp;
pub(crate) mod acp_session;
pub(crate) mod agent_rebuild;
pub(crate) mod chat_persistence;
pub(crate) mod events;
pub mod export;
pub mod feedback;
pub mod feedback_manager;
pub mod file_system;
pub mod fork;
pub(crate) mod fs_watch;
pub(crate) mod goal_classifier;
pub(crate) mod goal_evaluator;
pub(crate) mod goal_next_step;
pub(crate) mod goal_orchestrator;
pub(crate) mod goal_planner;
pub(crate) mod goal_role_tools;
pub(crate) mod goal_stop_detector;
pub(crate) mod goal_strategist;
pub(crate) mod goal_summarizer;
pub mod goal_tracker;
pub mod helpers;
pub(crate) mod image_describe;
pub(crate) mod image_normalize;
pub(crate) mod inference_metrics;
pub use xai_grok_shared::session::info;
pub mod managed_mcp;
pub(crate) mod mcp_descriptors;
pub(crate) mod mcp_dispatcher;
#[cfg(test)]
mod mcp_dispatcher_e2e_tests;
pub(crate) mod mcp_elicitation;
pub(crate) mod mcp_restart;
pub mod mcp_servers;
pub mod memory;
pub(crate) mod memory_observation;
pub(crate) mod normalize_cache;
pub mod persistence;
pub(crate) mod session_create_prefetch;
pub use xai_grok_shared::placeholder_images;
pub mod plan_mode;
pub mod prompt_history;
pub mod prompt_parser;
pub(crate) mod prompt_timing;
pub(crate) mod replay_events;
pub mod repo_changes;
#[path = "restore_stub.rs"]
pub mod restore;
pub mod result;
pub mod signals;
pub(crate) mod slash_authority;
pub(crate) mod slash_commands;
pub mod usage_file;
pub use slash_commands::PAGER_COMMAND_KEYS;
pub(crate) mod repo_status_prefix;
pub mod storage;
pub(crate) mod streaming_capture;
pub(crate) mod summary;
pub(crate) mod telemetry;
#[cfg(feature = "test-support")]
pub mod testkit;
pub mod tool_index;
pub mod turn_completion;
pub mod unified_list;
pub(crate) mod user_message;
pub(crate) mod wire_tags;
/// Workflow engine host (Rhai + journal). Public for grok-pi External runtime seams.
pub mod workflow;
pub mod worktree;
pub(crate) mod worktree_cleanup;
pub mod worktree_pool;
