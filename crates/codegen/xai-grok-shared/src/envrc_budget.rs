use std::time::Duration;
pub const ENVRC_LOAD_TIMEOUT: Duration = Duration::from_secs(10);
pub const ENVRC_TIMEOUT_ENV: &str = "GROK_ENVRC_TIMEOUT_SECS"; // seconds; 0 disables
pub const MAX_TIMEOUT: Duration = Duration::from_secs(3600);
/// Loader-side slack over the evaluator deadline (covers a wedged stat).
pub const JOIN_SLACK: Duration = Duration::from_secs(10);

pub fn effective_timeout() -> Duration {
    timeout_from(std::env::var(ENVRC_TIMEOUT_ENV).ok().as_deref())
}

/// Total budget callers should allow an in-flight load.
pub fn loader_budget() -> Duration {
    effective_timeout() + JOIN_SLACK
}

pub fn timeout_from(overriding: Option<&str>) -> Duration {
    let trimmed = overriding.map(str::trim).unwrap_or_default();
    if trimmed.is_empty() {
        return ENVRC_LOAD_TIMEOUT;
    }
    match trimmed.parse::<u64>() {
        Ok(secs) => {
            let capped = Duration::from_secs(secs).min(MAX_TIMEOUT);
            if capped.as_secs() < secs {
                tracing::warn!(secs, "clamping {ENVRC_TIMEOUT_ENV} to one hour");
            }
            capped
        }
        Err(_) => {
            tracing::warn!(value = trimmed, "ignoring unparseable {ENVRC_TIMEOUT_ENV}");
            ENVRC_LOAD_TIMEOUT
        }
    }
}
