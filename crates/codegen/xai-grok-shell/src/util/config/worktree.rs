pub use xai_grok_shared::config::worktree::*;
use super::RemoteSettings;
use toml::Value as TomlValue;
pub fn worktree_type() -> WorktreeType {
    let root: TomlValue = match crate::config::load_effective_config() {
        Ok(r) => r,
        Err(_) => return WorktreeType::Linked,
    };
    worktree_type_from_toml(&root)
}

pub fn grove_worktree_enabled(remote: Option<&RemoteSettings>) -> bool {
    let root: TomlValue = match crate::config::load_effective_config() {
        Ok(r) => r,
        Err(_) => TomlValue::Table(toml::map::Map::new()),
    };
    gate_grove_worktree(None, &root, remote).0
}

/// Resolve `[worktree.auto_gc]` from parsed settings: env > local > remote > defaults (clamped).
/// Platform age policy is applied later in `maybe_auto_gc`.
/// (`xai-fast-worktree`'s `resolve_worktree_auto_gc_from_layers` owns and tests precedence and clamping; this only maps settings to layers.)
pub(crate) fn resolve_worktree_auto_gc_from_settings(
    local: Option<&super::WorktreeAutoGcSettings>,
    remote: Option<&super::WorktreeAutoGcSettings>,
) -> xai_fast_worktree::ResolvedWorktreeAutoGc {
    use xai_grok_workspace::worktree::worktree_auto_gc_layer_from_settings;
    let local_layer = local.map(worktree_auto_gc_layer_from_settings);
    let remote_layer = remote.map(worktree_auto_gc_layer_from_settings);
    xai_fast_worktree::resolve_worktree_auto_gc_from_layers(
        local_layer.as_ref(),
        remote_layer.as_ref(),
    )
}

#[cfg(test)]
mod tests {
    use super::RemoteSettings;
    use super::*;
    use serial_test::serial;
    use toml::Value as TomlValue;

    #[test]
    fn test_worktree_type_linked() {
        let toml_str = r#"
[cli]
worktree_type = "linked"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(worktree_type_from_toml(&root), WorktreeType::Linked);
    }

    #[test]
    fn test_worktree_type_standalone() {
        let toml_str = r#"
[cli]
worktree_type = "standalone"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(worktree_type_from_toml(&root), WorktreeType::Standalone);
    }

    #[test]
    fn test_worktree_type_git() {
        let toml_str = r#"
[cli]
worktree_type = "git"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(worktree_type_from_toml(&root), WorktreeType::Git);
    }

    #[test]
    fn test_worktree_type_default_linked() {
        let toml_str = r#"
[cli]
auto_update = true
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(worktree_type_from_toml(&root), WorktreeType::Linked);
    }

    #[test]
    fn test_worktree_type_no_cli_section() {
        let toml_str = r#"
[models]
default = "grok-code-fast-1"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(worktree_type_from_toml(&root), WorktreeType::Linked);
    }

    #[test]
    fn test_worktree_type_invalid_value() {
        let toml_str = r#"
[cli]
worktree_type = "invalid"
"#;
        let root: TomlValue = toml::from_str(toml_str).unwrap();
        assert_eq!(worktree_type_from_toml(&root), WorktreeType::Linked);
    }

    #[test]
    fn test_worktree_type_fromstr() {
        assert_eq!("linked".parse::<WorktreeType>(), Ok(WorktreeType::Linked));
        assert_eq!(
            "standalone".parse::<WorktreeType>(),
            Ok(WorktreeType::Standalone)
        );
        assert_eq!("git".parse::<WorktreeType>(), Ok(WorktreeType::Git));
        assert!("invalid".parse::<WorktreeType>().is_err());
        assert!("LINKED".parse::<WorktreeType>().is_err());
    }

    #[test]
    fn test_worktree_type_from_toml_opt_present() {
        let root: TomlValue = toml::from_str("[cli]\nworktree_type = \"standalone\"").unwrap();
        assert_eq!(
            worktree_type_from_toml_opt(&root),
            Some(WorktreeType::Standalone)
        );
    }

    #[test]
    fn test_worktree_type_from_toml_opt_absent() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        assert_eq!(worktree_type_from_toml_opt(&root), None);
    }

    #[test]
    fn test_worktree_type_from_toml_opt_invalid() {
        let root: TomlValue = toml::from_str("[cli]\nworktree_type = \"bogus\"").unwrap();
        assert_eq!(worktree_type_from_toml_opt(&root), None);
    }

    #[test]
    fn test_worktree_type_from_toml_opt_no_cli_section() {
        let root: TomlValue = toml::from_str("[models]\ndefault = \"grok\"").unwrap();
        assert_eq!(worktree_type_from_toml_opt(&root), None);
    }

    #[test]
    fn test_resolve_worktree_type_local_wins_over_remote() {
        let root: TomlValue = toml::from_str("[cli]\nworktree_type = \"git\"").unwrap();
        let remote = RemoteSettings {
            worktree_type: Some("standalone".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            resolve_worktree_type(&root, Some(&remote)),
            (WorktreeType::Git, "local")
        );
    }

    #[test]
    fn test_resolve_worktree_type_remote_fallback() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        let remote = RemoteSettings {
            worktree_type: Some("standalone".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            resolve_worktree_type(&root, Some(&remote)),
            (WorktreeType::Standalone, "remote")
        );
    }

    #[test]
    fn test_resolve_worktree_type_default_when_no_config() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        assert_eq!(
            resolve_worktree_type(&root, None),
            (WorktreeType::Linked, "default")
        );
    }

    #[test]
    fn test_resolve_worktree_type_invalid_remote_falls_back_to_default() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        let remote = RemoteSettings {
            worktree_type: Some("bogus".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            resolve_worktree_type(&root, Some(&remote)),
            (WorktreeType::Linked, "default")
        );
    }

    #[test]
    fn test_resolve_worktree_type_remote_none_field() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        let remote = RemoteSettings {
            worktree_type: None,
            ..Default::default()
        };
        assert_eq!(
            resolve_worktree_type(&root, Some(&remote)),
            (WorktreeType::Linked, "default")
        );
    }

    fn clear_worktree_type_env() {
        unsafe { std::env::remove_var(ENV_WORKTREE_TYPE) };
    }

    fn remote_unset() -> RemoteSettings {
        RemoteSettings {
            grove_worktree: None,
            ..Default::default()
        }
    }

    #[test]
    #[serial]
    fn resolve_grove_worktree_default_copy() {
        clear_worktree_type_env();
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote_unset())),
            (false, "default")
        );
        assert_eq!(resolve_grove_worktree(&root, None), (false, "default"));
    }

    #[test]
    #[serial]
    fn resolve_grove_worktree_toml_bool_and_type_spelling() {
        clear_worktree_type_env();
        let remote = remote_unset();
        let root: TomlValue = toml::from_str("[cli]\ngrove_worktree = true").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (true, "local")
        );
        let root: TomlValue = toml::from_str("[cli]\nnfs_worktree = true").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (true, "local")
        );
        let root: TomlValue = toml::from_str("[cli]\nworktree_type = \"grove\"").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (true, "local")
        );
        let root: TomlValue = toml::from_str("[cli]\nworktree_type = \"nfs\"").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (true, "local")
        );
        let root: TomlValue = toml::from_str("[cli]\nworktree_type = \"copy\"").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (false, "local")
        );
        let root: TomlValue = toml::from_str("[cli]\nworktree_type = \"linked\"").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (false, "default")
        );
        let root: TomlValue =
            toml::from_str("[cli]\ngrove_worktree = false\nnfs_worktree = true").unwrap();
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (false, "local")
        );
    }

    #[test]
    #[serial]
    fn resolve_grove_worktree_env_wins_over_local() {
        clear_worktree_type_env();
        let remote = remote_unset();
        unsafe { std::env::set_var(ENV_WORKTREE_TYPE, "grove") };
        let root: TomlValue = toml::from_str("[cli]\ngrove_worktree = false").unwrap();
        assert_eq!(resolve_grove_worktree(&root, Some(&remote)), (true, "env"));
        unsafe { std::env::set_var(ENV_WORKTREE_TYPE, "copy") };
        let root: TomlValue = toml::from_str("[cli]\ngrove_worktree = true").unwrap();
        assert_eq!(resolve_grove_worktree(&root, Some(&remote)), (false, "env"));
        clear_worktree_type_env();
    }

    #[test]
    fn gate_grove_worktree_layers_resume_parity_without_process_env() {
        let local_grove: TomlValue = toml::from_str("[cli]\ngrove_worktree = true").unwrap();
        let empty: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        let remote_unset = remote_unset();
        let remote_kill = RemoteSettings {
            grove_worktree: Some(false),
            ..Default::default()
        };

        assert_eq!(
            gate_grove_worktree_layers(None, None, &local_grove, Some(&remote_unset)),
            (true, "local"),
            "remote Some(unset) + local grove must enable, same as create"
        );
        assert_eq!(
            gate_grove_worktree_layers(None, Some(true), &empty, Some(&remote_unset)),
            (true, "env"),
            "remote Some(unset) + env grove must enable, same as create"
        );
        assert_eq!(
            gate_grove_worktree_layers(None, Some(true), &empty, Some(&remote_kill)),
            (false, "remote_kill"),
            "remote kill must disable even when env asked for grove"
        );
        assert_eq!(
            gate_grove_worktree_layers(None, Some(true), &empty, None),
            (false, "remote_unavailable"),
            "true unavailability must fail-close even if env asked for grove"
        );
        assert_eq!(
            gate_grove_worktree_layers(None, None, &empty, None),
            (false, "default"),
            "a missing remote must not be reported as a refused grove request"
        );
        assert_eq!(
            gate_grove_worktree_layers(Some(false), None, &local_grove, Some(&remote_kill)),
            (false, "request"),
            "the kill switch must not claim a request that asked for copy"
        );
    }

    #[test]
    #[serial]
    fn gate_grove_worktree_kill_switch_wins_over_request() {
        clear_worktree_type_env();
        let root: TomlValue = toml::from_str("[cli]\ngrove_worktree = true").unwrap();
        let remote = RemoteSettings {
            grove_worktree: Some(false),
            ..Default::default()
        };
        assert_eq!(
            gate_grove_worktree(Some(true), &root, Some(&remote)),
            (false, "remote_kill")
        );
        unsafe { std::env::set_var(ENV_WORKTREE_TYPE, "grove") };
        assert_eq!(
            gate_grove_worktree(Some(true), &root, None),
            (false, "remote_unavailable")
        );
        clear_worktree_type_env();
    }

    #[test]
    #[serial]
    fn resolve_grove_worktree_remote_kill_switch_wins() {
        clear_worktree_type_env();
        unsafe { std::env::set_var(ENV_WORKTREE_TYPE, "grove") };
        let root: TomlValue = toml::from_str("[cli]\ngrove_worktree = true").unwrap();
        let remote = RemoteSettings {
            grove_worktree: Some(false),
            ..Default::default()
        };
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (false, "remote_kill")
        );
        clear_worktree_type_env();
    }

    #[test]
    #[serial]
    fn resolve_grove_worktree_remote_true_when_unset() {
        clear_worktree_type_env();
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        let remote = RemoteSettings {
            grove_worktree: Some(true),
            ..Default::default()
        };
        assert_eq!(
            resolve_grove_worktree(&root, Some(&remote)),
            (true, "remote")
        );
    }

    #[test]
    fn remote_settings_deserializes_nfs_worktree_alias() {
        let s: RemoteSettings = serde_json::from_str(r#"{"nfs_worktree":false}"#).unwrap();
        assert_eq!(s.grove_worktree, Some(false));
        let s: RemoteSettings = serde_json::from_str(r#"{"grove_worktree":true}"#).unwrap();
        assert_eq!(s.grove_worktree, Some(true));
    }

    // === restore_code config tests ===

    #[test]
    fn test_restore_code_from_toml_present_true() {
        let root: TomlValue = toml::from_str("[cli]\nrestore_code = true").unwrap();
        assert_eq!(restore_code_from_toml(&root), Some(true));
    }

    #[test]
    fn test_restore_code_from_toml_present_false() {
        let root: TomlValue = toml::from_str("[cli]\nrestore_code = false").unwrap();
        assert_eq!(restore_code_from_toml(&root), Some(false));
    }

    #[test]
    fn test_restore_code_from_toml_absent() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        assert_eq!(restore_code_from_toml(&root), None);
    }

    #[test]
    fn test_restore_code_from_toml_no_cli_section() {
        let root: TomlValue = toml::from_str("[models]\ndefault = \"grok\"").unwrap();
        assert_eq!(restore_code_from_toml(&root), None);
    }

    #[test]
    fn test_restore_code_from_toml_wrong_type() {
        let root: TomlValue = toml::from_str("[cli]\nrestore_code = \"yes\"").unwrap();
        assert_eq!(restore_code_from_toml(&root), None);
    }

    #[test]
    fn test_resolve_restore_code_local_wins_over_remote() {
        let root: TomlValue = toml::from_str("[cli]\nrestore_code = true").unwrap();
        let remote = RemoteSettings {
            restore_code: Some(false),
            ..Default::default()
        };
        assert!(resolve_restore_code(&root, Some(&remote)));
    }

    #[test]
    fn test_resolve_restore_code_remote_fallback() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        let remote = RemoteSettings {
            restore_code: Some(true),
            ..Default::default()
        };
        assert!(resolve_restore_code(&root, Some(&remote)));
    }

    #[test]
    fn test_resolve_restore_code_default_false() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        assert!(!resolve_restore_code(&root, None));
    }

    #[test]
    fn test_resolve_restore_code_remote_none_falls_to_default() {
        let root: TomlValue = toml::from_str("[cli]\nauto_update = true").unwrap();
        let remote = RemoteSettings {
            restore_code: None,
            ..Default::default()
        };
        assert!(!resolve_restore_code(&root, Some(&remote)));
    }

    #[test]
    fn test_resolve_restore_code_local_false_overrides_remote_true() {
        let root: TomlValue = toml::from_str("[cli]\nrestore_code = false").unwrap();
        let remote = RemoteSettings {
            restore_code: Some(true),
            ..Default::default()
        };
        assert!(!resolve_restore_code(&root, Some(&remote)));
    }
}
