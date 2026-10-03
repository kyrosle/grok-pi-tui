//! Product-scoped TUI settings: complete section snapshot, real disk loading, and locked atomic writes.
pub mod announcements;
pub mod consent;
pub mod hints;
pub mod load;
pub mod permissions;
pub mod persist;
pub mod resolve;
pub mod sections;
pub mod settings_writes;
pub mod tips;
pub use announcements::*;
pub use consent::*;
pub use hints::*;
pub use load::*;
pub use permissions::*;
pub use persist::*;
pub use resolve::*;
pub use sections::*;
pub use settings_writes::*;
pub use tips::*;
pub use xai_grok_config_types::*;

pub fn user_config_path() -> std::path::PathBuf {
    xai_grok_config::grok_home().join(xai_grok_config::USER_CONFIG_FILENAME)
}
pub fn project_config_path(root: &std::path::Path) -> std::path::PathBuf {
    xai_grok_config::project_config_dir(root).join(xai_grok_config::USER_CONFIG_FILENAME)
}

pub mod local_preferences;
pub use local_preferences::*;
// The settings facade exports the layer-reading resolvers, while ConfigTypes
// keeps the scalar tier cores under its own direct namespace.
pub use resolve::features::resolve_turn_transient_retry;
pub use resolve::mcp::{
    resolve_mcp_auto_restart, resolve_mcp_liveness_watchers, resolve_mcp_push_server_status,
    resolve_mcp_recursive_config_watch,
};
pub use xai_grok_config::DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT;

pub mod worktree;
pub use worktree::*;

pub mod plugin_cta;
pub use plugin_cta::*;
