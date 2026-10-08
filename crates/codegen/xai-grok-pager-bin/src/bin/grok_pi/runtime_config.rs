use super::{bash_extension, tools_extension::should_inject_tools_extension};

pub(super) fn durable_enabled(cli_on: bool, cli_off: bool, config: Option<&toml::Value>) -> anyhow::Result<bool> {
    if cli_on { return Ok(true); }
    if cli_off { return Ok(false); }
    match config.and_then(|root| root.get("ui")).and_then(|ui| ui.get("pi_durable")) {
        None => Ok(false),
        Some(toml::Value::Boolean(value)) => Ok(*value),
        Some(_) => anyhow::bail!("[ui].pi_durable must be true or false"),
    }
}

#[cfg(test)]
mod durable_tests {
    use super::durable_enabled;
    use clap::Parser;
    use crate::cli::Args;
    #[test]
    fn durable_preference_and_cli_overrides() {
        let on: toml::Value = toml::from_str("[ui]\npi_durable=true").unwrap();
        assert!(!durable_enabled(false, false, None).unwrap());
        assert!(durable_enabled(false, false, Some(&on)).unwrap());
        assert!(!durable_enabled(false, true, Some(&on)).unwrap());
        assert!(durable_enabled(true, false, None).unwrap());
        let invalid: toml::Value = toml::from_str("[ui]\npi_durable='yes'").unwrap();
        assert!(durable_enabled(false, false, Some(&invalid)).is_err());
        assert!(Args::try_parse_from(["grok-pi", "--durable", "--no-durable"]).is_err());
    }
}

/// Best-effort host terminal size for Remote TUI viewport (Pi child has no TTY).
pub(super) fn host_terminal_size() -> Option<(u16, u16)> {
    #[cfg(unix)]
    {
        // SAFETY: ioctl(TIOCGWINSZ) on stdout; fails cleanly when not a TTY.
        unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0
                && ws.ws_col > 0
                && ws.ws_row > 0
            {
                return Some((ws.ws_col, ws.ws_row));
            }
        }
    }
    None
}

/// Feature flags that default to ON. Explicit `0`/`false`/`off`/`no` disables.
/// Unset or any other value (including `1`) enables.
pub(super) fn env_flag_default_on(name: &str) -> bool {
    match std::env::var(name) {
        Err(_) => true,
        Ok(value) => {
            let v = value.trim();
            !(v.eq_ignore_ascii_case("0")
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
    }
}

/// `[ui].pi_bash` — switch for grok-pi's enhanced Bash bridge only.
/// An explicitly-set `PI_GROK_BASH`
/// environment variable remains a process-local override; otherwise F2/TOML is
/// authoritative. Missing/invalid config defaults on.
pub(super) fn bash_bridge_enabled() -> bool {
    if std::env::var_os("PI_GROK_BASH").is_some() {
        return env_flag_default_on("PI_GROK_BASH");
    }
    let config = xai_grok_config::load_effective_config_disk_only().ok();
    bash_bridge_enabled_from_config(config.as_ref())
}

pub(super) fn bash_bridge_enabled_from_config(config: Option<&toml::Value>) -> bool {
    config
        .and_then(|root| root.get("ui"))
        .and_then(|ui| ui.get("pi_bash"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(true)
}

const DEFAULT_BASH_MAX_WAIT_MINS: &str = "4.5";

pub(super) fn resolve_bash_max_wait_mins(cli: Option<f64>, inherited: Option<&str>) -> String {
    cli.map(|value| value.to_string())
        .or_else(|| inherited.map(str::to_owned))
        .unwrap_or_else(|| DEFAULT_BASH_MAX_WAIT_MINS.to_string())
}

/// Adapter background/kill RPC is valid only while enhanced Bash is active.
pub(super) fn bash_control_meta_for_adapter(
    bash_enabled: bool,
    extension: Option<&bash_extension::BashExtension>,
) -> Option<std::path::PathBuf> {
    extension
        .filter(|_| bash_enabled)
        .map(|extension| extension.control_meta_path().to_path_buf())
}

pub(super) fn normal_f2_tool_policy_applies(
    pi_args: &[String],
    bridge_extensions_enabled: bool,
) -> bool {
    bridge_extensions_enabled && should_inject_tools_extension(pi_args)
}
