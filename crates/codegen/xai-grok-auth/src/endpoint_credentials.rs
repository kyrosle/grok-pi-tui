//! Endpoint-bound credential handles shared by their producer and HTTP clients.
use std::sync::Arc;

/// Embedding-client credentials scoped to a trusted endpoint.
/// Only [`Self::for_endpoint`] retains a live credential; the empty default fails closed.
#[derive(Clone, Default)]
pub struct EndpointScopedCredentials {
    endpoint: Option<reqwest::Url>,
    auth_credentials: Option<Arc<dyn crate::AuthCredentialProvider>>,
    api_key_provider: Option<xai_tool_types::auth::SharedApiKeyProvider>,
}

// Manual Debug that redacts the credential handles; only their presence shows.
impl std::fmt::Debug for EndpointScopedCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EndpointScopedCredentials")
            .field("endpoint", &self.endpoint)
            .field("has_auth_credentials", &self.auth_credentials.is_some())
            .field("has_api_key_provider", &self.api_key_provider.is_some())
            .finish()
    }
}

impl EndpointScopedCredentials {
    pub fn endpoint(&self) -> Option<&reqwest::Url> {
        self.endpoint.as_ref()
    }
    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.auth_credentials.is_none() && self.api_key_provider.is_none()
    }

    /// Retains the credentials only for a trusted, parsable `endpoint`; otherwise drops them.
    pub fn for_endpoint(
        endpoint: &str,
        is_trusted: impl FnOnce(&str) -> bool,
        auth_credentials: Option<Arc<dyn crate::AuthCredentialProvider>>,
        api_key_provider: Option<xai_tool_types::auth::SharedApiKeyProvider>,
    ) -> Self {
        if is_trusted(endpoint)
            && let Ok(url) = reqwest::Url::parse(endpoint)
        {
            return Self {
                endpoint: Some(url),
                auth_credentials,
                api_key_provider,
            };
        }
        if auth_credentials.is_some() || api_key_provider.is_some() {
            tracing::info!(
                target: "xai_memory",
                endpoint,
                "memory embeddings: session credentials withheld for non-first-party endpoint; its own key, if any, still applies"
            );
        }
        Self::none()
    }

    pub fn auth_credentials(&self) -> Option<&Arc<dyn crate::AuthCredentialProvider>> {
        self.auth_credentials.as_ref()
    }

    pub fn api_key_provider(&self) -> Option<&xai_tool_types::auth::SharedApiKeyProvider> {
        self.api_key_provider.as_ref()
    }

    pub fn approved_for(&self, base_url: &str) -> bool {
        match &self.endpoint {
            None => self.is_empty(),
            Some(endpoint) => reqwest::Url::parse(base_url).is_ok_and(|url| &url == endpoint),
        }
    }
}
