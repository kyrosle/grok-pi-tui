use super::exec_risk::{SAFE_GIT_SUBCOMMANDS, git_words_are_read_only_query};
/// kubectl flags that select caller-controlled config / endpoint / auth / identity (including shorthands).
/// Shared with `manager.rs::kubectl_has_unsafe_flag` so the two classifiers cannot drift.
pub const KUBECTL_UNSAFE_FLAGS: &[&str] = &[
    "--kubeconfig",
    "--context",
    "--cluster",
    "--server",
    "-s",
    "--token",
    "--user",
    "--as",
    "--as-group",
    "--as-uid",
    "--as-user-extra",
    "--username",
    "--password",
    "--client-certificate",
    "--client-key",
    "--certificate-authority",
];

/// ripgrep flags that spawn a caller-controlled binary. Shared with `manager.rs`.
pub const RG_UNSAFE_FLAGS: &[&str] = &["--pre", "--hostname-bin"];

/// True when `words` is `rg` with a [`RG_UNSAFE_FLAGS`] entry (`--pre-glob` excluded).
pub fn rg_has_unsafe_flag(words: &[String]) -> bool {
    if super::bash_patterns::normalized_command_head(words).as_deref() != Some("rg") {
        return false;
    }
    words.iter().skip(1).any(|w| {
        let name = w.split_once('=').map_or(w.as_str(), |(name, _)| name);
        RG_UNSAFE_FLAGS.contains(&name)
    })
}

/// True when `words` is a `kubectl` invocation that selects a caller-controlled kubeconfig, endpoint, auth, or identity.
/// A read verb like `get`/`logs`/`describe` is not side-effect-free once any of these flags point kubectl at attacker-supplied config/auth.
/// Such invocations must not ride the safe-command auto-allow (nor a broader whitelist *prefix* grant, see `evaluate_bash`).
pub fn kubectl_has_unsafe_flag(words: &[String]) -> bool {
    if super::bash_patterns::normalized_command_head(words).as_deref() != Some("kubectl") {
        return false;
    }
    words.iter().skip(1).any(|w| {
        let name = w.split_once('=').map_or(w.as_str(), |(name, _)| name);
        KUBECTL_UNSAFE_FLAGS.contains(&name)
    })
}

/// True when `words` is a `ps` that dumps process environments. Uppercase `E` dumps env on macOS (`-E`); we prompt on any `E` on all platforms because the runtime OS is unknown (fail-safe).
/// Plain UNIX `-e`/`-ef`/`-Ae` stay select-all; the `a`/`x` match is deliberately case-sensitive so `-Ae` is not treated as BSD.
pub fn ps_dumps_environment(words: &[String]) -> bool {
    if super::bash_patterns::normalized_command_head(words).as_deref() != Some("ps") {
        return false;
    }
    let mut skip_next = false;
    for w in words.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        let s = w.as_str();
        if s.starts_with("--format=") || s.starts_with("--sort=") {
            continue;
        }
        // Only flags whose VALUES can contain e/E need listing; an omission merely over-prompts (never leaks)
        // Skipping only ever swallows a ps operand
        if matches!(
            s,
            "-o" | "-O"
                | "--format"
                | "--sort"
                | "-p"
                | "-q"
                | "-t"
                | "-u"
                | "-U"
                | "-g"
                | "-G"
                | "-C"
                | "-s"
                | "--pid"
                | "--ppid"
                | "--sid"
                | "--tty"
                | "--user"
                | "--group"
                | "--cols"
                | "--columns"
                | "--width"
                // BSD dashless format selectors take a following format list.
                | "o"
                | "O"
        ) {
            skip_next = true;
            continue;
        }
        // Attached short form: `-oetime`, `-Opid`, …
        if s.starts_with("-o") || s.starts_with("-O") {
            continue;
        }

        // Env-dump option letters (checked before the trailing-o skip so `-Eo`/`-axeo` still force a prompt)
        let has_upper_e = s.contains('E');
        let has_lower_e = s.contains('e');
        let dashless = !s.starts_with('-');
        // Lowercase a/x only: `-Ae` is UNIX select-all, `-AE` has an E and dumps env
        let bsd_selector_cluster =
            s.starts_with('-') && !s.starts_with("--") && s.contains(['a', 'x']);
        if has_upper_e || (has_lower_e && (dashless || bsd_selector_cluster)) {
            return true;
        }

        // Short cluster ending in arg-taking `o`/`O` (`-eo etime`, `-axo cmd`): the next word is the format list, not an option cluster
        if s.starts_with('-') && !s.starts_with("--") && s.ends_with(['o', 'O']) {
            skip_next = true;
            continue;
        }
    }
    false
}

/// Check whether the command words (already parsed by tree-sitter) match one of the known safe command prefixes.
pub fn is_safe_command_words(words: &[String]) -> bool {
    if words.is_empty() {
        return false;
    }
    if rg_has_unsafe_flag(words) {
        return false;
    }
    if kubectl_has_unsafe_flag(words) {
        return false;
    }
    if ps_dumps_environment(words) {
        return false;
    }
    // Git rides its own shared decision helper (verb allowlist and unsafe-option table in `exec_risk.rs`), not the string prefixes below
    if words.first().map(String::as_str) == Some("git") {
        return git_words_are_read_only_query(words);
    }
    let joined = words.join(" ");
    is_safe_command_words_str(&joined)
}

pub fn matches_command_prefix(cmd: &str, pattern: &str) -> bool {
    cmd == pattern || (cmd.starts_with(pattern) && cmd.as_bytes().get(pattern.len()) == Some(&b' '))
}

/// `git <read-only verb>` prefix match, derived from the single [`SAFE_GIT_SUBCOMMANDS`] verb table.
/// String-level only (whitelist scope and fallback); the words paths decide via [`git_words_are_read_only_query`], which also rejects unsafe options.
pub fn is_safe_git_query_prefix(cmd: &str) -> bool {
    cmd.strip_prefix("git ").is_some_and(|rest| {
        SAFE_GIT_SUBCOMMANDS
            .iter()
            .any(|verb| matches_command_prefix(rest, verb))
    })
}

/// Shared prefix check used by both the tree-sitter path and the fallback path.
pub fn is_safe_command_words_str(cmd: &str) -> bool {
    matches_command_prefix(cmd, "ls")
        || matches_command_prefix(cmd, "cat")
        || matches_command_prefix(cmd, "pwd")
        || matches_command_prefix(cmd, "date")
        || is_safe_git_query_prefix(cmd)
        || matches_command_prefix(cmd, "whoami")
        || matches_command_prefix(cmd, "hostname")
        || matches_command_prefix(cmd, "uptime")
        || matches_command_prefix(cmd, "grep")
        || matches_command_prefix(cmd, "rg")
        || matches_command_prefix(cmd, "kubectl get")
        || matches_command_prefix(cmd, "kubectl logs")
        || matches_command_prefix(cmd, "kubectl describe")
        || matches_command_prefix(cmd, "ps")
        || matches_command_prefix(cmd, "bin/explorer ls")
        || matches_command_prefix(cmd, "head")
        || matches_command_prefix(cmd, "tail")
        || matches_command_prefix(cmd, "wc")
        || matches_command_prefix(cmd, "sort")
        || matches_command_prefix(cmd, "uniq")
        || matches_command_prefix(cmd, "tr")
        || matches_command_prefix(cmd, "cut")
        // Stdout-only; a redirect to a real file floors the script as a request-level `FileWrite` before the safe-list allow
        // Without these, an `…; echo saved` tail makes the whole chain impossible to cover with a grant
        || matches_command_prefix(cmd, "echo")
        || matches_command_prefix(cmd, "printf")
    // CWE-863: `tee` is not safe-listed; it writes stdin to arbitrary files, so pipelines like `cat data | tee /target` could bypass edit permissions
    //
    // [`rg_has_unsafe_flag`] is checked at the words level; the string form here cannot see flag structure reliably after join
}

/// Whether an always-allow grant for `words` must pin to the exact full command instead of a narrower prefix. Dangerous verbs (`rm`, `git push`, …) qualify because enforcement honors them only as exact whole-command grants.
/// Exec vehicles (interpreters, package runners, `sudo`/`ssh`) qualify because a bare `python3`/`sudo git` prefix would authorize any arguments.
pub fn always_allow_scope_pinned(words: &[String]) -> bool {
    // `sed` writes via script content (`-i`, `1w/path`), not a word prefix, so a `sed -n` prefix grant would silently cover those writes; pin it
    is_dangerous_command_words(words)
        || super::bash_patterns::head_is_exec_vehicle(words)
        || super::bash_patterns::normalized_command_head(words).as_deref() == Some("sed")
}

/// Default always-allow whitelist scope (word count) for a parsed command. Scope narrowing applies only when the **full** invocation is safe-listed.
/// Otherwise a non-auto-allowed form like `rg --pre …` would still scope to bare `rg`, and "Always allow" would re-open the preprocessor exec hole.
pub fn default_always_allow_scope(words: &[String]) -> usize {
    if words.is_empty() {
        return 0;
    }
    // Pinned commands (dangerous verbs, exec vehicles) offer only the full command A narrowed default like "Always allow:
    // git push" would save a rule that can never match "Always allow: sudo git" or "python3" would authorize arbitrary
    // arguments Wrapped/chained forms whose full-scope grant still cannot match get no row at all (`always_allow_row_is_effective`)
    if always_allow_scope_pinned(words) {
        return words.len();
    }
    if let Some(n) = gh_always_allow_scope(words) {
        return n;
    }
    base_scope(words)
}

/// `gh`'s remote-mutating verb is its third word (`gh pr merge`), so a narrower `gh pr` prefix would cover it.
/// Scope to group and action, else pin to the full command.
/// Both the default and the minimum use this, so the left arrow can't narrow below it.
pub fn gh_always_allow_scope(words: &[String]) -> Option<usize> {
    if super::bash_patterns::normalized_command_head(words).as_deref() != Some("gh") {
        return None;
    }
    Some(match (words.get(1), words.get(2)) {
        (Some(group), Some(verb)) if !group.starts_with('-') && !verb.starts_with('-') => 3,
        _ => words.len(),
    })
}

/// Default "Never allow" scope (word count) for a parsed command.
/// Denies honor prefixes for every command, so the dangerous full-command pin does not apply.
/// "Never allow: git push" blocking all pushes is the point.
pub fn default_always_deny_scope(words: &[String]) -> usize {
    if words.is_empty() {
        return 0;
    }
    base_scope(words)
}

/// Verb-plus-flags scope shared by the allow default (non-dangerous arm) and the deny default.
pub fn base_scope(words: &[String]) -> usize {
    if is_safe_command_words(words) {
        if is_safe_command_words_str(&words[0]) {
            return 1;
        }
        if words.len() >= 2 && is_safe_command_words_str(&words[..2].join(" ")) {
            return 2;
        }
    }
    let mut n = words.len().min(2);
    while n < words.len() && words[n].starts_with('-') {
        n += 1;
    }
    n
}

/// Narrowest always-allow scope (word count) the prompt may offer for a parsed command. Only the exact command the user saw may persist.
/// Deny scopes are not pinned (see [`default_always_deny_scope`]).
pub fn minimum_always_allow_scope(words: &[String]) -> usize {
    if always_allow_scope_pinned(words) {
        return words.len();
    }
    // Narrowing `gh` broadens the grant (fewer words cover more subcommands), so the floor equals the default: the left arrow cannot reach `gh pr`
    gh_always_allow_scope(words).unwrap_or(1)
}

/// Check whether parsed command words begin with a known dangerous command. Applied per chained segment, not only the start of the script.
/// A segment matching this check is NEVER auto-approved via a user whitelist; the user must always be prompted for it.
pub fn is_dangerous_command_words(words: &[String]) -> bool {
    // Match on the normalized basename so `/bin/rm`, `RM`, and `rm.exe` are all caught (consistent with `head_is_exec_vehicle` and the sed pin)
    let Some(head) = super::bash_patterns::normalized_command_head(words) else {
        return false;
    };
    let joined = if words.len() == 1 {
        head
    } else {
        format!("{head} {}", words[1..].join(" "))
    };
    matches_command_prefix(&joined, "rm")
        || matches_command_prefix(&joined, "chmod")
        || matches_command_prefix(&joined, "chown")
        || matches_command_prefix(&joined, "chgrp")
        || matches_command_prefix(&joined, "chattr")
        || matches_command_prefix(&joined, "pkill")
        || matches_command_prefix(&joined, "kill")
        || matches_command_prefix(&joined, "killall")
        || matches_command_prefix(&joined, "git push")
}

use super::bash_command_splitting::{
    BashCommandHighlights, try_parse_shell, try_parse_word_only_commands_sequence,
};
/// Whether accepting the always-allow row at scope `n` persists a grant that replays. Arrows skip scopes that would save nothing.
/// What persists is an argv-unambiguous prefix at or above the dangerous-command floor, or the full unwrapped script as raw text. Ambiguous joins are refused.
pub fn always_allow_scope_persists(h: &BashCommandHighlights, n: usize) -> bool {
    let words = &h.highlighted_words;
    if n == 0 || n > words.len() || n < minimum_always_allow_scope(words) {
        return false;
    }
    // The raw-text fallback needs a single unwrapped command spanning the whole script (empty prefix/suffix)
    words_join_unambiguously(&words[..n])
        || (n == words.len() && h.prefix.is_empty() && h.suffix.is_empty())
}

/// Whether `join(" ")` round-trips these words to the same single command.
/// Whitespace or metacharacters collapse into a different argv or chain, so a join is a grant key only if re-parsing yields this exact argv.
pub fn words_join_unambiguously(words: &[String]) -> bool {
    let joined = words.join(" ");
    try_parse_shell(&joined)
        .and_then(|tree| try_parse_word_only_commands_sequence(&tree, &joined))
        .is_some_and(|segments| {
            matches!(segments.as_slice(),
                [only] if only.spans_whole_script(&joined) && only.words() == words)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }
    #[test]
    fn native_scope_preview_keeps_sensitive_invocations_pinned() {
        assert_eq!(default_always_allow_scope(&words("ls src")), 1);
        assert_eq!(default_always_allow_scope(&words("git status --short")), 2);
        for cmd in [
            "git push origin main",
            "python3 -c code",
            "sudo git status",
            "sed -i s/a/b/ file",
        ] {
            let w = words(cmd);
            assert_eq!(default_always_allow_scope(&w), w.len(), "{cmd}");
            assert_eq!(minimum_always_allow_scope(&w), w.len(), "{cmd}");
        }
        assert_eq!(minimum_always_allow_scope(&words("gh pr merge 1")), 3);
        assert!(rg_has_unsafe_flag(&words("rg --pre=cmd text")));
        assert!(!rg_has_unsafe_flag(&words("rg --pre-glob=*.txt text")));
        assert!(kubectl_has_unsafe_flag(&words(
            "kubectl get pods --kubeconfig=remote"
        )));
        assert!(ps_dumps_environment(&words("ps auxe")));
        assert!(!ps_dumps_environment(&words("ps -ef")));
        let h = BashCommandHighlights {
            prefix: Vec::new(),
            highlighted_words: vec!["python3".into(), "-c".into(), "two words".into()],
            suffix: Vec::new(),
        };
        assert!(!always_allow_scope_persists(&h, 2));
        assert!(always_allow_scope_persists(&h, 3));
    }
}
