pub use xai_grok_shared::config::settings_writes::*;
use anyhow::Result;

/// Stock model picks continue to dismiss active campaigns.
pub async fn set_default_model(value: String) -> Result<()> {
    super::campaigns::persist_models_default(
        if value.is_empty() { None } else { Some(value) },
        None,
    )
    .await
}


/// Stock follow-up reads retain the campaign-aware loader.
pub async fn follow_up_steer_enabled() -> bool {
    xai_grok_shared::config::settings_writes::follow_up_steer_enabled_with(crate::config::load_effective_config)
}
