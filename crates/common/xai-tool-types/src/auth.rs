//! Credential and redacted-error interfaces; no tool implementation dependency.
use std::{future::Future, pin::Pin, sync::Arc};

/// Resolves the current API key for tool HTTP requests.
pub trait ApiKeyProvider: Send + Sync + 'static {
    /// Sync cached read (no refresh). Override point for static providers.
    fn current_api_key(&self) -> Option<String>;

    /// Per-request resolve. `AuthManager` overrides this to drive the
    /// refresh chain; default delegates to the sync method.
    fn current_api_key_async(&self) -> Pin<Box<dyn Future<Output = Option<String>> + Send + '_>> {
        Box::pin(std::future::ready(self.current_api_key()))
    }
}

/// Shared provider used across tool clients.
pub type SharedApiKeyProvider = Arc<dyn ApiKeyProvider>;

/// Which tool endpoint produced the 401.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::AsRefStr, strum::IntoStaticStr)]
pub enum ToolConsumer {
    #[strum(serialize = "ImageGen")]
    ImageGen,
    #[strum(serialize = "VideoGen.start")]
    VideoGenStart,
    #[strum(serialize = "VideoGen.poll")]
    VideoGenPoll,
    #[strum(serialize = "WebSearch")]
    WebSearch,
}
/// 401 attribution callback. Shell wires this to emit telemetry.
pub trait Auth401AttributionCallback: Send + Sync + std::fmt::Debug {
    /// `sent_bearer_suffix` is truncated to the fixed suffix length
    /// before crossing this boundary. `None` = no bearer was sent.
    fn record_401(&self, consumer: ToolConsumer, sent_bearer_suffix: Option<&str>);
}

/// Shared, cheap-to-clone alias for the attribution callback.
pub type SharedAttributionCallback = Arc<dyn Auth401AttributionCallback>;
