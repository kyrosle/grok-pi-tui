//! Native host contracts without the stock Grok agent runtime.
//! This module exposes only canonical types and working helpers from existing lower layers.

pub use xai_grok_config as config;
pub use xai_grok_config_types as config_types;
pub use xai_grok_sampling_types as sampling;
pub use xai_grok_shared::{host_features, ui_config};
pub use xai_grok_shell_base::{cpu_profile, env, util};
pub use xai_tool_types as tool_types;
pub use xai_workflow as workflow;
