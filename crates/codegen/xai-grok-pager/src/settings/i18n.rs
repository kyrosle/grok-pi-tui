//! Bilingual copy for settings only. Persistence always uses the original keys and values.

use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    ZhCn,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    /// OS detection runs at most once, never once per frame.
    pub fn system() -> Self {
        static SYSTEM: LazyLock<Language> = LazyLock::new(detect_system_language);
        *SYSTEM
    }
}

pub fn canonical_language(preference: &str) -> &'static str {
    match preference {
        "en" => "en",
        "zh-CN" => "zh-CN",
        _ => "auto",
    }
}

pub fn resolve_language(preference: &str) -> Language {
    match canonical_language(preference) {
        "en" => Language::En,
        "zh-CN" => Language::ZhCn,
        _ => Language::system(),
    }
}

/// Known UI copy is translated; dynamic model/theme names remain untouched.
pub fn translate(language: Language, source: &str) -> &str {
    static CHINESE: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
        serde_json::from_str(include_str!("translations.json"))
            .expect("settings translations must be valid JSON")
    });
    if language == Language::ZhCn {
        CHINESE.get(source).map(String::as_str).unwrap_or(source)
    } else {
        source
    }
}

/// `C`, `POSIX`, and UTF-8-only terminal locales carry no human language.
fn language_from_locale(locale: &str) -> Option<Language> {
    let locale = locale.trim().to_ascii_lowercase();
    let base = locale.split(['_', '-', '.', '@']).next().unwrap_or("");
    match base {
        "" | "c" | "posix" | "utf" => None,
        "zh" => Some(Language::ZhCn),
        _ => Some(Language::En),
    }
}

fn preferred_language<'a>(locales: impl IntoIterator<Item = &'a str>) -> Option<Language> {
    // The first nonempty locale wins, including C/POSIX (which requests OS fallback).
    locales
        .into_iter()
        .find(|locale| !locale.trim().is_empty())
        .and_then(language_from_locale)
}

fn detect_system_language() -> Language {
    let locales =
        ["LC_ALL", "LC_MESSAGES", "LANG"].map(|key| std::env::var(key).unwrap_or_default());
    preferred_language(locales.iter().map(String::as_str))
        .or_else(platform_language)
        .unwrap_or(Language::En)
}

#[cfg(target_os = "macos")]
fn platform_language() -> Option<Language> {
    let output = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8(output.stdout).ok()?;
    output
        .lines()
        .map(|line| line.trim().trim_matches(['(', ')', '"', ',', ' ']))
        .find(|line| !line.is_empty())
        .and_then(language_from_locale)
}

#[cfg(target_os = "windows")]
fn platform_language() -> Option<Language> {
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[System.Globalization.CultureInfo]::CurrentUICulture.Name",
        ])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
        .as_deref()
        .and_then(language_from_locale)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_language() -> Option<Language> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_does_not_depend_on_os_locale() {
        assert_eq!(resolve_language("en"), Language::En);
        assert_eq!(resolve_language("zh-CN"), Language::ZhCn);
        assert_eq!(canonical_language("invalid"), "auto");
    }

    #[test]
    fn locale_precedence_and_chinese_variants() {
        assert_eq!(
            preferred_language(["en_US.UTF-8", "zh_CN", "zh_TW"]),
            Some(Language::En)
        );
        assert_eq!(
            preferred_language(["", "zh_Hans_CN", "en_US"]),
            Some(Language::ZhCn)
        );
        assert_eq!(preferred_language(["", "", "zh-TW"]), Some(Language::ZhCn));
        assert_eq!(preferred_language(["C.UTF-8", "zh_CN", "en_US"]), None);
        assert_eq!(language_from_locale("UTF-8"), None);
        assert_eq!(language_from_locale("fr_FR.UTF-8"), Some(Language::En));
    }

    #[test]
    fn every_external_setting_has_chinese_copy() {
        use crate::settings::{SettingCategory, SettingKind, SettingsRegistry};
        let translations: HashMap<String, String> =
            serde_json::from_str(include_str!("translations.json")).unwrap();
        let registry = SettingsRegistry::defaults_with_host_features(&Default::default());
        let mut missing = Vec::new();
        let mut check = |source: &str| {
            if !source.is_empty() && !translations.contains_key(source) {
                missing.push(source.to_string());
            }
        };
        for meta in registry.all() {
            check(meta.label);
            check(meta.description);
            if let SettingKind::Enum { choices, .. } = &meta.kind {
                for choice in *choices {
                    check(choice.display);
                    check(choice.description);
                }
            }
        }
        for choice in crate::settings::dynamic_enum_choices(
            crate::settings::DynamicEnumSource::ActiveModelCatalog,
            &Default::default(),
        ) {
            check(&choice.display);
            check(&choice.description);
        }
        for category in SettingCategory::ALL {
            check(category.label());
            check(category.tab_label());
            for section in crate::settings::layout::sections_for(*category) {
                check(section);
            }
        }
        assert!(
            missing.is_empty(),
            "Missing Chinese settings copy: {missing:#?}"
        );
    }

    #[test]
    fn translations_preserve_dynamic_values_and_english() {
        assert_eq!(translate(Language::En, "Settings"), "Settings");
        assert_eq!(translate(Language::ZhCn, "Settings"), "设置");
        assert_eq!(
            translate(Language::ZhCn, "my-private-model"),
            "my-private-model"
        );
    }
}
