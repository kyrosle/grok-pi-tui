//! Recompute host admission from Pi's public resolved-resource snapshot.
use anyhow::{Context, Result, bail};
use pi_grok_adapter::ResourceAdmissionPlanner;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};
use xai_grok_pager::{
    pi_resource_config::{
        PiProjectOverride, PiResource, PiResourceCatalog, PiResourceOrigin, PiResourceScope,
        PiResourceType,
    },
    pi_resource_policy::ResourcePolicy,
};

fn identity(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn resource_origin(metadata: &Value) -> PiResourceOrigin {
    if metadata["origin"] == "package" {
        PiResourceOrigin::Package
    } else if metadata["source"] == "auto" {
        PiResourceOrigin::Auto
    } else {
        PiResourceOrigin::Settings
    }
}

fn catalog(snapshot: &Value) -> Result<PiResourceCatalog> {
    let cwd = PathBuf::from(
        snapshot["cwd"]
            .as_str()
            .context("Pi resource snapshot cwd missing")?,
    );
    let trusted = snapshot["projectTrusted"]
        .as_bool()
        .context("Pi resource snapshot trust missing")?;
    let mut catalog = PiResourceCatalog::load_with_trust(cwd.clone(), Some(trusted))?;
    // SDK resolve owns package/settings identity, relative bases, scope and
    // filters. Keep only native filesystem auto discovery from the old reader.
    catalog
        .resources
        .retain(|resource| resource.origin == PiResourceOrigin::Auto);
    for (key, kind) in [
        ("extensions", PiResourceType::Extensions),
        ("skills", PiResourceType::Skills),
        ("prompts", PiResourceType::Prompts),
        ("themes", PiResourceType::Themes),
    ] {
        let paths = snapshot["resolvedPaths"][key]
            .as_array()
            .context("Pi resolved resource paths missing")?;
        for value in paths {
            let path = PathBuf::from(value["path"].as_str().context("Pi resource path missing")?);
            let meta = &value["metadata"];
            let scope = match meta["scope"].as_str() {
                Some("user") => PiResourceScope::User,
                Some("project") if trusted => PiResourceScope::Project,
                Some("project") => continue,
                _ => bail!("Unexpected resolved Pi resource scope"),
            };
            let enabled = value["enabled"]
                .as_bool()
                .context("Pi resource enabled missing")?;
            let resource = PiResource {
                path: path.clone(),
                resource_type: kind,
                scope,
                origin: resource_origin(meta),
                source: meta["source"]
                    .as_str()
                    .context("Pi resource source missing")?
                    .to_owned(),
                base_dir: meta["baseDir"]
                    .as_str()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| cwd.clone()),
                enabled,
                inherited_enabled: enabled,
                project_override: PiProjectOverride::Inherit,
            };
            catalog.resources.retain(|existing| {
                existing.resource_type != kind || identity(&existing.path) != identity(&path)
            });
            catalog.resources.push(resource);
        }
    }
    catalog.normalize_resource_order();
    Ok(catalog)
}

/// Remove only the last matching generated groups. Earlier user-explicit
/// duplicates survive. Keep the original insertion position before host tools.
fn remove_managed(mut args: Vec<String>, managed: &[String]) -> (Vec<String>, usize) {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < managed.len() {
        let end = i + if managed[i].starts_with("--no-") {
            1
        } else {
            2
        };
        groups.push(&managed[i..end]);
        i = end;
    }
    let mut at = args.len();
    for group in groups.into_iter().rev() {
        if let Some(index) = args
            .windows(group.len())
            .rposition(|candidate| candidate == group)
        {
            args.drain(index..index + group.len());
            at = at.min(index);
        }
    }
    (args, at)
}

pub fn planner(
    healed_args: Vec<String>,
    managed: Vec<String>,
    policy: ResourcePolicy,
    auto_extensions: bool,
    auto_prompts: bool,
    auto_themes: bool,
    skip_herdr: bool,
) -> ResourceAdmissionPlanner {
    let (base, insertion) = remove_managed(healed_args, &managed);
    Rc::new(move |snapshot| {
        for key in ["settingsErrors", "resolutionErrors"] {
            if snapshot
                .get(key)
                .and_then(Value::as_array)
                .is_some_and(|errors| !errors.is_empty())
            {
                bail!("Pi resource resolution contains errors; preserve the running session");
            }
        }
        let catalog = catalog(snapshot)?;
        let mut current = ResourcePolicy::load_for_cwd(&catalog.cwd);
        current.enabled_native_features = policy.enabled_native_features.clone();
        current.forced_native_features = policy.forced_native_features.clone();
        let plan = current.evaluate(&catalog);
        let mut managed = Vec::new();
        for (enabled, disable, flag, paths) in [
            (
                auto_extensions,
                "--no-extensions",
                "--extension",
                &plan.extensions,
            ),
            (
                auto_prompts,
                "--no-prompt-templates",
                "--prompt-template",
                &plan.prompts,
            ),
            (auto_themes, "--no-themes", "--theme", &plan.themes),
        ] {
            if !enabled {
                continue;
            }
            managed.push(disable.to_owned());
            for path in paths {
                if flag == "--extension"
                    && skip_herdr
                    && crate::herdr_extension::is_managed_pi_integration(path)
                {
                    continue;
                }
                managed.extend([flag.to_owned(), path.to_string_lossy().into_owned()]);
            }
        }
        let mut args = base.clone();
        args.splice(insertion..insertion, managed);
        Ok(args)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sdk_auto_metadata_preserves_settings_precedence_without_reading_home() {
        let auto = json!({"origin":"top-level","source":"auto"});
        let settings = json!({"origin":"top-level","source":"settings"});
        assert_eq!(resource_origin(&auto), PiResourceOrigin::Auto);
        assert_eq!(resource_origin(&settings), PiResourceOrigin::Settings);
        assert_eq!(
            resource_origin(&json!({"origin":"package","source":"auto"})),
            PiResourceOrigin::Package
        );
        let path = PathBuf::from("/__pi_resource_fixture__/extension.ts");
        let resource = |metadata: &Value, enabled| PiResource {
            path: path.clone(),
            resource_type: PiResourceType::Extensions,
            scope: PiResourceScope::User,
            origin: resource_origin(metadata),
            source: metadata["source"].as_str().unwrap().into(),
            base_dir: PathBuf::from("/__pi_resource_fixture__"),
            enabled,
            inherited_enabled: enabled,
            project_override: PiProjectOverride::Inherit,
        };
        let mut catalog = PiResourceCatalog {
            resources: vec![resource(&auto, true), resource(&settings, false)],
            project_trusted: false,
            agent_dir: PathBuf::from("/__pi_resource_fixture__/agent"),
            cwd: PathBuf::from("/__pi_resource_fixture__"),
        };
        catalog.normalize_resource_order();
        assert_eq!(catalog.resources.len(), 1);
        assert_eq!(catalog.resources[0].origin, PiResourceOrigin::Settings);
        assert!(
            !catalog.resources[0].enabled,
            "explicit settings must retain their override over auto discovery"
        );
    }
    #[test]
    fn removes_only_generated_groups_preserving_explicit_duplicates_and_host_order() {
        let args = [
            "--extension",
            "user.ts",
            "--extension",
            "same.ts",
            "--no-extensions",
            "--extension",
            "same.ts",
            "--extension",
            "host.ts",
        ]
        .map(str::to_owned)
        .to_vec();
        let managed = ["--no-extensions", "--extension", "same.ts"].map(str::to_owned);
        let (base, at) = remove_managed(args, &managed);
        assert_eq!(
            base,
            [
                "--extension",
                "user.ts",
                "--extension",
                "same.ts",
                "--extension",
                "host.ts"
            ]
        );
        assert_eq!(at, 4);
    }
}
