#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unreachable_code,
    dead_code
)]
//! xai-grok-pager: Grok Build TUI.
//!
//! A clean-room implementation built on the v3 pager rendering engine.
pub mod acp;
pub mod actions;
pub mod app;
pub mod best_effort_stderr;
pub mod client_identity;
pub mod completions_cmd;
mod config_toml_edit;
pub mod diagnostics;
pub mod disk_usage_cmd;
pub mod docs;
#[cfg(feature = "stock-runtime")]
pub mod doctor_cmd;
pub mod export_cmd;
pub(crate) mod fs_size;
pub mod git_info;
#[cfg(feature = "stock-runtime")]
pub mod headless;
pub mod hyperlink_route;
pub mod inline_media_ffmpeg;
pub mod input_log;
#[cfg(feature = "stock-runtime")]
pub mod mcp_cmd;
#[cfg(feature = "stock-runtime")]
pub mod memory_cmd;
pub mod memory_release;
pub mod memory_trace;
#[path = "minimal/api.rs"]
pub mod minimal_api;
#[path = "minimal/hook.rs"]
pub mod minimal_hook;
#[path = "minimal/reprint.rs"]
pub mod minimal_reprint;
#[cfg(feature = "stock-runtime")]
pub mod models;
pub mod native_feature_conflicts;
pub mod notifications;
#[allow(unused_imports, unused_macros)]
pub mod obf;
pub mod pi_model_config;
pub mod pi_resource_config;
pub mod pi_resource_policy;
#[cfg(feature = "stock-runtime")]
pub mod plugin_cmd;
pub mod pty_wrap;
pub mod recent_dirs;
pub mod scrollback;
#[cfg(feature = "stock-runtime")]
pub mod sessions_cmd;
pub mod settings;
#[cfg(feature = "stock-runtime")]
pub mod share_cmd;
pub mod slash;
pub mod startup;
pub mod tips;
pub mod tool_usage;
pub mod tutorial_docs;
pub mod usage_cmd;
pub mod wrap_clipboard_image;
pub mod wrap_cmd;
pub(crate) mod wrap_filter;
pub(crate) mod wrap_restore;
pub use xai_grok_gboom as gboom;
pub use xai_grok_pager_render::key;
pub use xai_grok_pager_render::{
    appearance, clipboard, glyphs, host, input, link_opener, modal_window_state, prompt_images,
    render, search, syntax, terminal, theme, util,
};
#[cfg(test)]
pub mod test_util;
#[cfg(feature = "stock-runtime")]
pub mod trace_cmd;
pub mod tracing;
pub mod unified_log;
pub mod views;
pub mod voice;
#[cfg(feature = "stock-runtime")]
pub mod worktree_cmd;

#[cfg(not(feature = "stock-runtime"))]
pub(crate) use xai_grok_config::load_effective_config_disk_only as load_effective_config;
#[cfg(feature = "stock-runtime")]
pub(crate) use xai_grok_shell::config::load_effective_config;

// Stock wrappers retain campaign/remote behavior; Pi reads and writes its real product disk settings.
#[cfg(not(feature = "stock-runtime"))]
pub(crate) use xai_grok_shared::config as settings_config;
#[cfg(feature = "stock-runtime")]
pub(crate) use xai_grok_shell::util::config as settings_config;
