//! Canonical edit details shared by tool producers and native diff rendering.

use serde::{Deserialize, Serialize};

/// Contains the edit details present as a struct
#[derive(Debug, Default, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchReplaceEditContextInformation {
    pub details: Vec<SearchReplaceEditDetail>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchReplaceEditDetail {
    /// The exact old string that was matched in the file
    pub old_string: String,
    /// 1-based line number where the match begins in the original file
    pub old_line: usize,
    /// The replacement string that was written
    pub new_string: String,
    /// 1-based line number where the replacement begins in the updated file
    pub new_line: usize,
    /// The context before the match
    pub context_before: String,
    /// The context after the match
    pub context_after: String,
    /// Leading text on the first line before the matched `old_string` begins. When the match starts mid-line (e.g., after
    /// indentation), this captures the prefix so the diff renderer can display proper alignment. Empty when the match
    /// starts at the beginning of a line or when unknown.
    #[serde(default)]
    pub line_prefix: String,
}
