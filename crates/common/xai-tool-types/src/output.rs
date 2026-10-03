use crate::SubagentCompletedOutput;
use crate::output_dependencies::SendSubagentMessageOutput;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use strip_ansi_escapes::strip_str;
/// `(added, removed)` line counts for the `edit.lines` telemetry counter.
pub fn line_diff(old: &str, new: &str) -> (i64, i64) {
    let mut added = 0i64;
    let mut removed = 0i64;
    for change in similar::TextDiff::from_lines(old, new).iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => added += 1,
            similar::ChangeTag::Delete => removed += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    (added, removed)
}
/// Wrapper for [`ToolOutput::Text`] so it can round-trip through
/// `#[serde(tag = "type")]` (internally-tagged enums require struct/map
/// payloads, not bare primitives).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextOutput {
    pub text: String,
    /// Background-task id for auto-wake suppression when set (see
    /// [`crate::reminders::task_completion::consumed_completion_ids`]);
    /// omitted from prompts and from JSON when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_completion_task_id: Option<String>,
}
impl From<String> for TextOutput {
    fn from(text: String) -> Self {
        Self {
            text,
            consumed_completion_task_id: None,
        }
    }
}
impl From<&str> for TextOutput {
    fn from(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            consumed_completion_task_id: None,
        }
    }
}
/// Wrapper for [`ToolOutput::Dynamic`] so it can round-trip through
/// `#[serde(tag = "type")]` (internally-tagged enums require struct/map
/// payloads, not bare primitives).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicOutput {
    pub value: serde_json::Value,
}
impl From<serde_json::Value> for DynamicOutput {
    fn from(value: serde_json::Value) -> Self {
        Self { value }
    }
}
/// Typed saved path for the media tools (`image_gen` / `video_gen` / `image_edit`), so consumers
/// read it directly instead of scraping the prose. A struct (not a bare `PathBuf`) is required:
/// `ToolOutput` is internally tagged and only accepts map payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaGenOutput {
    /// Absolute path to the saved media file. Empty for [`Self::uploaded`].
    pub path: PathBuf,
    /// Basename of the saved media file (for example, `8.jpg`).
    #[serde(default)]
    pub filename: String,
    /// Session-relative media directory name (for example, `images` or `videos`).
    #[serde(default)]
    pub session_folder: String,
    /// Set when the media was uploaded to a remote presigned URL (ZDR video
    /// output) and is not available locally; omitted otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uploaded_url: Option<String>,
}
impl MediaGenOutput {
    pub fn new(path: PathBuf) -> Self {
        let filename = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let session_folder = path
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            path,
            filename,
            session_folder,
            uploaded_url: None,
        }
    }
    /// Media uploaded to a remote presigned URL and not available locally
    /// (ZDR video output). No local path/filename/session folder.
    pub fn uploaded(url: String) -> Self {
        Self {
            path: PathBuf::new(),
            filename: String::new(),
            session_folder: String::new(),
            uploaded_url: Some(url),
        }
    }
    /// Model-facing prose. `action` is the variant's lead-in
    /// ("Image generated" / "Video generated" / "Image edited"); the trailing
    /// guidance stops the model re-reading or narrating the result.
    pub fn prompt_text(&self, action: &str) -> String {
        if let Some(url) = &self.uploaded_url {
            return format!(
                "{action} and uploaded to {url}. The file is not available locally — reference it by this URL. Do not read or re-display it, and do not describe how it appears to the user."
            );
        }
        let path = self.path.to_string_lossy().to_string();
        let message = format!(
            "{action} and saved to {path}. Do not read or re-display it, and do not describe how it appears to the user."
        );
        serde_json::json!({
            "path": path,
            "filename": &self.filename,
            "session_folder": &self.session_folder,
            "message": message,
        })
        .to_string()
    }
}
use crate::output_dependencies::SkillOutput;
use crate::presentation::{DEFAULT_SOFT_WRAP_WIDTH, soft_wrap_lines};
use crate::todo::{TodoItem, TodoState};
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListDirContent {
    /// Formatted directory listing string
    pub content: String,
    /// Root directory path (absolute) for this listing
    pub absolute_root_path: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ListDirOutput {
    Content(ListDirContent),
    /// Target path does not exist
    NotFound(String),
    /// Target path exists but is a file, not a directory
    IsAFile(String),
    /// Target path exists but is not a directory
    NotADirectory(String),
    /// Permission denied accessing the directory
    PermissionDenied(String),
    /// Generic / unclassified error
    Error(String),
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GrepLineMatch {
    pub line_number: usize,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GrepFileMatch {
    pub path: String,
    pub matches: Vec<GrepLineMatch>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GrepSearchOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub match_count: usize,
    #[serde(default)]
    pub file_matches: Vec<GrepFileMatch>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileContent {
    /// content here is the model friendly output which will always be present since even
    /// on failures we want to present the model with some information
    pub content: String,
    /// Concise version of content (arrow separator, no padding) for models
    /// that use the concise output format
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_concise: Option<String>,
    pub absolute_path: PathBuf,
    pub offset: Option<usize>,
    /// The line limit used for this read. `None` means no limit was applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Contains the raw output from the tool invocation without any formatting
    pub raw_output: String,
    /// Total number of lines in the file. Used by system reminders to detect
    /// offset-past-end vs genuinely-empty files.
    #[serde(default)]
    pub total_lines: usize,
    /// Pre-truncation image captures for session harvest. Must survive
    /// ToolDyn hub `to_value`/`from_value`; session drains before PostToolUse
    /// and ACP wire serialize.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(skip)]
    pub extracted_images: Vec<crate::output_dependencies::ExtractedImage>,
}
/// Image content returned when reading an image file. This is a local type so it can derive `schemars::JsonSchema`
/// v0.8, which the `Tool` trait requires for its `Output` associated type. Conversion to the protocol-level image type
/// happens at the protocol boundary in `xai-grok-shell`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImageContent {
    /// Base64-encoded image data
    pub data: String,
    /// MIME type of the image (e.g., "image/png", "image/jpeg")
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}
/// A single rendered PDF page.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PdfPageImage {
    /// Base64-encoded JPEG data
    pub data: String,
    /// MIME type (always "image/jpeg")
    pub mime_type: String,
    /// 1-based page number
    pub page_number: usize,
}
/// Multiple rendered PDF page images.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PdfPageImages {
    /// Rendered page images, one per requested page
    pub pages: Vec<PdfPageImage>,
    /// Total pages in the PDF document
    pub total_pages: usize,
    /// File size in bytes
    pub file_size: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ReadFileOutput {
    FileContent(FileContent),
    /// Target file does not exist
    FileNotFound(String),
    /// Target path is a directory, not a file
    IsADirectory(String),
    /// Permission denied reading the file
    PermissionDenied(String),
    /// File content exceeds maximum token limit
    FileTooLarge(String),
    /// Generic / unclassified read error
    FileReadError(String),
    ImageContent(ImageContent),
    ImageSizeError(String),
    PdfPageImages(PdfPageImages),
}
/// Represents successful edits applied by SearchReplace
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchReplaceEditsApplied {
    pub old_string: String,
    pub new_string: String,
    pub tool_output_for_prompt: String,
    /// Concise version of tool_output_for_prompt (shorter, no snippet) for
    /// models that use the concise output format
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_output_for_prompt_concise: Option<String>,
    pub absolute_path: PathBuf,
    pub edits: SearchReplaceEditContextInformation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
    /// `true` when the match used Unicode confusable normalization
    /// (exact byte match failed, but normalized match succeeded).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unicode_normalized: bool,
}
pub use crate::edit::{SearchReplaceEditContextInformation, SearchReplaceEditDetail};

/// Output of the codex `grep_files` tool — file paths matching a regex.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CodexGrepFilesOutput {
    /// Matching file paths, one per line.
    Matches { content: String, file_count: usize },
    /// No files matched the pattern.
    NoMatches(String),
    /// Error (e.g., path not found, rg failed).
    Error(String),
}
/// Per-file result included in a successful `ApplyPatchOutput`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApplyPatchFileResult {
    /// Absolute path to the affected file.
    pub path: PathBuf,
    /// What happened: `"added"`, `"modified"`, `"deleted"`, or `"moved"`.
    pub action: String,
    /// Full file content before the change. `None` for new files (add).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_text: Option<String>,
    /// Full file content after the change. Empty string for deleted files.
    pub new_text: String,
    /// Destination path (only for moves).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_to: Option<PathBuf>,
}
/// Output of the `apply_patch` tool.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ApplyPatchOutput {
    /// Patch applied successfully.
    Success {
        files: Vec<ApplyPatchFileResult>,
        tool_output_for_prompt: String,
    },
    /// Patch text could not be parsed.
    ParseError(String),
    /// Patch parsed but could not be applied to the filesystem.
    ApplicationError(String),
    /// No hunks in the patch.
    EmptyPatch(String),
}
/// Payload for `SearchReplaceOutput::NoMatchesFound`. Separate struct so consumers (reminders,
/// outcome trackers) can extract the file path without needing to know the call-site context.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NoMatchesFoundError {
    /// Human-readable error message shown to the model.
    pub message: String,
    /// Canonical absolute path of the file that was searched.
    pub file_path: std::path::PathBuf,
    /// Full file text from the same read the edit used when reporting no match. In-process only: never serialized on the
    /// wire (avoids leaking fresher or broader file content than the read/edit path already loaded). Used for `StrReplace`
    /// fuzzy hints without a second `read_file`.
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub file_snapshot_at_edit: Option<String>,
}
/// Output type for the SearchReplace tool
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum SearchReplaceOutput {
    FileAlreadyExists(String),
    EditsApplied(SearchReplaceEditsApplied),
    MultipleMatchesFound(String),
    /// The `old_string` was not found in the file.
    /// Carries the canonical absolute path so reminders and outcome trackers
    /// can do per-file accounting without needing extra context.
    NoMatchesFound(NoMatchesFoundError),
    InvalidInput(String),
    /// Target file does not exist
    FileNotFound(String),
    /// A path component exceeds the OS filename length limit (ENAMETOOLONG)
    FilenameTooLong(String),
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BashOutput {
    pub output: Vec<u8>,
    /// ANSI-stripped and soft-wrapped output for model prompt.
    /// Pre-baked at construction time so `to_prompt_format` is a simple read.
    #[serde(default)]
    pub output_for_prompt: String,
    pub exit_code: i32,
    pub command: String,
    pub truncated: bool,
    pub signal: Option<String>,
    pub timed_out: bool,
    /// describes the intent of this bash command
    pub description: Option<String>,
    /// the current working directory after the command completes
    pub current_dir: String,
    /// Path to the output file where full output is stored.
    /// Use read_file tool to retrieve full output when truncated.
    pub output_file: String,
    /// Total bytes of output (before truncation).
    pub total_bytes: usize,
    /// Incremental output delta (new bytes since last notification). When present, consumers should append to their
    /// accumulated buffer instead of replacing with `output`. When `Some(vec![])`, consumers should clear their accumulated
    /// buffer (reset signal). When `None`, the consumer should use `output` as the full buffer.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output_delta: Option<Vec<u8>>,
    /// Set by the grok_build `run_terminal_cmd` implementation when the command was detected as a bare `echo "<msg>"` (or close variant: echo -n,
    /// echo -e, simple printf for literal output, etc.). Telemetry / statistics on this pattern for the grok_build backend. Potential doom-loop /
    /// stagnation signals (repeated trivial echoes are a common "no progress" signal). Model hints (see BareEchoHintState in the bash tool).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub was_bare_echo: bool,
}
impl BashOutput {
    /// Compute `output_for_prompt` from raw output string.
    /// Strips ANSI escapes and soft-wraps long lines.
    pub fn make_output_for_prompt(raw: &str) -> String {
        let stripped = strip_str(raw);
        soft_wrap_lines(&stripped, DEFAULT_SOFT_WRAP_WIDTH)
    }
}
/// Output when a background task is started (matches the vendor-compat XML format)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BackgroundTaskStarted {
    /// Unique task ID (UUID) for querying later
    pub task_id: String,
    /// Type of background task (e.g., "bash")
    pub task_type: String,
    /// Path to the output file on disk
    pub output_file: String,
    /// Current status (always "running" when returned)
    pub status: String,
    /// The command that was started
    pub command: String,
    /// Human-readable summary
    pub summary: String,
    /// Pre-resolved hint text telling the model how to retrieve output.
    /// Built by the tool's run() using resolved tool/param names.
    #[serde(default)]
    pub retrieval_hint: String,
    /// Optional pre-formatted prompt body. When set, `to_prompt_format` uses this string verbatim instead of the default
    /// `<task-id>...</task-id>` XML envelope. Used by namespace-specific adapters that need to emit a different
    /// model-visible shape without disturbing the structured fields above (which other consumers still parse).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_formatted: Option<String>,
    /// PID of the spawned shell process, when available. Surfaced by
    /// adapters in their background-start template; left as `None` when the
    /// underlying backend cannot report a PID (e.g. ACP/remote terminals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebSearchOutput {
    pub query: String,
    pub content: String,
    pub citations: Vec<String>,
    pub allowed_domains: Option<Vec<String>>,
    /// When set, `to_prompt_format()` returns this text directly instead of
    /// wrapping `content` with the default header. Used by the compat adapter
    /// to produce the exact `Title: / Content: / ---` schema.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pre_formatted: Option<String>,
}
#[derive(Debug, Clone)]
pub struct WebFetchSourceArtifact {
    /// Session artifact containing the complete converted response.
    pub path: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebFetchOutputLocation {
    /// Absolute path to the complete rendered output.
    pub file_path: String,
    /// Exact file size in bytes.
    pub size_bytes: usize,
    /// Number of lines in the file.
    pub line_count: usize,
}
/// Successful web fetch result with page content.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WebFetchContent {
    /// The final URL (may differ from input after redirects).
    pub url: String,
    /// Page content converted to markdown (or raw text for non-HTML).
    pub content: String,
    /// Content type: "markdown" for converted HTML, or the original MIME type.
    pub content_type: String,
    /// HTTP status code.
    pub status_code: u16,
    /// Size of the content in bytes (before truncation).
    pub bytes: usize,
    /// Internal path to the complete converted body when GrokBuild persisted overflow.
    #[serde(skip)]
    #[schemars(skip)]
    pub source_artifact: Option<WebFetchSourceArtifact>,
    /// Structurally selected inline fallback when the complete body is unavailable.
    #[serde(skip)]
    #[schemars(skip)]
    pub inline_fallback: Option<String>,
    /// Vendor-compat file location for large rendered output.
    #[serde(
        rename = "outputLocation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub output_location: Option<WebFetchOutputLocation>,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum WebFetchOutput {
    /// Successful fetch with content.
    Content(WebFetchContent),
    /// Domain is not in the allowed domains list.
    DomainNotAllowed(String),
    /// Server redirected to a different host.
    CrossHostRedirect {
        original_host: String,
        redirect_url: String,
    },
    /// Pre-formatted error message (returned without the `Tool \`X\` failed:`
    /// wrapper that `ToolError` propagation would add).
    Error {
        url: Option<String>,
        message: String,
    },
}
impl WebFetchOutput {
    pub fn to_prompt_format(&self) -> String {
        match self {
            Self::Content(c) => c.content.clone(),
            Self::DomainNotAllowed(domain) => {
                format!(
                    "Error: domain {} is not in the allowed domains list",
                    domain
                )
            }
            Self::CrossHostRedirect {
                original_host,
                redirect_url,
            } => {
                format!(
                    "Error: cross-host redirect from {} to {}. Make a new web_fetch call with the redirect URL if needed.",
                    original_host, redirect_url
                )
            }
            Self::Error {
                url: Some(url),
                message,
            } => {
                format!("Error fetching URL {url}: {message}")
            }
            Self::Error { url: None, message } => format!("Error: {message}"),
        }
    }
}
use crate::KillTaskOutput;
use crate::TaskOutputOutput;
/// Output schema for the bash tool. The bash tool can either complete synchronously (`Bash`) or be
/// started in the background (`BackgroundTaskStarted`). This enum exists to provide a precise JSON
/// Schema via the `Tool::Output` associated type.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type")]
pub enum BashToolOutput {
    Bash(BashOutput),
    BackgroundTaskStarted(BackgroundTaskStarted),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchToolOutput {
    pub result_count: usize,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, derive_more::From)]
#[serde(tag = "type")]
pub enum ToolOutput {
    Bash(BashOutput),
    BackgroundTaskStarted(BackgroundTaskStarted),
    GrepSearch(GrepSearchOutput),
    ReadFile(ReadFileOutput),
    ListDir(ListDirOutput),
    SearchReplace(SearchReplaceOutput),
    Todo(TodoWriteOutput),
    WebSearch(WebSearchOutput),
    WebFetch(WebFetchOutput),
    MCP(MCPOutput),
    TaskOutput(TaskOutputOutput),
    KillTask(KillTaskOutput),
    Skill(SkillOutput),
    ApplyPatch(ApplyPatchOutput),
    CodexGrepFiles(CodexGrepFilesOutput),
    SearchTool(SearchToolOutput),
    SubagentCompleted(SubagentCompletedOutput),
    EnterPlanMode(EnterPlanModeOutput),
    ExitPlanMode(ExitPlanModeOutput),
    AskUserQuestion(AskUserQuestionOutput),
    #[serde(alias = "SendAgentMessage")]
    SendSubagentMessage(SendSubagentMessageOutput),
    Monitor(crate::output_dependencies::MonitorOutput),
    SchedulerCreate(crate::output_dependencies::SchedulerCreateOutput),
    SchedulerDelete(crate::output_dependencies::SchedulerDeleteOutput),
    SchedulerList(crate::output_dependencies::SchedulerListOutput),
    UpdateGoal(crate::output_dependencies::UpdateGoalOutput),
    Workflow(crate::output_dependencies::WorkflowToolOutput),
    /// Dynamic output for runtime-registered tools (MCP, test tools, etc.)
    Dynamic(DynamicOutput),
    /// Generic text output for tools that produce simple formatted text
    /// (e.g., memory_search, memory_get). The string is the pre-formatted
    /// prompt text — no additional rendering is needed.
    Text(TextOutput),
    #[from(skip)]
    ImageGen(MediaGenOutput),
    #[from(skip)]
    ImageToVideo(MediaGenOutput),
    #[from(skip)]
    ReferenceToVideo(MediaGenOutput),
    #[from(skip)]
    ImageEdit(MediaGenOutput),
}
impl From<BashToolOutput> for ToolOutput {
    fn from(o: BashToolOutput) -> Self {
        match o {
            BashToolOutput::Bash(b) => ToolOutput::Bash(b),
            BashToolOutput::BackgroundTaskStarted(b) => ToolOutput::BackgroundTaskStarted(b),
        }
    }
}

impl ToolOutput {
    /// Whether this output is a logical tool failure, for `tool.execution`'s
    /// `success`/`outcome`. Conservative: only known error variants count, so we
    /// never report a *false failure*.
    pub fn is_error(&self) -> bool {
        match self {
            ToolOutput::MCP(m) => m.is_error,
            ToolOutput::Bash(b) => b.exit_code != 0,
            ToolOutput::SearchReplace(SearchReplaceOutput::EditsApplied(_)) => false,
            ToolOutput::SearchReplace(_) => true,
            ToolOutput::ListDir(ListDirOutput::Content(_)) => false,
            ToolOutput::ListDir(_) => true,
            ToolOutput::ReadFile(
                ReadFileOutput::FileContent(_)
                | ReadFileOutput::ImageContent(_)
                | ReadFileOutput::PdfPageImages(_),
            ) => false,
            ToolOutput::ReadFile(_) => true,
            ToolOutput::TaskOutput(TaskOutputOutput::TaskNotFound(_)) => true,
            ToolOutput::KillTask(KillTaskOutput::TaskNotFound(_)) => true,
            ToolOutput::Skill(s) => !s.success,
            ToolOutput::WebFetch(WebFetchOutput::Content(_)) => false,
            ToolOutput::WebFetch(_) => true,
            ToolOutput::ApplyPatch(ApplyPatchOutput::Success { .. }) => false,
            ToolOutput::ApplyPatch(_) => true,
            ToolOutput::CodexGrepFiles(CodexGrepFilesOutput::Error(_)) => true,
            ToolOutput::SendSubagentMessage(output) => {
                matches!(
                    output.disposition(),
                    crate::output_dependencies::SendSubagentMessageDisposition::Rejected
                )
            }
            ToolOutput::Todo(
                TodoWriteOutput::DuplicateId(_) | TodoWriteOutput::InvalidArgument(_),
            ) => true,
            ToolOutput::GrepSearch(g) => g.exit_code > 1,
            _ => false,
        }
    }
    /// Render tool output for inclusion in the model prompt with specified format.
    pub fn to_prompt_format(&self) -> String {
        match self {
            ToolOutput::ReadFile(read_file_output) => match read_file_output {
                ReadFileOutput::FileContent(file_content) if file_content.content.is_empty() => {
                    if file_content.total_lines == 0 {
                        "File is empty.".to_string()
                    } else if file_content
                        .offset
                        .is_some_and(|offset| offset > file_content.total_lines)
                    {
                        format!(
                            "(no lines returned: the requested window is past the end of the \
                             file; the file has {} lines)",
                            file_content.total_lines
                        )
                    } else {
                        "(no lines returned)".to_string()
                    }
                }
                ReadFileOutput::FileContent(file_content) => file_content.content.clone(),
                ReadFileOutput::ImageContent(image_content) => {
                    format!(
                        "[Image content of type: {} is included inline in this tool result]",
                        image_content.mime_type,
                    )
                }
                ReadFileOutput::FileNotFound(error_msg)
                | ReadFileOutput::IsADirectory(error_msg)
                | ReadFileOutput::PermissionDenied(error_msg)
                | ReadFileOutput::FileTooLarge(error_msg)
                | ReadFileOutput::FileReadError(error_msg)
                | ReadFileOutput::ImageSizeError(error_msg) => error_msg.to_owned(),
                ReadFileOutput::PdfPageImages(pdf) => {
                    let page_list: Vec<String> = pdf
                        .pages
                        .iter()
                        .map(|p| p.page_number.to_string())
                        .collect();
                    format!(
                        "[Read PDF: {} pages rendered (pages {}). Total document: {} pages, {:.1} KB]",
                        pdf.pages.len(),
                        page_list.join(", "),
                        pdf.total_pages,
                        pdf.file_size as f64 / 1024.0,
                    )
                }
            },
            ToolOutput::ListDir(list_dir_output) => match list_dir_output {
                ListDirOutput::Content(content) => content.content.clone(),
                ListDirOutput::NotFound(error_msg)
                | ListDirOutput::IsAFile(error_msg)
                | ListDirOutput::NotADirectory(error_msg)
                | ListDirOutput::PermissionDenied(error_msg)
                | ListDirOutput::Error(error_msg) => error_msg.to_owned(),
            },
            ToolOutput::SearchReplace(search_replace_output) => match search_replace_output {
                SearchReplaceOutput::EditsApplied(edits_applied) => {
                    edits_applied.tool_output_for_prompt.to_owned()
                }
                SearchReplaceOutput::NoMatchesFound(e) => e.message.clone(),
                SearchReplaceOutput::FileAlreadyExists(error_string)
                | SearchReplaceOutput::MultipleMatchesFound(error_string)
                | SearchReplaceOutput::InvalidInput(error_string)
                | SearchReplaceOutput::FileNotFound(error_string)
                | SearchReplaceOutput::FilenameTooLong(error_string) => error_string.to_owned(),
            },
            ToolOutput::Bash(bash_output) => bash_output.output_for_prompt.clone(),
            ToolOutput::GrepSearch(grep_search_output) => {
                String::from_utf8_lossy(&grep_search_output.stdout).into_owned()
            }
            ToolOutput::Todo(todo_output) => match todo_output {
                TodoWriteOutput::TodosUpdated(success) => success.summary_for_prompt.to_owned(),
                TodoWriteOutput::DuplicateId(msg) => msg.to_owned(),
                TodoWriteOutput::InvalidArgument(msg) => msg.to_owned(),
            },
            ToolOutput::WebSearch(web_search_output) => {
                if let Some(ref pre) = web_search_output.pre_formatted {
                    pre.clone()
                } else {
                    format!(
                        "Web search results for: \"{}\"\n\n{}",
                        web_search_output.query, web_search_output.content
                    )
                }
            }
            ToolOutput::WebFetch(o) => o.to_prompt_format(),
            ToolOutput::MCP(mcp_output) => match &mcp_output.output {
                MCPOutputDetails::Error(error) => {
                    format!("Failed to call {}: {}", &mcp_output.tool_name, error)
                }
                MCPOutputDetails::OkayOutput(output) => output.to_owned(),
            },
            ToolOutput::BackgroundTaskStarted(bg) => {
                if let Some(body) = bg.pre_formatted.as_deref() {
                    body.to_string()
                } else {
                    format!(
                        "<task-id>{}</task-id>\n\
                         <task-type>{}</task-type>\n\
                         <output-file>{}</output-file>\n\
                         <status>{}</status>\n\
                         <summary>{}</summary>\n\
                         {}",
                        bg.task_id,
                        bg.task_type,
                        bg.output_file,
                        bg.status,
                        bg.summary,
                        bg.retrieval_hint
                    )
                }
            }
            ToolOutput::TaskOutput(task_output) => match task_output {
                TaskOutputOutput::Result(r) => {
                    let mut lines = vec![
                        format!("=== Task {} ===", r.task_id),
                        format!("Command: {}", r.command),
                        format!("Status: {}", r.status),
                        format!("Duration: {:.2}s", r.duration_secs),
                    ];
                    if let Some(code) = r.exit_code {
                        lines.push(format!("Exit Code: {}", code));
                    }
                    if !r.output_file.is_empty() {
                        lines.push(format!("Output File: {}", r.output_file));
                    }
                    lines.push(String::new());
                    lines.push("=== Output ===".to_string());
                    if r.output.is_empty() {
                        if r.status == "running" {
                            lines.push("(no output yet)".to_string());
                        } else {
                            lines.push("(no output)".to_string());
                        }
                    } else {
                        lines.push(r.output.clone());
                    }
                    if r.truncated {
                        lines.push(r.truncation_hint.clone());
                    }
                    lines.join("\n")
                }
                TaskOutputOutput::TaskNotFound(msg) => msg.to_owned(),
                TaskOutputOutput::MultiResult(mr) => {
                    let mut lines = vec![format!("=== Multi-wait ({}) ===", mr.mode)];
                    for r in &mr.results {
                        lines.push(format!(
                            "--- Task {} [{}] ---\nCommand: {}\nDuration: {:.2}s",
                            r.task_id, r.status, r.command, r.duration_secs,
                        ));
                        if let Some(code) = r.exit_code {
                            lines.push(format!("Exit Code: {code}"));
                        }
                        if !r.output.is_empty() {
                            lines.push(r.output.clone());
                        }
                    }
                    lines.push(format!("\n{}", mr.summary));
                    lines.join("\n")
                }
            },
            ToolOutput::KillTask(kill_output) => match kill_output {
                KillTaskOutput::Result(r) => format!("{}: {}", r.outcome, r.message),
                KillTaskOutput::TaskNotFound(msg) => msg.to_owned(),
            },
            ToolOutput::Skill(skill_output) => skill_output
                .skill_message
                .clone()
                .unwrap_or_else(|| skill_output.tool_result.clone()),
            ToolOutput::ApplyPatch(apply_patch_output) => match apply_patch_output {
                ApplyPatchOutput::Success {
                    tool_output_for_prompt,
                    ..
                } => tool_output_for_prompt.to_owned(),
                ApplyPatchOutput::ParseError(msg)
                | ApplyPatchOutput::ApplicationError(msg)
                | ApplyPatchOutput::EmptyPatch(msg) => msg.to_owned(),
            },
            ToolOutput::CodexGrepFiles(output) => match output {
                CodexGrepFilesOutput::Matches { content, .. } => content.clone(),
                CodexGrepFilesOutput::NoMatches(msg) | CodexGrepFilesOutput::Error(msg) => {
                    msg.clone()
                }
            },
            ToolOutput::SearchTool(out) => out.content.clone(),
            ToolOutput::SubagentCompleted(sub) => {
                let mut text = sub.output.clone();
                if let Some(ref wt) = sub.worktree_path {
                    text.push_str(&format!("\n\n<worktree_path>{wt}</worktree_path>"));
                }
                text.push_str("\n\n");
                text.push_str(&sub.resume_footer());
                text
            }
            ToolOutput::EnterPlanMode(EnterPlanModeOutput::Entered {
                message,
                plan_file_path,
                tool_hints,
                plan_file_seed,
            }) => {
                let ask = &tool_hints.ask_user;
                let exit = &tool_hints.exit_plan;
                let task_hint = if tool_hints.task.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n     You can use the {} tool with subagent_type=\"explore\" to \
                         parallelize codebase exploration without filling your context window.",
                        tool_hints.task
                    )
                };
                let plan_status = match plan_file_seed {
                    PlanFileSeedStatus::Empty => {
                        format!(
                            "Write your plan to {plan_file_path}. The file exists and is empty."
                        )
                    }
                    PlanFileSeedStatus::NonEmpty => {
                        format!(
                            "Write your plan to {plan_file_path}. The file exists but is not empty."
                        )
                    }
                    PlanFileSeedStatus::Missing(reason) => {
                        let detail = match reason {
                            PlanFileSeedFailure::NotCreated => "The file has not yet been created.",
                            PlanFileSeedFailure::NotAFile => {
                                "A directory already exists at that path."
                            }
                            PlanFileSeedFailure::Inaccessible => "The file could not be accessed.",
                            PlanFileSeedFailure::Unavailable => {
                                "The plan file location is unavailable."
                            }
                        };
                        format!("Write your plan to {plan_file_path}. {detail}")
                    }
                };
                format!(
                    "{message}\n\n\
                     {plan_status}\n\n\
                     In plan mode, you should:\n\
                     1. Thoroughly explore the codebase to understand existing patterns{task_hint}\n\
                     2. Identify similar features, codebase architecture, and understand trade-offs\n\
                     3. Use {ask} if you need to clarify the approach\n\
                     4. Design a concrete implementation strategy\n\
                     5. Write your plan to the plan file above\n\
                     6. When ready, use {exit} to present your plan to the user."
                )
            }
            ToolOutput::ExitPlanMode(exit) => match exit {
                ExitPlanModeOutput::PlanReady {
                    message,
                    plan_content,
                    plan_file_path,
                } => {
                    format!(
                        "{message}\n\nYour plan has been saved at: {plan_file_path}\n\n\
                         ## Plan:\n{plan_content}"
                    )
                }
                ExitPlanModeOutput::EmptyPlan { message, .. } => message.clone(),
            },
            ToolOutput::AskUserQuestion(
                AskUserQuestionOutput::QuestionsSent { message, .. }
                | AskUserQuestionOutput::UserAnswered { message },
            ) => message.clone(),
            ToolOutput::SendSubagentMessage(output) => output.to_string(),
            ToolOutput::Monitor(o) => {
                if o.persistent {
                    format!(
                        "Monitor started (task {}, persistent -- runs until kill_task or session end).\n\
                         You will be notified on each event. Keep working -- do not poll or sleep.\n\
                         Events may arrive while you are waiting for the user -- an event is not their reply.",
                        o.task_id
                    )
                } else {
                    format!(
                        "Monitor started (task {}, timeout {}ms).\n\
                         You will be notified on each event. Keep working -- do not poll or sleep.\n\
                         Events may arrive while you are waiting for the user -- an event is not their reply.",
                        o.task_id, o.timeout_ms
                    )
                }
            }
            ToolOutput::SchedulerCreate(o) => {
                let verb = if o.updated { "updated" } else { "created" };
                format!(
                    "Scheduled task {} (ID: {}, {}).",
                    verb, o.id, o.human_schedule
                )
            }
            ToolOutput::SchedulerDelete(o) => o.message.clone(),
            ToolOutput::SchedulerList(o) => {
                if o.tasks.is_empty() {
                    "No scheduled tasks.".into()
                } else {
                    serde_json::to_string_pretty(&o.tasks).unwrap_or_default()
                }
            }
            ToolOutput::UpdateGoal(o) => o.summary.clone(),
            ToolOutput::Workflow(o) => o.message.clone(),
            ToolOutput::Dynamic(v) => serde_json::to_string_pretty(&v.value).unwrap_or_default(),
            ToolOutput::Text(text) => text.text.clone(),
            ToolOutput::ImageGen(m) => m.prompt_text("Image generated"),
            ToolOutput::ImageToVideo(m) => m.prompt_text("Video generated"),
            ToolOutput::ReferenceToVideo(m) => m.prompt_text("Video generated"),
            ToolOutput::ImageEdit(m) => m.prompt_text("Image edited"),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TodoWriteSuccess {
    pub summary_for_prompt: String,
    pub todos: Vec<TodoItem>,
    /// Full state snapshot for consumer persistence/restoration.
    #[schemars(skip)]
    pub state: TodoState,
}
/// Output from the TodoWrite tool. Follows the error-as-output-variant pattern (like
/// `ReadFileOutput`, `SearchReplaceOutput`) so consumers (Python side, ACP layer) can distinguish
/// tool-logic errors from infrastructure errors.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum TodoWriteOutput {
    /// Successfully updated todo state.
    TodosUpdated(TodoWriteSuccess),
    /// Duplicate todo ID found in the input.
    DuplicateId(String),
    /// Argument validation failed (model-facing message is returned verbatim).
    /// Used so missing-field errors surface as the terse `Invalid argument: …`
    /// line, instead of the framework's wrapper around a `ToolError`.
    InvalidArgument(String),
}
/// Why the session plan file is not a ready (empty/non-empty) file.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum PlanFileSeedFailure {
    /// Did not exist and could not be created.
    #[default]
    NotCreated,
    /// A directory (or other non-regular file) occupies the path.
    NotAFile,
    /// Exists but could not be read (permission / other IO error).
    Inaccessible,
    /// No `FileSystem` resource or no absolute path was available to seed.
    Unavailable,
}
/// Result of probing / seeding the session plan file on `enter_plan_mode`. Defaults to
/// `Missing(NotCreated)` when the field is absent on older payloads (fail-closed in
/// `to_prompt_format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanFileSeedStatus {
    /// Not ready; carries why (see [`PlanFileSeedFailure`]).
    Missing(PlanFileSeedFailure),
    /// Present and empty (fresh seed or pre-existing empty).
    Empty,
    /// Present with prior content (re-entry; not truncated).
    NonEmpty,
}
impl Default for PlanFileSeedStatus {
    fn default() -> Self {
        Self::Missing(PlanFileSeedFailure::NotCreated)
    }
}
/// Output from the `EnterPlanMode` tool. Confirms plan mode entry and reports session plan-file
/// seed status. The tool may create an empty session plan file (never truncating non-empty
/// content); broader read-only enforcement is handled by orchestration.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum EnterPlanModeOutput {
    /// Successfully signaled plan mode entry.
    Entered {
        /// Confirmation message for the model, nudging it into
        /// exploration/planning behavior.
        message: String,
        /// Absolute or display path to the plan file so the model knows
        /// where to write its plan immediately.
        plan_file_path: String,
        /// Pre-resolved tool name hints for `to_prompt_format()`. Resolved at runtime via
        /// `TemplateRenderer` so no tool names are hardcoded. Falls back to canonical names when
        /// the renderer is unavailable.
        #[serde(default)]
        tool_hints: EnterPlanModeToolHints,
        /// Probe / seed outcome; defaults to `Missing` when absent.
        #[serde(default)]
        plan_file_seed: PlanFileSeedStatus,
    },
}
/// Pre-resolved tool name hints embedded in `EnterPlanModeOutput`.
///
/// Resolved at runtime so `to_prompt_format()` never hardcodes tool names.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EnterPlanModeToolHints {
    /// Client-facing name for `ask_user_question` (ToolKind::AskUser).
    #[serde(default = "EnterPlanModeToolHints::default_ask_user")]
    pub ask_user: String,
    /// Client-facing name for `exit_plan_mode` (ToolKind::ExitPlan).
    #[serde(default = "EnterPlanModeToolHints::default_exit_plan")]
    pub exit_plan: String,
    /// Client-facing name for the subagent `task` tool (ToolKind::Task).
    /// Empty when the task tool is not registered.
    #[serde(default)]
    pub task: String,
}
impl Default for EnterPlanModeToolHints {
    fn default() -> Self {
        Self {
            ask_user: "ask_user_question".to_owned(),
            exit_plan: "exit_plan_mode".to_owned(),
            task: String::new(),
        }
    }
}
impl EnterPlanModeToolHints {
    fn default_ask_user() -> String {
        "ask_user_question".to_owned()
    }
    fn default_exit_plan() -> String {
        "exit_plan_mode".to_owned()
    }
}
/// Output from the `AskUserQuestion` tool. This is a thin signal — the tool sends the questions to the client via a notification and returns a
/// confirmation. The actual answers come back from the client as the tool result (handled by the orchestration layer). Because the answers are
/// provided by the client asynchronously (the user interacts with a UI), the tool output here just confirms the questions were dispatched.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum AskUserQuestionOutput {
    /// Questions were successfully dispatched to the client for user input.
    /// Used during migration fallback when `UserQuestionSender` is not yet
    /// injected by the shell.
    QuestionsSent {
        /// Confirmation message for the model.
        message: String,
        /// Number of questions sent.
        question_count: usize,
    },
    /// The user has responded (or cancelled). The `message` is the fully-formatted tool result
    /// string produced by the format module. All four user paths (accepted, chat about this, skip
    /// interview, cancel) return this variant with `ToolCall` status `Completed`.
    UserAnswered {
        /// Pre-formatted tool result string for the model.
        message: String,
    },
}
/// Output from the `ExitPlanMode` tool. The tool reads the plan file from disk and surfaces its
/// content. The orchestration layer / client is responsible for presenting the plan to the user for
/// approval and determining the exit outcome.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ExitPlanModeOutput {
    /// Plan file had content — surfaced for approval.
    PlanReady {
        /// Confirmation message for the model.
        message: String,
        /// The plan file content read from disk.
        plan_content: String,
        /// Path to the plan file (for the model to reference later).
        plan_file_path: String,
    },
    /// Plan file was empty or did not exist.
    EmptyPlan {
        /// Message informing the model there was no plan content.
        message: String,
        /// Path where the plan file was expected.
        plan_file_path: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MCPOutputDetails {
    OkayOutput(String),
    Error(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPOutput {
    tool_name: String,
    server_name: String,
    output: MCPOutputDetails,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reconnect_attempted: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auth_retry_attempted: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_timeout: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
    /// Pre-truncation image captures for session harvest (same contract as
    /// [`FileContent::extracted_images`]). Must survive ToolDyn hub
    /// `to_value`/`from_value`; session drains before PostToolUse and ACP.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extracted_images: Vec<crate::output_dependencies::ExtractedImage>,
}
impl MCPOutput {
    pub fn okay_output(tool_name: String, server_name: String, output: String) -> Self {
        Self {
            tool_name,
            server_name,
            output: MCPOutputDetails::OkayOutput(output),
            reconnect_attempted: false,
            auth_retry_attempted: false,
            is_timeout: false,
            is_error: false,
            extracted_images: Vec::new(),
        }
    }
    pub fn errored(tool_name: String, server_name: String, error: String) -> Self {
        Self {
            tool_name,
            server_name,
            output: MCPOutputDetails::Error(error),
            reconnect_attempted: false,
            auth_retry_attempted: false,
            is_timeout: false,
            is_error: true,
            extracted_images: Vec::new(),
        }
    }
    pub fn output(&self) -> &MCPOutputDetails {
        &self.output
    }
    pub fn output_mut(&mut self) -> &mut MCPOutputDetails {
        &mut self.output
    }
}
