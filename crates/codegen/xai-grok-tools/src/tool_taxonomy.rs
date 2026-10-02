//! Compatibility re-export of the canonical tool metadata vocabulary.
pub use xai_tool_types::taxonomy::*;

#[cfg(test)]
mod producer_tests {
    use super::*;
    use crate::types::tool::ToolKind;
    /// Every writing-visible spelling stays glued to its definition site:
    /// the map must agree with the live tool's `id()` and metadata `kind()`.
    #[test]
    fn writing_tool_kind_matches_definition_sites() {
        use crate::types::tool_metadata::ToolMetadata;
        use xai_tool_runtime::Tool;
        fn covered<T: Tool + ToolMetadata>(tool: T) {
            assert_eq!(
                writing_tool_kind(tool.id().as_str()),
                Some(ToolMetadata::kind(&tool)),
                "writing_tool_kind drifted for `{}`",
                tool.id()
            );
        }
        covered(crate::implementations::grok_build::SearchReplaceTool);
        covered(crate::implementations::grok_build::BashTool);
        covered(crate::implementations::grok_build::TodoWriteTool);
        covered(crate::implementations::grok_build::WorkflowTool);
        covered(crate::implementations::grok_build::ImageGenTool);
        covered(crate::implementations::grok_build::ImageEditTool);
        covered(crate::implementations::grok_build::ImageToVideoTool);
        covered(crate::implementations::grok_build::ReferenceToVideoTool);
        covered(crate::implementations::grok_build::AskUserQuestionTool);
        covered(crate::implementations::opencode::OpenCodeWriteTool);
        covered(crate::implementations::opencode::OpenCodeEditTool);
        covered(crate::implementations::opencode::OpenCodeBashTool);
        covered(crate::implementations::opencode::OpenCodeTodoWriteTool);
        covered(crate::implementations::codex::ApplyPatchTool);
        covered(crate::implementations::grok_build_hashline::HashlineEditTool);
    }
    /// Spellings with no instantiable definition site in this crate
    /// (client-facing renames) and the deliberate absences.
    #[test]
    fn writing_tool_kind_renames_and_absences() {
        assert_eq!(
            writing_tool_kind("run_terminal_command"),
            Some(ToolKind::Execute)
        );
        assert_eq!(writing_tool_kind("read_file"), None);
        assert_eq!(writing_tool_kind("grep"), None);
        assert_eq!(writing_tool_kind("list_dir"), None);
        assert_eq!(writing_tool_kind(crate::USE_TOOL_NAME), None);
        assert_eq!(writing_tool_kind(crate::SEARCH_TOOL_NAME), None);
    }
}
