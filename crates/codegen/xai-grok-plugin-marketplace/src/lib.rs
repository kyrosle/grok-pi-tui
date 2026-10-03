//! Provides marketplace source configuration and plugin discovery, indexed with a filesystem fallback.
//! Install integration goes through the existing `InstallRegistry` pipeline.

pub mod catalog;
pub mod config;
pub mod error;
pub mod git;
pub mod index;
pub mod install_resolve;
pub mod installer;
pub mod matcher;
pub mod scanner;
pub mod types;

pub use config::{
    env_require_sha, foreign_settings_roots, load_extra_sources_from_settings,
    load_extra_sources_from_settings_in, load_require_sha, load_sources, native_settings_roots,
};
pub use error::MarketplaceError;
pub use scanner::scan_marketplace;
pub use types::*;

pub(crate) use xai_hooks_plugins_types::marketplace::canonical_github_owner_repo;
pub use xai_hooks_plugins_types::marketplace::{
    OFFICIAL_SOURCE_GIT_URL, OFFICIAL_SOURCE_NAME, is_official_source_url,
};
