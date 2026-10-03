//! Canonical marketplace display identity and URL matching; no discovery/runtime.

/// Display name of the official xAI marketplace source.
pub const OFFICIAL_SOURCE_NAME: &str = "xAI Official";

/// Git URL of the official xAI marketplace source, auto-registered on first run.
pub const OFFICIAL_SOURCE_GIT_URL: &str = "https://github.com/xai-org/plugin-marketplace.git";

/// Whether `url` is the official xAI marketplace source.
/// Case, a `www.` prefix, a trailing `/` or `.git`, and HTTPS/SSH forms are normalized before comparing.
pub fn is_official_source_url(url: &str) -> bool {
    canonical_github_owner_repo(url).as_deref() == Some("xai-org/plugin-marketplace")
}

/// Normalized lowercase `owner/repo` from a GitHub URL (HTTPS/http/ssh/scp, `www.`, trailing `.git`/`/`), or `None` if not a GitHub URL.
pub fn canonical_github_owner_repo(url: &str) -> Option<String> {
    let s = url.trim();
    let s = s.strip_suffix('/').unwrap_or(s);
    let s = s.strip_suffix(".git").unwrap_or(s);
    let lower = s.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .or_else(|| lower.strip_prefix("ssh://"))
        .unwrap_or(&lower);
    let rest = rest.strip_prefix("git@").unwrap_or(rest);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let owner_repo = rest
        .strip_prefix("github.com/")
        .or_else(|| rest.strip_prefix("github.com:"))?;
    if owner_repo.is_empty() {
        None
    } else {
        Some(owner_repo.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_official_matches_canonical_https() {
        assert!(is_official_source_url(OFFICIAL_SOURCE_GIT_URL));
        assert!(is_official_source_url(
            "https://github.com/xai-org/plugin-marketplace"
        ));
    }

    #[test]
    fn is_official_matches_ssh_form() {
        assert!(is_official_source_url(
            "git@github.com:xai-org/plugin-marketplace.git"
        ));
        assert!(is_official_source_url(
            "git@github.com:xai-org/plugin-marketplace"
        ));
        assert!(is_official_source_url(
            "ssh://git@github.com/xai-org/plugin-marketplace.git"
        ));
        assert!(is_official_source_url(
            "ssh://git@github.com/xai-org/plugin-marketplace"
        ));
    }

    #[test]
    fn is_official_rejects_unrelated_urls() {
        assert!(!is_official_source_url(
            "https://github.com/anthropics/claude-plugins-official.git"
        ));
        assert!(!is_official_source_url(
            "https://github.com/xai-org/some-other-repo.git"
        ));
        assert!(!is_official_source_url(""));
    }

    #[test]
    fn is_official_matches_noncanonical_forms() {
        assert!(is_official_source_url(
            "https://GitHub.com/XAI-org/Plugin-Marketplace"
        ));
        assert!(is_official_source_url(
            "https://github.com/xai-org/plugin-marketplace/"
        ));
        assert!(is_official_source_url(
            "https://github.com/xai-org/plugin-marketplace.git/"
        ));
        assert!(is_official_source_url(
            "http://github.com/xai-org/plugin-marketplace"
        ));
        assert!(is_official_source_url(
            "https://www.github.com/xai-org/plugin-marketplace.git"
        ));
        assert!(is_official_source_url(
            "git@github.com:XAI-org/plugin-marketplace.git"
        ));
    }
}
