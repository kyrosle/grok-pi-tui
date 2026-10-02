//! Memory policy interface; storage and ordinary file execution stay with their owners.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Result of routing a write through the memory v2 policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryV2Write {
    /// The path is outside configured memory v2 roots; preserve normal tool behavior.
    Outside,
    /// The policy completed the write and refreshed the generated manifest.
    Written { previous_content: Option<Vec<u8>> },
}

/// Session-scoped policy implemented by the memory storage layer.
pub trait MemoryV2Access: std::fmt::Debug + Send + Sync {
    /// Validate a read/search/list target. Returns whether it belongs to memory v2.
    fn validate_read(&self, path: &Path) -> Result<bool, String>;

    /// Record the full content observed by a successful ordinary file read.
    fn record_read(&self, path: &Path, contents: &[u8]) -> Result<(), String>;

    /// Validate a create/replace without persisting it.
    ///
    /// Returns whether the path belongs to memory v2. Implementations must
    /// perform the same deterministic policy checks as [`Self::write_file`].
    fn preflight_write(&self, path: &Path, contents: &[u8]) -> Result<bool, String>;

    /// Atomically create or replace a permitted memory v2 file.
    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<MemoryV2Write, String>;

    /// Return both configured scope roots for prompt and UI metadata.
    fn scope_roots(&self) -> [PathBuf; 2];
}

/// Ephemeral ToolBridge resource shared by all ordinary file tools in a session.
#[derive(Clone)]
pub struct MemoryV2AccessResource(pub Arc<dyn MemoryV2Access>);

impl std::fmt::Debug for MemoryV2AccessResource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("MemoryV2AccessResource").finish()
    }
}
