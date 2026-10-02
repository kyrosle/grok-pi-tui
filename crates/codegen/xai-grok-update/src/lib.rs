#[cfg(feature = "stock-runtime")]
pub mod auto_update;
pub mod pi_update;
mod update_available;
pub mod version;
#[cfg(feature = "stock-runtime")]
mod version_policy;

#[cfg(feature = "stock-runtime")]
pub use auto_update::UpdateStatus;
pub use pi_update::{
    PiUpdateChannel, PiUpdateOptions, check_pi_update_background, fetch_pi_latest_version,
    install_pi_update, load_pi_update_channel, run_pi_update,
};
pub use update_available::UpdateAvailable;
pub use version::{UpdateConfig, channel_label, channel_name, write_version_cache};
#[cfg(feature = "stock-runtime")]
pub use version_policy::enforce_version_policy_or_exit;
