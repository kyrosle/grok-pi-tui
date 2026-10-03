//! Shared metadata flags on ACP content chunks.

use agent_client_protocol as acp;

pub const HOST_TURN_META_KEY: &str = "hostTurn";

pub fn is_host_turn_chunk(chunk: &acp::ContentChunk) -> bool {
    chunk_meta_flag(chunk, HOST_TURN_META_KEY)
}

/// `ContentChunk._meta` flag on a persisted mid-turn interjection's user chunks. The text block keeps the
/// model-facing frame and carries the typed text in `displayText`; the pager and the chat rebuilders key on this.
pub const INTERJECTION_META_KEY: &str = "interjection";

pub fn is_interjection_chunk(chunk: &acp::ContentChunk) -> bool {
    chunk_meta_flag(chunk, INTERJECTION_META_KEY)
}

/// Boolean `ContentChunk._meta` flag; anything but a literal `true` reads as `false`.
pub fn chunk_meta_flag(chunk: &acp::ContentChunk, key: &str) -> bool {
    chunk
        .meta
        .as_ref()
        .and_then(|m| m.get(key))
        .and_then(|v| v.as_bool())
        == Some(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn flags_accept_only_literal_true() {
        let mut chunk =
            agent_client_protocol::ContentChunk::new(agent_client_protocol::ContentBlock::Text(
                agent_client_protocol::TextContent::new("body"),
            ));
        for value in [
            serde_json::json!(false),
            serde_json::json!("true"),
            serde_json::json!(1),
            serde_json::Value::Null,
        ] {
            chunk.meta = Some(serde_json::Map::from_iter([(
                super::INTERJECTION_META_KEY.into(),
                value,
            )]));
            assert!(!super::is_interjection_chunk(&chunk));
        }
        chunk.meta = Some(serde_json::Map::from_iter([(
            super::INTERJECTION_META_KEY.into(),
            serde_json::json!(true),
        )]));
        assert!(super::is_interjection_chunk(&chunk));
    }
}

/// Original stock persistent-memory mode wire key; absent for native Pi sessions.
pub const MEMORY_MODE_META_KEY: &str = "x.ai/memoryMode";
