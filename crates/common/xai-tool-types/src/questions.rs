//! Question data and ACP wire contracts shared by native UI and tool producers.
use indexmap::IndexMap;
use std::collections::HashMap;

/// A single option within a question.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct QuestionOption {
    /// Option text shown to the user; a few words at most.
    #[schemars(description = "Option text shown to the user. A few words at most.")]
    pub label: String,

    /// What picking this option means or implies.
    #[schemars(description = "What picking this option means or implies.")]
    pub description: String,

    /// Optional content shown while the option is focused — mockups, code
    /// snippets, anything the user should compare. Single-select only.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Optional content shown while the option is focused — mockups, code snippets, anything the user should compare. Single-select questions only."
    )]
    pub preview: Option<String>,

    /// Opaque id; hidden from the model. Grok callers leave it `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub id: Option<String>,
}

/// A single question with its options.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// The question to ask, phrased as a full question.
    #[schemars(description = "The question to ask, phrased as a full question.")]
    pub question: String,

    /// The choices for this question.
    #[schemars(description = "The choices for this question.")]
    pub options: Vec<QuestionOption>,

    /// Let the user pick more than one option (default false). Model-facing schema name is
    /// snake_case (`multi_select`); deserialize also accepts the legacy/ACP `multiSelect` so the
    /// shared `Question` type stays wire-compatible with the camelCase ACP ext_method.
    #[serde(
        default,
        alias = "multi_select",
        deserialize_with = "crate::deserialize_lenient_option_bool"
    )]
    #[schemars(
        rename = "multi_select",
        description = "Let the user pick more than one option (default false)."
    )]
    pub multi_select: Option<bool>,

    /// See `QuestionOption.id`. Hidden from the JSON schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub id: Option<String>,
}

/// Annotation on a single question's answer. Carried inside the `accepted` response alongside the
/// selected label. `preview`: verbatim `Option.preview` of the selected option (single-select
/// only). `notes`: free-text the user typed in the freeform input.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuestionAnnotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Mode context for the question UI. Sent as part of the ACP `ext_method` request so the pager
/// knows whether to show plan-mode-only actions (Chat about this / Skip interview).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskUserQuestionMode {
    /// Normal mode. Client shows only Accept and Cancel.
    Default,
    /// Plan mode. Client shows Accept, Cancel, Chat about this, Skip interview.
    Plan,
}

/// ACP `ext_method` request payload (shell coordinator sends to client/pager).
///
/// Serialized as `camelCase` for the ACP JSON-RPC wire format.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskUserQuestionExtRequest {
    pub session_id: String,
    pub tool_call_id: String,
    pub questions: Vec<Question>,
    /// Controls whether the client shows plan-mode-only actions.
    pub mode: AskUserQuestionMode,
}

/// Accepts both `"value"` (old wire format) and `["value"]` (new wire format)
/// for each answer entry, normalizing strings into single-element vectors.
fn deserialize_string_or_vec_answers<'de, D>(
    deserializer: D,
) -> Result<IndexMap<String, Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        Vec(Vec<String>),
        String(String),
    }

    let raw: IndexMap<String, StringOrVec> = serde::Deserialize::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|(k, v)| match v {
            StringOrVec::Vec(vec) => (k, vec),
            StringOrVec::String(s) => (k, vec![s]),
        })
        .collect())
}

/// ACP `ext_method` response payload (client/pager returns to shell coordinator). Internally tagged
/// on `"outcome"` with `snake_case` variant names so the JSON looks like `{ "outcome": "accepted",
/// "answers": { ... } }`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AskUserQuestionExtResponse {
    /// User accepted and submitted answers (Path A).
    Accepted {
        /// Answered questions in original order; unanswered omitted.
        /// One element per selected option; freeform-only is `["Other"]`
        /// with typed text in `annotations[q].notes`.
        #[serde(deserialize_with = "deserialize_string_or_vec_answers")]
        answers: IndexMap<String, Vec<String>>,
        /// Per-question annotations (preview, notes). Absent when empty.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations: Option<HashMap<String, QuestionAnnotation>>,
    },
    /// User chose "Chat about this" (Path B, plan mode only).
    ChatAboutThis {
        /// Partial answers: answered questions only, label only (no notes).
        /// Freeform-only => `"Other"` (notes dropped in plan-mode paths).
        #[serde(default)]
        partial_answers: HashMap<String, String>,
    },
    /// User chose "Skip interview and plan immediately" (Path C, plan mode only).
    SkipInterview {
        /// Same partial-answer rules as `ChatAboutThis`.
        #[serde(default)]
        partial_answers: HashMap<String, String>,
    },
    /// User cancelled / dismissed (Path D). NOT an error.
    Cancelled,
}

/// In-process result: coordinator -> tool. `Ok(UserQuestionResponse)` for all 4 user paths
/// (accepted, chat, skip, cancel). `Err(UserQuestionError)` for transport failures or malformed
/// responses.
pub type UserQuestionResult = Result<UserQuestionResponse, UserQuestionError>;

/// Successful user response (all 4 user paths). Every variant here produces `Ok(UserAnswered {
/// message })` at the tool level with `ToolCall` status `Completed`.
#[derive(Debug, Clone)]
pub enum UserQuestionResponse {
    /// User accepted and submitted answers (Path A).
    Accepted {
        /// See `AskUserQuestionExtResponse::Accepted::answers`.
        answers: IndexMap<String, Vec<String>>,
        annotations: Option<HashMap<String, QuestionAnnotation>>,
    },
    /// User chose "Chat about this" (Path B, plan mode only).
    /// Carries the original questions so the formatter can iterate all of them.
    ChatAboutThis {
        questions: Vec<Question>,
        partial_answers: HashMap<String, String>,
    },
    /// User chose "Skip interview" (Path C, plan mode only).
    /// Carries the original questions so the formatter can iterate all of them.
    SkipInterview {
        questions: Vec<Question>,
        partial_answers: HashMap<String, String>,
    },
    /// User explicitly dismissed (Esc). NOT an error.
    Cancelled,
}

/// Infrastructure failure (NOT a user action). These produce `Err(ToolError::ExecutionError { ..
/// })` at the tool level with `ToolCall` status `Failed`.
#[derive(Debug, Clone)]
pub enum UserQuestionError {
    /// ACP `ext_method` call failed (client disconnect, timeout, etc.).
    TransportError(String),
    /// Client returned JSON that could not be deserialized into
    /// `AskUserQuestionExtResponse`.
    MalformedResponse(String),
}

impl AskUserQuestionExtResponse {
    /// Convert the wire-format ACP response into the in-process response type. Called by the shell coordinator after deserializing the client's
    /// JSON. The `questions` parameter carries the original question list so that `ChatAboutThis` and `SkipInterview` responses can iterate all
    /// questions (answered and unanswered) when formatting the tool result.
    pub fn into_response(self, questions: Vec<Question>) -> UserQuestionResponse {
        match self {
            Self::Accepted {
                answers,
                annotations,
            } => UserQuestionResponse::Accepted {
                answers,
                annotations,
            },
            Self::ChatAboutThis { partial_answers } => UserQuestionResponse::ChatAboutThis {
                questions,
                partial_answers,
            },
            Self::SkipInterview { partial_answers } => UserQuestionResponse::SkipInterview {
                questions,
                partial_answers,
            },
            Self::Cancelled => UserQuestionResponse::Cancelled,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────
