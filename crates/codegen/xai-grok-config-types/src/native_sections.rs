//! Full settings section DTOs, retained from their original producers.
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use xai_grok_sampling_types::ReasoningEffort;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CliConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dismissed_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_registry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_leader: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_tips: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_registry: Option<bool>,
    /// Env `GROK_MINIMUM_VERSION`.
    /// See [`crate::util::config::VersionPolicy`] for the version-policy knobs.
    /// (Unrelated to `version_overrides[].maximum_version`, which gates config patches.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_version: Option<String>,
    /// Env `GROK_MAXIMUM_VERSION`. See [`crate::util::config::VersionPolicy`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_version: Option<String>,
    /// Env `GROK_REQUIRED_MINIMUM_VERSION`. See [`crate::util::config::VersionPolicy`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_minimum_version: Option<String>,
    /// Env `GROK_REQUIRED_MAXIMUM_VERSION`. See [`crate::util::config::VersionPolicy`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_maximum_version: Option<String>,
    /// Group sessions by repo in the picker and CLI listings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_picker_grouped: Option<bool>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnosticsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash_handler: Option<bool>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// The pre-campaign `models.default` (merged user/managed/requirements), captured when a campaign is overriding the default.
    /// Model resolution recovers to it if the campaign points at a model missing from the catalog.
    /// `None` when there is nothing to recover to. Runtime-only; never serialized.
    #[serde(skip)]
    pub pre_campaign_default: Option<String>,
    /// Whether an active campaign is currently overriding `models.default`.
    /// The authoritative campaign-driven-default signal (set from the resolved active set), correct even when the user has no base default.
    /// Runtime-only.
    #[serde(skip)]
    pub default_is_campaign_driven: bool,
    /// Persisted effort for the default model; applied in `resolve_model_catalog`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_summary: Option<String>,
    /// Vision model used to transcribe user-supplied images via a separate endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_description: Option<String>,
    /// Model pin for next-prompt suggestions (tab-autocomplete ghost text).
    /// When unset: the remote pin, then the client hint / built-in `grok-4.6` default with the catalog guard; see `ModelOverrideConfig::resolve`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_suggestion: Option<String>,
    /// Restricts which models are user-selectable for normal chat (picker, `/model`, `-m`).
    /// Non-matching models stay in the catalog but are never shown, defaulted to, or selectable.
    /// Special/internal models (web_search, image_description, subagents, fork secondary) are exempt. User-config globs (`*`, `?`, `[...]`) match the catalog key or model id, case-sensitive. Empty = no restriction; an excluded explicit `default`/`-m` is rejected once the model catalog is fetched. Fleet pins live on [`Requirements::allowed_models`] and replace this list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_models: Option<Vec<String>>,
    /// Force `hidden = true` on these model IDs (still usable via `-m`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden_models: Option<Vec<String>>,
    /// Remove these model IDs from the catalog entirely. Wins over `hidden_models`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_models: Option<Vec<String>>,
    /// Fallback `agent_type` for models without a per-model override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    /// Global default request headers applied to every model.
    /// A per-model `[model.<id>].extra_headers` entry overrides per key (case-insensitive).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub extra_headers: IndexMap<String, String>,
    /// Global default values applied to every model that leaves the field unset; a per-model `[model.<id>]` value always wins.
    /// A deliberately small, allow-listed subset of the per-model fields (only `Option` ones, so "unset" is unambiguous).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_retry_threshold: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_idle_timeout_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_rate_limit_max_attempts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_tool_calls: Option<bool>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HarnessConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_for_uploads: Option<bool>,
    /// Deprecated; a real field, not a serde alias, because an alias rejects configs setting both keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_for_upload: Option<bool>,
    /// Budget (seconds) for the turn-end upload flush when `wait_for_uploads` is active.
    /// Default 60.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_flush_timeout_secs: Option<u64>,
}
impl HarnessConfig {
    pub fn merge_deprecated_keys(&mut self) {
        self.wait_for_uploads = self.wait_for_uploads.or(self.block_for_upload.take());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SessionConfig {
    /// Context window usage percentage (0-100) at which auto-compact is triggered. `None` means the user didn't set it.
    /// The resolver in `crate::util::config::resolve_auto_compact_threshold_percent` falls through to remote tiers and then the hardcoded default 85.
    /// Read this field via the resolver, not directly, to honor the full precedence chain (env, per-model, remote, default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_threshold_percent: Option<u8>,
    /// When enabled, the session will parse .envrc in the workspace directory and inject the environment variables into bash commands.
    /// Defaults to `true` when unset.
    /// `Option<bool>` so `None` round-trips as absent on disk (managed config wins over default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_envrc: Option<bool>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoModeConfig {
    /// The Auto-mode gate.
    /// Lowest-precedence layer of the gate chain (env and local `[auto_mode] enabled` config win over this remote value).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// How much context the classifier prompt includes.
    /// `None` means the wire fn's built-in default (`just_command`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_type: Option<ClassifierPromptType>,
    /// Routing slug for a dedicated classifier model.
    /// `None` inherits the session model.
    /// Resolved via `resolve_aux_model_sampling_config`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classifier_model: Option<String>,
    /// Classifier side-query duration in milliseconds; resolved with bounded defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classify_timeout_ms: Option<u64>,
    /// Classifier reasoning effort. Applies on BOTH the routed-model path and the inherited session-model path.
    /// `None` means the wire fn's built-in default (`low` if the effective model supports reasoning effort, else unset).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SkillsConfig {
    /// Additional skill locations to load.
    /// Each entry is a `SKILL.md` file or a directory walked recursively.
    /// Supports `~` expansion.
    #[serde(default)]
    pub paths: Vec<String>,

    /// Path prefixes to exclude.
    /// Any skill whose resolved path starts with one of these entries is filtered out.
    /// Supports `~` expansion.
    #[serde(default)]
    pub ignore: Vec<String>,

    /// Skill names that are disabled.
    /// Disabled skills remain in the list (unlike `ignore` which hides them entirely).
    /// They are excluded from the system prompt and skill tool invocation.
    #[serde(default)]
    pub disabled: Vec<String>,

    /// Skill dirs the launcher injects after syncing from the server (tagged `Server` scope).
    #[serde(default)]
    pub server_skill_dirs: Vec<String>,

    /// Skill dirs the launcher injects for skills bundled with the platform (tagged `Bundled` scope).
    #[serde(default)]
    pub bundled_skill_dirs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AskUserQuestionToolConfig {
    /// Whether the questionnaire timeout is enabled (default: `true`).
    /// `false` waits forever for answers.
    pub timeout_enabled: Option<bool>,
    /// Wait budget in seconds when the timer is enabled (positive integer; default: 1800 / 30 minutes).
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassifierPromptType {
    #[default]
    Full,
    NoUserToolPrefix,
    BareInstructions,
    JustCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Prompt the user for each tool call (default).
    #[default]
    Ask,
    /// Approve everything without prompting.
    AlwaysApprove,
    /// LLM transcript classifier reviews non-fast-path tool calls.
    Auto,
}

impl PermissionMode {
    pub fn is_always_approve(self) -> bool {
        matches!(self, Self::AlwaysApprove)
    }

    pub fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }

    pub fn from_yolo(yolo: bool) -> Self {
        if yolo { Self::AlwaysApprove } else { Self::Ask }
    }
}

/// Mirrors the internal `CreationMode` enum from xai-fast-worktree but uses config-friendly naming (lowercase strings in TOML).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeType {
    /// Linked worktree via `git worktree add --no-checkout` and a parallel CoW copy.
    /// This is the fastest mode for large repos.
    #[default]
    Linked,
    /// Standalone repository copy with independent `.git/` directory.
    /// Can be promoted to replace the source via `rename()`.
    Standalone,
    /// Plain `git worktree add` with full checkout.
    Git,
}

impl std::str::FromStr for WorktreeType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "linked" => Ok(Self::Linked),
            "standalone" => Ok(Self::Standalone),
            "git" => Ok(Self::Git),
            _ => Err(()),
        }
    }
}
