//! Shared protocol and channel types for the AskUserQuestion blocking flow.
//!
//! These types define the request/response contract between three crates:
//!
//! - **`xai-grok-tools`** — tool blocks on a oneshot, formats the result.
//! - **`xai-grok-shell`** — coordinator receives requests over mpsc, calls the
//!   client via ACP `ext_method`, sends results back over the oneshot.
//! - **`xai-grok-pager`** — handles the `ExtMethod`, renders UI, returns a
//!   typed response.
//!
//! All three crates import these types from `xai-grok-tools`.

#[cfg(test)]
use std::collections::HashMap;

use educe::Educe;
#[cfg(test)]
use indexmap::IndexMap;
use tokio::sync::{mpsc, oneshot};

use super::Question;
use crate::register_resource;

// ── ACP wire-format types ────────────────────────────────────────────────

pub use xai_tool_types::questions::{
    AskUserQuestionExtRequest, AskUserQuestionExtResponse, AskUserQuestionMode, QuestionAnnotation,
    UserQuestionError, UserQuestionResponse, UserQuestionResult,
};

// ── In-process types (coordinator <-> tool) ──────────────────────────────

/// In-process request: tool -> coordinator (carries oneshot for reply). Sent over the `mpsc`
/// channel. The coordinator receives this, performs the ACP `ext_method` round-trip, and sends the
/// result back on `result_tx`.
#[derive(Educe)]
#[educe(Debug)]
pub struct UserQuestionRequest {
    pub tool_call_id: String,
    pub questions: Vec<Question>,
    #[educe(Debug(ignore))]
    pub result_tx: oneshot::Sender<UserQuestionResult>,
}

// ── Resource type ────────────────────────────────────────────────────────

/// Resource: `mpsc` sender injected into `SharedResources`. Same injection pattern as
/// `SubagentEventSender`. Cloned into each session so that any `AskUserQuestionTool` invocation can
/// emit a `UserQuestionRequest` to the session's coordinator.
#[derive(Clone, Educe)]
#[educe(Debug)]
pub struct UserQuestionSender(
    #[educe(Debug(ignore))] pub mpsc::UnboundedSender<UserQuestionRequest>,
);

register_resource!("grok_build", "UserQuestionSender", UserQuestionSender);

// ── Conversion helper ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers --

    fn sample_questions() -> Vec<Question> {
        vec![
            Question {
                question: "Which database?".to_string(),
                options: vec![
                    super::super::QuestionOption {
                        label: "Redis".to_string(),
                        description: "In-memory store".to_string(),
                        preview: Some("<div>redis preview</div>".to_string()),
                        id: None,
                    },
                    super::super::QuestionOption {
                        label: "Postgres".to_string(),
                        description: "Relational DB".to_string(),
                        preview: None,
                        id: None,
                    },
                ],
                multi_select: None,
                id: None,
            },
            Question {
                question: "Which framework?".to_string(),
                options: vec![
                    super::super::QuestionOption {
                        label: "React".to_string(),
                        description: "UI library".to_string(),
                        preview: None,
                        id: None,
                    },
                    super::super::QuestionOption {
                        label: "Vue".to_string(),
                        description: "Progressive framework".to_string(),
                        preview: None,
                        id: None,
                    },
                ],
                multi_select: None,
                id: None,
            },
        ]
    }

    // -- AskUserQuestionMode serde --

    #[test]
    fn mode_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_value(AskUserQuestionMode::Default).unwrap(),
            serde_json::json!("default")
        );
        assert_eq!(
            serde_json::to_value(AskUserQuestionMode::Plan).unwrap(),
            serde_json::json!("plan")
        );
    }

    #[test]
    fn mode_round_trips() {
        for mode in [AskUserQuestionMode::Default, AskUserQuestionMode::Plan] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: AskUserQuestionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    // -- AskUserQuestionExtRequest serde --

    #[test]
    fn ext_request_serializes_camel_case() {
        let req = AskUserQuestionExtRequest {
            session_id: "sess-1".to_string(),
            tool_call_id: "tc-1".to_string(),
            questions: vec![],
            mode: AskUserQuestionMode::Plan,
        };
        let json = serde_json::to_value(&req).unwrap();
        // camelCase field names
        assert!(json.get("sessionId").is_some());
        assert!(json.get("toolCallId").is_some());
        assert_eq!(json["mode"], "plan");
    }

    #[test]
    fn ext_request_round_trips() {
        let req = AskUserQuestionExtRequest {
            session_id: "sess-1".to_string(),
            tool_call_id: "tc-1".to_string(),
            questions: sample_questions(),
            mode: AskUserQuestionMode::Default,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: AskUserQuestionExtRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "sess-1");
        assert_eq!(back.tool_call_id, "tc-1");
        assert_eq!(back.questions.len(), 2);
        assert_eq!(back.mode, AskUserQuestionMode::Default);
    }

    // -- AskUserQuestionExtResponse serde --

    #[test]
    fn ext_response_accepted_serializes_tagged() {
        let mut answers = IndexMap::new();
        answers.insert("Which database?".to_string(), vec!["Redis".to_string()]);

        let mut annotations = HashMap::new();
        annotations.insert(
            "Which database?".to_string(),
            QuestionAnnotation {
                preview: Some("<div>redis preview</div>".to_string()),
                notes: None,
            },
        );

        let resp = AskUserQuestionExtResponse::Accepted {
            answers,
            annotations: Some(annotations),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["outcome"], "accepted");
        assert!(json["answers"].is_object());
        assert!(json["annotations"].is_object());
    }

    #[test]
    fn ext_response_accepted_omits_empty_annotations() {
        let mut answers = IndexMap::new();
        answers.insert("Q?".to_string(), vec!["A".to_string()]);

        let resp = AskUserQuestionExtResponse::Accepted {
            answers,
            annotations: None,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["outcome"], "accepted");
        assert!(json.get("annotations").is_none());
    }

    #[test]
    fn ext_response_chat_about_this_serializes() {
        let mut partial = HashMap::new();
        partial.insert("Which database?".to_string(), "Redis".to_string());

        let resp = AskUserQuestionExtResponse::ChatAboutThis {
            partial_answers: partial,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["outcome"], "chat_about_this");
        assert!(json["partial_answers"].is_object());
    }

    #[test]
    fn ext_response_skip_interview_serializes() {
        let resp = AskUserQuestionExtResponse::SkipInterview {
            partial_answers: HashMap::new(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["outcome"], "skip_interview");
    }

    #[test]
    fn ext_response_cancelled_serializes() {
        let resp = AskUserQuestionExtResponse::Cancelled;
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["outcome"], "cancelled");
    }

    #[test]
    fn ext_response_round_trips_all_variants() {
        // Accepted
        let mut answers = IndexMap::new();
        answers.insert("Q1?".to_string(), vec!["A1".to_string()]);
        let accepted = AskUserQuestionExtResponse::Accepted {
            answers,
            annotations: None,
        };
        let json = serde_json::to_string(&accepted).unwrap();
        let back: AskUserQuestionExtResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AskUserQuestionExtResponse::Accepted { .. }));

        // ChatAboutThis
        let chat = AskUserQuestionExtResponse::ChatAboutThis {
            partial_answers: HashMap::new(),
        };
        let json = serde_json::to_string(&chat).unwrap();
        let back: AskUserQuestionExtResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            AskUserQuestionExtResponse::ChatAboutThis { .. }
        ));

        // SkipInterview
        let skip = AskUserQuestionExtResponse::SkipInterview {
            partial_answers: HashMap::new(),
        };
        let json = serde_json::to_string(&skip).unwrap();
        let back: AskUserQuestionExtResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            AskUserQuestionExtResponse::SkipInterview { .. }
        ));

        // Cancelled
        let cancel = AskUserQuestionExtResponse::Cancelled;
        let json = serde_json::to_string(&cancel).unwrap();
        let back: AskUserQuestionExtResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, AskUserQuestionExtResponse::Cancelled));
    }

    // -- into_response conversion --

    #[test]
    fn into_response_accepted() {
        let mut answers = IndexMap::new();
        answers.insert("Which database?".to_string(), vec!["Redis".to_string()]);

        let mut annotations = HashMap::new();
        annotations.insert(
            "Which database?".to_string(),
            QuestionAnnotation {
                preview: Some("preview".to_string()),
                notes: Some("my notes".to_string()),
            },
        );

        let ext = AskUserQuestionExtResponse::Accepted {
            answers: answers.clone(),
            annotations: Some(annotations),
        };
        let resp = ext.into_response(sample_questions());

        match resp {
            UserQuestionResponse::Accepted {
                answers: a,
                annotations: ann,
            } => {
                assert_eq!(a, answers);
                assert!(ann.is_some());
                let ann = ann.unwrap();
                assert!(ann.contains_key("Which database?"));
            }
            other => panic!("Expected Accepted, got {:?}", other),
        }
    }

    #[test]
    fn into_response_chat_about_this_carries_questions() {
        let mut partial = HashMap::new();
        partial.insert("Which database?".to_string(), "Redis".to_string());

        let questions = sample_questions();
        let ext = AskUserQuestionExtResponse::ChatAboutThis {
            partial_answers: partial.clone(),
        };
        let resp = ext.into_response(questions.clone());

        match resp {
            UserQuestionResponse::ChatAboutThis {
                questions: q,
                partial_answers: p,
            } => {
                assert_eq!(q.len(), questions.len());
                assert_eq!(p, partial);
            }
            other => panic!("Expected ChatAboutThis, got {:?}", other),
        }
    }

    #[test]
    fn into_response_skip_interview_carries_questions() {
        let questions = sample_questions();
        let ext = AskUserQuestionExtResponse::SkipInterview {
            partial_answers: HashMap::new(),
        };
        let resp = ext.into_response(questions.clone());

        match resp {
            UserQuestionResponse::SkipInterview {
                questions: q,
                partial_answers: p,
            } => {
                assert_eq!(q.len(), questions.len());
                assert!(p.is_empty());
            }
            other => panic!("Expected SkipInterview, got {:?}", other),
        }
    }

    #[test]
    fn into_response_cancelled() {
        let ext = AskUserQuestionExtResponse::Cancelled;
        let resp = ext.into_response(sample_questions());
        assert!(matches!(resp, UserQuestionResponse::Cancelled));
    }

    // -- QuestionAnnotation serde --

    #[test]
    fn annotation_omits_none_fields() {
        let ann = QuestionAnnotation {
            preview: None,
            notes: None,
        };
        let json = serde_json::to_value(&ann).unwrap();
        assert!(json.get("preview").is_none());
        assert!(json.get("notes").is_none());
    }

    #[test]
    fn annotation_includes_present_fields() {
        let ann = QuestionAnnotation {
            preview: Some("prev".to_string()),
            notes: Some("note".to_string()),
        };
        let json = serde_json::to_value(&ann).unwrap();
        assert_eq!(json["preview"], "prev");
        assert_eq!(json["notes"], "note");
    }

    // -- Backwards-compatible deserialization (string -> vec) --

    #[test]
    fn deserialize_accepted_old_string_format() {
        let raw = r#"{
            "outcome": "accepted",
            "answers": {"Which cache?": "Only hot-path caches"}
        }"#;
        let resp: AskUserQuestionExtResponse = serde_json::from_str(raw).unwrap();
        match resp {
            AskUserQuestionExtResponse::Accepted { answers, .. } => {
                assert_eq!(
                    answers["Which cache?"],
                    vec!["Only hot-path caches".to_string()]
                );
            }
            other => panic!("Expected Accepted, got {:?}", other),
        }
    }

    #[test]
    fn deserialize_accepted_mixed_string_and_vec() {
        let raw = r#"{
            "outcome": "accepted",
            "answers": {"Q1?": "old-style", "Q2?": ["new-style"]}
        }"#;
        let resp: AskUserQuestionExtResponse = serde_json::from_str(raw).unwrap();
        match resp {
            AskUserQuestionExtResponse::Accepted { answers, .. } => {
                assert_eq!(answers["Q1?"], vec!["old-style".to_string()]);
                assert_eq!(answers["Q2?"], vec!["new-style".to_string()]);
            }
            other => panic!("Expected Accepted, got {:?}", other),
        }
    }

    // -- Deserialization from raw JSON (simulating pager responses) --

    #[test]
    fn deserialize_accepted_from_raw_json() {
        let raw = r#"{
            "outcome": "accepted",
            "answers": {"Which database?": ["Redis"]},
            "annotations": {
                "Which database?": {
                    "preview": "<div>redis preview</div>"
                }
            }
        }"#;
        let resp: AskUserQuestionExtResponse = serde_json::from_str(raw).unwrap();
        match resp {
            AskUserQuestionExtResponse::Accepted {
                answers,
                annotations,
            } => {
                assert_eq!(answers["Which database?"], vec!["Redis".to_string()]);
                let ann = annotations.unwrap();
                assert_eq!(
                    ann["Which database?"].preview.as_deref(),
                    Some("<div>redis preview</div>")
                );
                assert!(ann["Which database?"].notes.is_none());
            }
            other => panic!("Expected Accepted, got {:?}", other),
        }
    }

    #[test]
    fn deserialize_accepted_without_annotations() {
        let raw = r#"{"outcome": "accepted", "answers": {"Q?": ["A"]}}"#;
        let resp: AskUserQuestionExtResponse = serde_json::from_str(raw).unwrap();
        match resp {
            AskUserQuestionExtResponse::Accepted { annotations, .. } => {
                assert!(annotations.is_none());
            }
            other => panic!("Expected Accepted, got {:?}", other),
        }
    }

    #[test]
    fn deserialize_chat_about_this_empty_partials() {
        let raw = r#"{"outcome": "chat_about_this"}"#;
        let resp: AskUserQuestionExtResponse = serde_json::from_str(raw).unwrap();
        match resp {
            AskUserQuestionExtResponse::ChatAboutThis { partial_answers } => {
                assert!(partial_answers.is_empty());
            }
            other => panic!("Expected ChatAboutThis, got {:?}", other),
        }
    }

    #[test]
    fn deserialize_cancelled() {
        let raw = r#"{"outcome": "cancelled"}"#;
        let resp: AskUserQuestionExtResponse = serde_json::from_str(raw).unwrap();
        assert!(matches!(resp, AskUserQuestionExtResponse::Cancelled));
    }

    // -- IndexMap ordering preservation --

    #[test]
    fn accepted_answers_preserve_insertion_order() {
        let mut answers = IndexMap::new();
        answers.insert("Third?".to_string(), vec!["C".to_string()]);
        answers.insert("First?".to_string(), vec!["A".to_string()]);
        answers.insert("Second?".to_string(), vec!["B".to_string()]);

        let resp = AskUserQuestionExtResponse::Accepted {
            answers,
            annotations: None,
        };

        // Round-trip through JSON
        let json = serde_json::to_string(&resp).unwrap();
        let back: AskUserQuestionExtResponse = serde_json::from_str(&json).unwrap();

        if let AskUserQuestionExtResponse::Accepted { answers, .. } = back {
            let keys: Vec<&String> = answers.keys().collect();
            assert_eq!(keys, vec!["Third?", "First?", "Second?"]);
        } else {
            panic!("Expected Accepted");
        }
    }
}
