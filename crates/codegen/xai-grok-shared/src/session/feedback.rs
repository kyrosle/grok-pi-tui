//! Canonical feedback wire requests and existing pure submission construction.

pub use prod_mc_cli_chat_proxy_types::feedback_types::{
    ClientType, FeedbackImage, FeedbackTerminalInfo, MAX_FEEDBACK_IMAGE_BYTES,
    MAX_FEEDBACK_IMAGE_TOTAL_BYTES, MAX_FEEDBACK_IMAGES, RatingType, feedback_image_extension,
    validate_feedback_images,
};
use prod_mc_cli_chat_proxy_types::feedback_types::{FeedbackContent, FeedbackSubmission};

// ── Feedback ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackRequest {
    pub session_id: String,
    #[serde(default)]
    pub turn_number: Option<u64>,
    pub feedback_text: String,
}

/// Request to dismiss a feedback request (sent to the feedback backend).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct FeedbackRequestDismiss {
    pub session_id: String,
    pub request_id: String,
}

pub use super::{FeedbackOutcome, FeedbackResponse};

/// `turn_number` is optional from the client side.
/// Per-turn UIs (e.g. the thumbs button on a specific assistant message in the desktop chat history) may attach it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClientFeedbackInput {
    pub session_id: String,

    pub client_type: prod_mc_cli_chat_proxy_types::feedback_types::ClientType,

    #[serde(default)]
    pub rating_type: Option<prod_mc_cli_chat_proxy_types::feedback_types::RatingType>,

    /// Rating value (interpretation depends on rating_type).
    /// thumbs: -1 (down), 0 (neutral), 1 (up).
    /// Values are clamped to valid ranges on the agent side.
    #[serde(default)]
    pub rating_value: Option<i32>,

    #[serde(default)]
    pub feedback_text: Option<String>,

    #[serde(default)]
    pub images: Vec<prod_mc_cli_chat_proxy_types::feedback_types::FeedbackImage>,

    /// Feedback categories (e.g., ["accuracy", "speed", "helpfulness"])
    #[serde(default)]
    pub feedback_categories: Vec<String>,

    #[serde(default)]
    pub context_type: Option<prod_mc_cli_chat_proxy_types::feedback_types::ContextType>,

    /// 0-based turn number this feedback is about.
    #[serde(default, alias = "turnNumber")]
    pub turn_number: Option<i64>,

    /// Feedback request ID: if present, this is a response to a FeedbackRequestNotification (i.e., solicited feedback).
    /// If absent, this is spontaneous user feedback.
    #[serde(default)]
    pub request_id: Option<String>,

    #[serde(default)]
    pub client_version: Option<String>,

    #[serde(default)]
    pub metadata: Option<serde_json::Value>,

    #[serde(default)]
    pub terminal_info: Option<prod_mc_cli_chat_proxy_types::feedback_types::FeedbackTerminalInfo>,

    /// Requests a shell-issued one-shot trace capability after this feedback is accepted.
    #[serde(default, alias = "requestTraceUploadToken")]
    pub request_trace_upload_token: bool,
}

impl ClientFeedbackInput {
    fn clamp_rating_value(
        rating_type: Option<prod_mc_cli_chat_proxy_types::feedback_types::RatingType>,
        rating_value: Option<i32>,
    ) -> Option<i32> {
        use prod_mc_cli_chat_proxy_types::feedback_types::RatingType;

        match (rating_type, rating_value) {
            (Some(RatingType::Thumbs), Some(v)) => Some(v.clamp(-1, 1)),
            (Some(RatingType::Stars), Some(v)) => Some(v.clamp(1, 5)),
            (Some(RatingType::Nps), Some(v)) => Some(v.clamp(0, 10)),
            // No rating type specified, pass through (will be validated by server)
            (None, Some(v)) => Some(v),
            (_, None) => None,
        }
    }

    /// Convert to a FeedbackSubmission for sending to the feedback backend.
    /// `user_id` is absent here; the backend extracts it from the auth token.
    /// `&mut self`: drains `images` into the submission instead of cloning megabytes of base64; the input is not read for images afterwards.
    pub fn take_submission(
        &mut self,
        model_id: Option<String>,
        resolved_model_id: Option<String>,
        model_fingerprint: Option<String>,
        turn_number: Option<i64>,
    ) -> prod_mc_cli_chat_proxy_types::feedback_types::FeedbackSubmission {
        use prod_mc_cli_chat_proxy_types::feedback_types::FeedbackContent;

        let clamped_rating_value = Self::clamp_rating_value(self.rating_type, self.rating_value);
        let content = match (
            self.rating_type,
            clamped_rating_value,
            self.feedback_text.clone(),
        ) {
            (Some(rating_type), Some(rating_value), Some(text)) => {
                FeedbackContent::RatingWithText {
                    rating_type,
                    rating_value,
                    text,
                }
            }
            (Some(rating_type), Some(rating_value), None) => FeedbackContent::Rating {
                rating_type,
                rating_value,
            },
            // Fallback: any other shape becomes Text (empty string preserved).
            (_, _, text) => FeedbackContent::Text(text.unwrap_or_default()),
        };

        let mut s = new_submission(self.session_id.clone(), self.client_type, content);
        s.turn_number = turn_number;
        s.images = std::mem::take(&mut self.images);
        s.feedback_categories = self.feedback_categories.clone();
        s.model_id = model_id;
        s.resolved_model_id = resolved_model_id;
        s.model_fingerprint = model_fingerprint;
        s.context_type = self.context_type;
        s.request_id = self.request_id.clone();
        s.client_version = self.client_version.clone();
        s.metadata = self.metadata.clone();
        s.terminal_info = self.terminal_info.clone();
        s
    }

    pub fn is_solicited(&self) -> bool {
        self.request_id.is_some()
    }

    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

/// `x.ai/feedback/drafts/update` params, built by the pager and parsed by the shell. The full body
/// is required so a partial update fails the parse instead of half-updating the draft.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FeedbackDraftUpdateRequest {
    pub session_id: String,
    pub draft_id: xai_grok_feedback::FeedbackDraftId,
    #[serde(flatten)]
    pub input: xai_grok_feedback::FeedbackDraftInput,
}

/// The `draft_id` variant of `x.ai/feedback` params, built by the pager and parsed by the shell.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackDraftSendRequest {
    pub session_id: String,
    pub draft_id: xai_grok_feedback::FeedbackDraftId,
    #[serde(default)]
    pub request_trace_upload_token: bool,
    pub edited_body: FeedbackDraftEditedBody,
}

/// `edited_body` of [`FeedbackDraftSendRequest`]: the edited draft plus the pager's client context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedbackDraftEditedBody {
    #[serde(flatten)]
    pub input: xai_grok_feedback::FeedbackDraftInput,
    #[serde(default)]
    pub images: Vec<prod_mc_cli_chat_proxy_types::feedback_types::FeedbackImage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_info: Option<prod_mc_cli_chat_proxy_types::feedback_types::FeedbackTerminalInfo>,
}

/// Pager attestation carried on the one-shot `x.ai/feedback/upload-trace` request. Deliberately no
/// catch-all variant: an unknown intent fails the request instead of changing its gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackTraceUploadIntent {
    SendThisSession,
}

pub fn new_submission(
    session_id: String,
    client_type: ClientType,
    content: FeedbackContent,
) -> FeedbackSubmission {
    let mut s = FeedbackSubmission::with_content(session_id, client_type, content);
    s.shell_version = Some(xai_grok_version::VERSION.to_string());
    s
}
