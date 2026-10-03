pub const MCP_TOOL_NAME_DELIMITER: &str = "__";
pub fn parse_mcp_qualified_name(name: &str) -> Option<(crate::ToolId, &str, &str)> {
    let delimiter = MCP_TOOL_NAME_DELIMITER.as_bytes();
    // Byte windows preserve both overlapping `__` boundaries in `___`.
    let mut boundaries = name
        .as_bytes()
        .windows(delimiter.len())
        .enumerate()
        .filter_map(|(index, window)| (window == delimiter).then_some(index));
    let boundary = boundaries.next()?;
    if boundaries.next().is_some() {
        return None;
    }
    let (server, tool_with_delimiter) = name.split_at(boundary);
    let tool = &tool_with_delimiter[MCP_TOOL_NAME_DELIMITER.len()..];
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((crate::ToolId::new(name).ok()?, server, tool))
}

#[cfg(test)]
mod tests {
    #[test]
    fn mcp_names_preserve_the_original_protocol_validation() {
        assert!(super::parse_mcp_qualified_name("server__tool").is_some());
        for name in [
            "server_tool",
            "server___tool",
            "__tool",
            "server__",
            "server__a__b",
            "server__bad name",
        ] {
            assert!(super::parse_mcp_qualified_name(name).is_none(), "{name}");
        }
    }
}
