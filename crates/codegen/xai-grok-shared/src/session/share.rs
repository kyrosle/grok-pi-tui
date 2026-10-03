//! Canonical native session sharing request/response wire data.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShareSessionRequest {
    pub session_id: String,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShareSessionResponse {
    pub share_url: String,
}
