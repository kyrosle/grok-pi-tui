use std::path::{Path, PathBuf};
use tracing::warn;
/// Which requirements key activated the always-approve hard lock
/// ([`yolo_disabled_by_policy`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum YoloPinReason {
    /// `[ui] disable_bypass_permissions_mode = true` in requirements.toml.
    DisableBypassPermissionsMode,
    /// Back-compat: the legacy `[ui] yolo = false` requirements key still locks.
    LegacyYoloFalse,
}

impl YoloPinReason {
    /// Admin-facing pin message, shown in launch warnings and skip reasons.
    pub const fn message(self) -> &'static str {
        match self {
            Self::DisableBypassPermissionsMode => {
                "always-approve disabled by managed policy ([ui] disable_bypass_permissions_mode = true in requirements.toml)"
            }
            Self::LegacyYoloFalse => {
                "always-approve disabled by managed policy ([ui] yolo = false in requirements.toml)"
            }
        }
    }
}

/// The active always-approve hard lock: the pin reason plus the label of the requirements layer that set it.
/// The label is a file path, or the diskless macOS MDM source id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoloPolicyLock {
    pub source_label: String,
    pub reason: YoloPinReason,
}

/// Hard-lock predicate. `Some(reason)` iff a requirements layer sets `[ui] disable_bypass_permissions_mode` (or legacy `[ui] yolo = false`).
/// Vendor `disableBypassPermissionsMode` is not consulted, so grok does not inherit a host-wide lockdown; use root-owned `requirements.toml`. Fails open on user-writable layers.
pub fn yolo_disabled_by_policy() -> Option<&'static str> {
    yolo_policy_lock().map(|lock| lock.reason.message())
}

/// Layer-attributed form of [`yolo_disabled_by_policy`] so provenance displays can name the pinning layer.
pub fn yolo_policy_lock() -> Option<YoloPolicyLock> {
    let layers = xai_grok_config::requirements_layers();
    // Owned so the label outlives the borrowed layer temporaries below.
    let labeled: Vec<(PathBuf, &toml::Value)> = layers
        .iter()
        .map(|l| (PathBuf::from(l.source.label().as_ref()), &l.value))
        .collect();
    resolve_yolo_policy_block(labeled.iter().map(|(p, v)| (p.as_path(), *v)))
}

/// Read `[ui] <key>` as a bool; a non-bool value warns (naming key and layer) rather than silently failing to lock.
pub fn requirements_lock_bool(ui: Option<&toml::Value>, key: &str, path: &Path) -> Option<bool> {
    let value = ui?.get(key)?;
    match value.as_bool() {
        Some(b) => Some(b),
        None => {
            warn!(
                path = %path.display(),
                key,
                "[ui] {key} must be a boolean; ignoring non-bool value \
                 (always-approve lock not applied from this key in this layer)"
            );
            None
        }
    }
}

/// Pure form of [`yolo_policy_lock`] over pre-loaded layers; `path` labels the lock's provenance and non-bool warnings.
pub fn resolve_yolo_policy_block<'a>(
    requirement_layers: impl Iterator<Item = (&'a Path, &'a toml::Value)>,
) -> Option<YoloPolicyLock> {
    let lock = |path: &Path, reason| {
        Some(YoloPolicyLock {
            source_label: path.display().to_string(),
            reason,
        })
    };
    for (path, layer) in requirement_layers {
        let ui = layer.get("ui");
        // Native lock key (default false). `true` pins always-approve off.
        if requirements_lock_bool(ui, "disable_bypass_permissions_mode", path) == Some(true) {
            return lock(path, YoloPinReason::DisableBypassPermissionsMode);
        }
        // Back-compat alias: `[ui] yolo = false` in requirements.toml still pins (pre-rename configs)
        // A config.toml `yolo` is unaffected (not read here)
        if requirements_lock_bool(ui, "yolo", path) == Some(false) {
            return lock(path, YoloPinReason::LegacyYoloFalse);
        }
    }
    None
}
