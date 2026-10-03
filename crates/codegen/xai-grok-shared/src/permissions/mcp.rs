use xai_tool_protocol::parse_mcp_qualified_name;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BashCommandPermission {
    pub prompt_prefix: String,
}

/// The command terms the user selected; the selection is independent of the prompt's outcome.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BashCommandSelectedTerms {
    pub command_parts: Vec<String>,
    /// The user typed a free-form glob in the pattern editor rather than selecting literal words, so match it as a glob.
    #[serde(default)]
    pub is_glob: bool,
}

/// Delimiter that qualifies MCP tool names as `"<server>__<tool>"`.
/// Defined in `xai_grok_workspace_types`; re-exported here because callers historically reached it through this module.
/// MCP registration validates the delimiter before permission handling, so stripping it given a trusted `server_prefix` is unambiguous.
pub use xai_tool_protocol::MCP_TOOL_NAME_DELIMITER;

/// Extract the action segment of a qualified MCP tool name using a trusted `server_prefix`.
/// Returns the full `tool_name` when there is no server prefix.
/// When there is one, debug builds assert that `tool_name` starts with `"<server_prefix>__"`.
pub fn mcp_tool_action<'a>(tool_name: &'a str, server_prefix: Option<&str>) -> &'a str {
    let Some(prefix) = server_prefix else {
        return tool_name;
    };
    let action = tool_name
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix(MCP_TOOL_NAME_DELIMITER));
    debug_assert!(
        action.is_some(),
        "MCP tool name invariant: '{tool_name}' should start with '{prefix}{MCP_TOOL_NAME_DELIMITER}'"
    );
    action.unwrap_or(tool_name)
}

/// Pretty-format one MCP server- or tool-name segment: split on `'_'`, title-case each word, join with spaces.
/// Non-underscore characters (camelCase, hyphens) stay intact: `"list_issues"` becomes `"List Issues"` but `"getMyTaskList"` stays `"GetMyTaskList"`.
pub fn mcp_titleize_segment(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// User-facing tool label, e.g. `"(Linear) List Issues"`; falls back to the title-cased `tool_name` when there is no server prefix.
/// The `"(Server) Action"` form shows the owning server without relying on color (some clients are monochrome or use color for other state).
pub fn mcp_tool_display_name(tool_name: &str, server_prefix: Option<&str>) -> String {
    let action = mcp_tool_action(tool_name, server_prefix);
    match server_prefix {
        Some(server) => format!(
            "({}) {}",
            mcp_titleize_segment(server),
            mcp_titleize_segment(action)
        ),
        None => mcp_titleize_segment(tool_name),
    }
}

/// Display variant for callers that only have a tool-name string, such as ACP activity titles or scrollback blocks storing the wire name verbatim.
/// A valid qualified name formats as `"(Server) Action"` with each segment title-cased.
/// Anything else is returned unchanged: the input may be a bash command, file path, or other non-MCP text.
pub fn mcp_pretty_name_if_qualified(name: &str) -> String {
    match parse_mcp_qualified_name(name) {
        Some((_, server, action)) => format!(
            "({}) {}",
            mcp_titleize_segment(server),
            mcp_titleize_segment(action)
        ),
        None => name.to_owned(),
    }
}

/// Meta attached to the "Always allow" option for an MCP tool prompt.
/// Carries the full tool name and the server-prefix segment so the view can render the scope toggle without re-parsing the name.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpToolPermission {
    /// Static label prefix shown before the dynamic scope text, e.g. `"Always allow:"`.
    /// Mirrors `BashCommandPermission::prompt_prefix`.
    pub prompt_prefix: String,
    /// Full tool name as the agent called it (e.g. `"grok_com_notion__notion-fetch"`).
    pub tool_name: String,
    /// Server component of a valid qualified MCP ID (e.g. `"grok_com_notion"`).
    /// `None` for malformed or unqualified names, in which case the view hides the scope toggle and only offers tool-scope.
    pub server_prefix: Option<String>,
}

impl McpToolPermission {
    /// Action segment of the qualified tool name. See [`mcp_tool_action`].
    pub fn action(&self) -> &str {
        mcp_tool_action(&self.tool_name, self.server_prefix.as_deref())
    }

    /// User-facing tool label. See [`mcp_tool_display_name`].
    pub fn display_name(&self) -> String {
        mcp_tool_display_name(&self.tool_name, self.server_prefix.as_deref())
    }
}

/// User's selected scope for an MCP "always allow" grant.
/// Sent back from the view in `RequestPermissionResponse::meta` when the user picks the AllowAlways option for an MCP prompt.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpScopeSelection {
    /// Whitelist exactly this tool name.
    Tool { tool_name: String },
    /// Whitelist the server component of the current valid qualified MCP ID.
    Server { server: String },
}
