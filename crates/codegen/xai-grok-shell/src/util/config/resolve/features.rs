pub use xai_grok_shared::config::resolve::features::*;
use crate::util::config::RemoteSettings;
use toml::Value as TomlValue;
pub(crate) fn turn_transient_retry_from_toml(v: Option<&TomlValue>) -> Option<bool> {
    v?.get("features")?.get("turn_transient_retry")?.as_bool()
}

/// Spawn-time kill switch: `[features] turn_transient_retry`, `GROK_TURN_TRANSIENT_RETRY`, remote key (below local), default on.
/// Loads local layers itself; call once per session.
pub(crate) fn resolve_turn_transient_retry(remote: Option<bool>) -> bool {
    let user_cfg = crate::config::load_effective_config().ok();
    // Merged (user, system, MDM last-wins) like the sibling resolvers, so an enterprise pin is not silently dropped
    let requirements = crate::config::load_merged_requirements();
    compose_turn_transient_retry(requirements.as_ref(), user_cfg.as_ref(), remote)
}

fn compose_turn_transient_retry(
    requirements: Option<&TomlValue>,
    user: Option<&TomlValue>,
    remote: Option<bool>,
) -> bool {
    use turn_transient_retry_from_toml as from_toml;
    crate::agent::config::resolve_turn_transient_retry(
        from_toml(requirements),
        /* cli          */ None,
        from_toml(user),
        /* managed: merged into the config layer by load_effective_config */ None,
        remote,
    )
    .value
}

/// Precedence: requirements > env > config.toml > remote > default (on).
pub(crate) fn resolve_repo_status_in_system_prompt(remote: Option<&RemoteSettings>) -> bool {
    use crate::agent::config::{Feature, FeatureSources};
    let user_cfg = crate::config::load_effective_config().ok();
    let requirements = crate::config::load_merged_requirements();
    let env = FeatureSources::from_process_env(Feature::RepoStatusInSystemPrompt).env;
    compose_repo_status_in_system_prompt(requirements.as_ref(), user_cfg.as_ref(), remote, env)
}

fn compose_repo_status_in_system_prompt(
    requirements: Option<&TomlValue>,
    user: Option<&TomlValue>,
    remote: Option<&RemoteSettings>,
    env: Option<bool>,
) -> bool {
    use crate::agent::config::{Feature, FeatureSources};
    let feature = Feature::RepoStatusInSystemPrompt;
    let from_toml = |v: Option<&TomlValue>| -> Option<bool> {
        v?.get("features")?.get(feature.key())?.as_bool()
    };
    feature
        .resolve(FeatureSources {
            pin: from_toml(requirements),
            env,
            config: from_toml(user),
            remote: feature.remote_value(remote),
        })
        .value
}
