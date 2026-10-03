//! Canonical session-list wire facets and selection policy, without storage.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    strum::AsRefStr,
    strum::IntoStaticStr,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "snake_case")]
pub enum SessionKind {
    #[default]
    Build,
    Chat,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FacetValue {
    One(serde_json::Value),
    Many(Vec<serde_json::Value>),
}

impl FacetValue {
    pub fn values(&self) -> Vec<&serde_json::Value> {
        match self {
            FacetValue::One(v) => vec![v],
            FacetValue::Many(vs) => vs.iter().collect(),
        }
    }

    pub fn intersects(&self, allowed: &[serde_json::Value]) -> bool {
        self.values().into_iter().any(|v| allowed.contains(v))
    }
}

pub type FacetMap = BTreeMap<String, FacetValue>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetaEnvelope {
    pub kind: SessionKind,
    #[serde(default, skip_serializing_if = "FacetMap::is_empty")]
    pub facets: FacetMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, strum::AsRefStr, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ListScope {
    /// Scoped to the request cwd.
    #[default]
    Cwd,
    /// Relaxed to the cwd's repo when the cwd itself had no sessions.
    Repo,
    /// Relaxed to all directories when the cwd is not a git repo.
    All,
}
impl ListScope {
    /// True when the scope relaxed past the cwd, to the repo or to all directories.
    pub const fn is_relaxed(self) -> bool {
        !matches!(self, Self::Cwd)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum HeadlessPolicy {
    #[default]
    Exclude,
    Only,
    Include,
}
impl HeadlessPolicy {
    /// Missing values keep the wire behavior from before this policy existed: include everything.
    /// Unknown explicit values fail closed to [`Self::Exclude`].
    pub fn from_wire(value: Option<&str>) -> Self {
        match value {
            None | Some("include") => Self::Include,
            Some("exclude") => Self::Exclude,
            Some("only") => Self::Only,
            Some(_) => Self::Exclude,
        }
    }

    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Exclude => "exclude",
            Self::Only => "only",
            Self::Include => "include",
        }
    }

    pub const fn admits(self, is_headless: bool) -> bool {
        match self {
            Self::Exclude => !is_headless,
            Self::Only => is_headless,
            Self::Include => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentSessionSelection {
    Interactive,
    Any,
}

impl RecentSessionSelection {
    pub fn from_headless_policy(policy: HeadlessPolicy) -> Self {
        match policy {
            HeadlessPolicy::Exclude => Self::Interactive,
            HeadlessPolicy::Only | HeadlessPolicy::Include => Self::Any,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSessionHit {
    pub session_id: String,
    pub cwd: String,
    /// Session title/summary for display
    pub summary: String,
    /// RFC 3339 formatted updated_at
    pub updated_at: String,
    pub score: f32,
    pub matched_fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}
