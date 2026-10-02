//! Stock-session policy for the shared workflow registry.
use std::path::{Path, PathBuf};
pub use xai_workflow::registry::WorkflowListing;
pub(crate) use xai_workflow::registry::{
    BUILTIN_WORKFLOWS, MAX_WORKFLOW_SOURCE_BYTES, ResolveError, ResolvedWorkflow, WorkflowSource,
    resolve_inline, validate_workflow_name, warm_builtin_cache,
};
pub(crate) fn project_root(cwd: &Path) -> PathBuf {
    xai_grok_workspace::session::git::find_git_root_from_path(cwd)
        .unwrap_or_else(|_| cwd.to_path_buf())
}
pub(crate) fn user_workflow_dir() -> PathBuf {
    crate::util::grok_home::grok_home().join("workflows")
}
pub(crate) fn bundled_workflow_dir() -> PathBuf {
    crate::util::grok_home::grok_home().join("bundled/workflows")
}
pub(crate) fn stock_config(cwd: Option<&Path>) -> xai_workflow::registry::WorkflowRegistryConfig {
    let root = cwd.map(project_root);
    xai_workflow::registry::WorkflowRegistryConfig {
        project_workflow_dir: root
            .as_ref()
            .map(|root| xai_grok_config::project_config_dir(root).join("workflows")),
        project_root: root,
        project_allowed: cwd.is_some_and(crate::agent::folder_trust::project_scope_allowed),
        user_workflow_dir: Some(user_workflow_dir()),
        bundled_workflow_dir: Some(bundled_workflow_dir()),
        bundled_file_is_managed: std::sync::Arc::new(bundled_file_is_managed),
    }
}
/// True only while the file is byte-identical to what the GCS bundle update wrote; an edited copy loses builtin privilege.
fn bundled_file_is_managed(path: &Path) -> bool {
    let Some(workflows_dir) = path.parent() else {
        return false;
    };
    if workflows_dir.file_name().and_then(|name| name.to_str()) != Some("workflows") {
        return false;
    }
    let Some(root) = workflows_dir.parent() else {
        return false;
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    crate::bundle::is_managed_bundle_file(root, &relative)
}

pub(crate) struct WorkflowRegistry(xai_workflow::registry::WorkflowRegistry);
impl std::ops::Deref for WorkflowRegistry {
    type Target = xai_workflow::registry::WorkflowRegistry;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl WorkflowRegistry {
    pub(crate) fn scan(cwd: Option<&Path>) -> Self {
        Self(xai_workflow::registry::WorkflowRegistry::scan(
            &stock_config(cwd),
        ))
    }
}
pub(crate) fn resolve_by_name(
    name: &str,
    cwd: Option<&Path>,
) -> Result<ResolvedWorkflow, ResolveError> {
    WorkflowRegistry::scan(cwd).resolve_by_name(name)
}
pub(crate) fn resolve_by_path(
    path: &Path,
    cwd: &Path,
    session_dir: Option<&Path>,
) -> Result<ResolvedWorkflow, ResolveError> {
    xai_workflow::registry::resolve_by_path(path, cwd, session_dir, &stock_config(Some(cwd)))
}
pub(crate) fn save_project_workflow(
    cwd: &Path,
    name: &str,
    script: &str,
) -> Result<PathBuf, ResolveError> {
    xai_workflow::registry::save_project_workflow(cwd, name, script, &stock_config(Some(cwd)))
}
pub fn list_workflows(cwd: Option<&Path>) -> Vec<WorkflowListing> {
    WorkflowRegistry::scan(cwd).list()
}
pub(crate) fn workflow_snapshot(cwd: Option<&Path>) -> (WorkflowRegistry, Vec<WorkflowListing>) {
    let registry = WorkflowRegistry::scan(cwd);
    let listings = registry.list();
    (registry, listings)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn script(name: &str) -> String {
        format!("let meta = #{{ name: \"{name}\", description: \"d\" }}; complete(\"ok\");")
    }
    #[test]
    fn project_workflows_follow_folder_trust() {
        let dir = tempfile::tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        let workflows = dir.path().join(".grok/workflows");
        std::fs::create_dir_all(&workflows).unwrap();
        std::fs::write(workflows.join("project-only.rhai"), script("project-only")).unwrap();

        crate::agent::folder_trust::record_for_test(dir.path(), false);
        let untrusted = WorkflowRegistry::scan(Some(dir.path()));
        assert!(
            untrusted
                .list()
                .iter()
                .all(|listing| listing.name != "project-only")
        );
        assert!(matches!(
            untrusted.resolve_by_name("project-only"),
            Err(ResolveError::UnknownName(_))
        ));

        crate::agent::folder_trust::record_for_test(dir.path(), true);
        let trusted = WorkflowRegistry::scan(Some(dir.path()));
        assert_eq!(
            trusted.resolve_by_name("project-only").unwrap().meta.name,
            "project-only"
        );
    }

    #[test]
    fn managed_bundled_override_keeps_builtin_privileges() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("bundled");
        let workflows = root.join("workflows");
        std::fs::create_dir_all(&workflows).unwrap();
        let path = workflows.join("deep-research.rhai");
        let script = "let meta = #{ name: \"deep-research\", description: \"from-bundle\" };\ncomplete(\"ok\");";
        std::fs::write(&path, script).unwrap();
        let checksum = crate::bundle::checksum_file(&path).unwrap();
        let manifest = serde_json::json!({
            "version": "test",
            "checksums": { "workflows/deep-research.rhai": checksum },
        });
        std::fs::write(root.join("manifest.json"), manifest.to_string()).unwrap();

        let config = xai_workflow::registry::WorkflowRegistryConfig {
            bundled_workflow_dir: Some(workflows),
            bundled_file_is_managed: std::sync::Arc::new(bundled_file_is_managed),
            ..Default::default()
        };
        let registry = xai_workflow::registry::WorkflowRegistry::scan(&config);
        let entries = registry.list();
        let hit = entries
            .iter()
            .find(|entry| entry.name == "deep-research")
            .expect("deep-research");
        assert_eq!(hit.source, "builtin");
        assert_eq!(
            registry.resolve_by_name("deep-research").unwrap().source,
            WorkflowSource::Builtin
        );
        assert_eq!(hit.description, "from-bundle");
    }
}
