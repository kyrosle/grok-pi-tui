#![allow(
    unused_imports,
    unused_variables,
    unused_mut,
    unreachable_code,
    dead_code
)]
#![warn(unreachable_pub)]
#[cfg(all(test, feature = "dhat-heap"))]
#[global_allocator]
static DHAT_ALLOC: dhat::Alloc = dhat::Alloc;
#[cfg(feature = "stock-runtime")]
pub(crate) use xai_grok_telemetry::unified_log;
#[cfg(feature = "stock-runtime")]
pub use xai_tracing_macros::{teprintln, timed, tprintln};
#[cfg(feature = "stock-runtime")]
pub mod agent;
#[cfg(feature = "stock-runtime")]
pub mod auth {
    pub use crate::agent::init::run_cli_logout;
    pub use crate::credential_factory::{
        build_bootstrap_otel_credentials, build_storage_client_for_proxy,
    };
    pub use xai_grok_login::*;
}
#[cfg(feature = "stock-runtime")]
pub mod builtin;
#[cfg(feature = "stock-runtime")]
pub use xai_grok_bundle as bundle;
#[cfg(feature = "stock-runtime")]
pub mod claude_import;
#[cfg(feature = "stock-runtime")]
pub mod claude_import_state;
#[cfg(feature = "stock-runtime")]
pub mod cli_models;
#[cfg(feature = "stock-runtime")]
pub mod config;
#[cfg(all(test, feature = "config-docs"))]
#[cfg(feature = "stock-runtime")]
pub mod config_docs;
#[cfg(feature = "stock-runtime")]
pub mod credential_factory;
pub use xai_grok_shell_base::cpu_profile;
pub use xai_grok_shell_base::env;
#[cfg(feature = "stock-runtime")]
pub mod extensions;
#[cfg(feature = "stock-runtime")]
pub use xai_grok_foreign_sessions as foreign_sessions;
#[cfg(feature = "stock-runtime")]
pub mod heap_profile;
pub mod host_features;
#[cfg(feature = "stock-runtime")]
pub use xai_grok_http as http;
#[cfg(feature = "stock-runtime")]
pub mod inspect;
#[cfg(feature = "stock-runtime")]
pub mod instrumentation;
#[cfg(feature = "stock-runtime")]
pub mod leader;
#[cfg(feature = "stock-runtime")]
pub mod managed_config;
#[cfg(feature = "stock-runtime")]
pub mod mcp_doctor;
#[cfg(feature = "stock-runtime")]
pub use xai_grok_models as models;
#[cfg(feature = "stock-runtime")]
pub mod plugin;
#[cfg(feature = "stock-runtime")]
pub mod relay;
#[cfg(feature = "stock-runtime")]
pub mod remote;
#[cfg(feature = "stock-runtime")]
pub mod sampling;
#[cfg(feature = "stock-runtime")]
pub mod session;
#[cfg(feature = "stock-runtime")]
pub use xai_grok_shell_terminal as terminal;
#[cfg(all(test, feature = "stock-runtime"))]
pub(crate) mod test_support;
#[cfg(feature = "stock-runtime")]
pub mod tier;
#[cfg(feature = "stock-runtime")]
pub mod tools;
#[cfg(feature = "stock-runtime")]
pub mod upload;
#[cfg(feature = "stock-runtime")]
pub mod util;
#[doc(hidden)]
#[cfg(feature = "stock-runtime")]
pub mod waterfall;

/// Runtime-independent contracts for native hosts. All items are re-exports of their canonical implementations.
pub mod contracts;
#[cfg(not(feature = "stock-runtime"))]
pub use contracts::{config, sampling, util};
