use super::bash_command_splitting::{
    MAX_TRANSPARENT_PREFIX_DEPTH, MAX_WRAPPER_DEPTH, TransparentPrefixPeel,
    peel_transparent_prefixes, unwrap_wrappers_checked,
};
/// Shared peel budget for nested `command env …` chains; remaining peelable layers fail closed.
const MAX_NORMALIZE_ROUNDS: usize = MAX_WRAPPER_DEPTH + MAX_TRANSPARENT_PREFIX_DEPTH;

pub enum NormalizedArgv<'a> {
    Ready(&'a [String]),
    FailClosed,
}

/// Alternate canonical wrappers and transparent prefixes until fixed point.
pub fn normalize_for_exec_risk(words: &[String]) -> NormalizedArgv<'_> {
    let mut current = words;
    for _ in 0..MAX_NORMALIZE_ROUNDS {
        let before = current;
        let checked = unwrap_wrappers_checked(current);
        if checked.exhausted || checked.has_split_string || checked.has_chdir {
            return NormalizedArgv::FailClosed;
        }
        let after_wrap = checked.words;
        let after_trans = match peel_transparent_prefixes(after_wrap) {
            TransparentPrefixPeel::Ambiguous => return NormalizedArgv::FailClosed,
            TransparentPrefixPeel::Ready(inner) => inner,
        };
        if std::ptr::eq(after_trans.as_ptr(), before.as_ptr()) && after_trans.len() == before.len()
        {
            return NormalizedArgv::Ready(after_trans);
        }
        if std::ptr::eq(after_trans.as_ptr(), after_wrap.as_ptr())
            && after_trans.len() == after_wrap.len()
        {
            return NormalizedArgv::Ready(after_trans);
        }
        current = after_trans;
    }
    NormalizedArgv::FailClosed
}

/// `min_len` is the shortest unique stem vs sibling options (e.g. sort `--co` vs `--check`).
pub fn is_accepted_long_option_prefix(flag: &str, full: &str, min_len: usize) -> bool {
    flag.starts_with("--")
        && flag.len() >= min_len
        && full.starts_with(flag)
        && flag.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub fn normalized_program_name(words: &[String]) -> Option<String> {
    let raw = words.first()?;
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw.as_str());
    if base.is_empty() {
        return None;
    }
    let mut name = base.to_ascii_lowercase();
    if let Some(stem) = name.strip_suffix(".exe") {
        name = stem.to_owned();
    }
    Some(name)
}

pub fn is_git_program(words: &[String]) -> bool {
    normalized_program_name(words).as_deref() == Some("git")
}

pub fn is_sort_program(words: &[String]) -> bool {
    normalized_program_name(words).as_deref() == Some("sort")
}

pub fn normalized_token_basename(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    let mut name = base.to_ascii_lowercase();
    if let Some(stem) = name.strip_suffix(".exe") {
        name = stem.to_owned();
    }
    name
}

/// GNU `sort --compress-program`; stops at `--`. Min stem `--co` vs `--check`.
pub fn sort_has_compress_program_flag(words: &[String]) -> bool {
    for w in words.iter().skip(1) {
        if w == "--" {
            break;
        }
        if w == "--compress-program" || w.starts_with("--compress-program=") {
            return true;
        }
        let flag = w.split_once('=').map(|(f, _)| f).unwrap_or(w.as_str());
        if is_accepted_long_option_prefix(flag, "--compress-program", 4) {
            return true;
        }
    }
    false
}

pub fn is_git_config_env_flag(tok: &str) -> bool {
    if tok == "--config-env" || tok.starts_with("--config-env=") {
        return true;
    }
    let flag = tok.split_once('=').map(|(f, _)| f).unwrap_or(tok);
    // Sole git global `--config*`; min stem `--co` (len 4).
    is_accepted_long_option_prefix(flag, "--config-env", 4)
}

/// Presence fails closed: these retarget which config git reads.
pub fn is_git_repo_retarget_flag(tok: &str) -> bool {
    if tok == "--git-dir"
        || tok.starts_with("--git-dir=")
        || tok == "--work-tree"
        || tok.starts_with("--work-tree=")
    {
        return true;
    }
    let flag = tok.split_once('=').map(|(f, _)| f).unwrap_or(tok);
    // git.c globals: `--gi` unique vs `--glob-pathspecs`; `--wor` sole `--wor*`.
    is_accepted_long_option_prefix(flag, "--git-dir", 4)
        || is_accepted_long_option_prefix(flag, "--work-tree", 4)
}

pub fn is_attached_git_config_c(tok: &str) -> bool {
    tok.starts_with("-c") && tok.len() > 2 && !tok.starts_with("--")
}

pub fn attached_git_c_path(tok: &str) -> Option<&str> {
    tok.strip_prefix("-C")
        .filter(|rest| !rest.is_empty() && !tok.starts_with("--"))
}

pub fn git_global_option_takes_value(tok: &str) -> bool {
    matches!(
        tok,
        "-C" | "-c"
            | "--git-dir"
            | "--work-tree"
            | "--namespace"
            | "--super-prefix"
            | "--exec-path"
            | "--list-cmds"
            | "--attr-source"
            | "--config-env"
    ) || is_accepted_long_option_prefix(tok, "--config-env", 4)
        || is_accepted_long_option_prefix(tok, "--git-dir", 4)
        || is_accepted_long_option_prefix(tok, "--work-tree", 4)
        || is_accepted_long_option_prefix(tok, "--namespace", 7)
        || is_accepted_long_option_prefix(tok, "--super-prefix", 8)
        || is_accepted_long_option_prefix(tok, "--exec-path", 7)
        || is_accepted_long_option_prefix(tok, "--list-cmds", 7)
        || is_accepted_long_option_prefix(tok, "--attr-source", 8)
}

/// Pre-subcommand only; missing values fail closed. Post-subcommand `git log -c` is not scanned.
pub fn git_has_exec_risk_global(words: &[String]) -> bool {
    let mut i = 1;
    while i < words.len() {
        let tok = words[i].as_str();
        if tok == "--" {
            return false;
        }
        if !tok.starts_with('-') || tok == "-" {
            return false;
        }
        if tok == "-c"
            || is_attached_git_config_c(tok)
            || is_git_config_env_flag(tok)
            || is_git_repo_retarget_flag(tok)
        {
            return true;
        }
        // `-Cpath` is cwd-only (ambient); skip so it is not treated as the subcommand.
        if attached_git_c_path(tok).is_some() {
            i += 1;
            continue;
        }
        if !tok.contains('=')
            && git_global_option_takes_value(tok)
            && words
                .get(i + 1)
                .is_some_and(|n| !n.starts_with('-') || n == "-")
        {
            i += 1;
        }
        i += 1;
    }
    false
}

pub fn segment_has_exec_risk_flag(words: &[String]) -> bool {
    if is_sort_program(words) {
        return sort_has_compress_program_flag(words);
    }
    if is_git_program(words) {
        return git_has_exec_risk_global(words);
    }
    false
}

#[derive(Debug, Clone, Copy)]
pub struct SegmentExecFacts {
    pub exec_risk: bool,
    pub has_git: bool,
}

/// Normalize raw segment words, then inspect git/sort. Unmodeled peels fail closed.
pub fn segment_exec_facts(words: &[String]) -> SegmentExecFacts {
    match normalize_for_exec_risk(words) {
        NormalizedArgv::FailClosed => SegmentExecFacts {
            exec_risk: true,
            has_git: false,
        },
        NormalizedArgv::Ready(inner) => SegmentExecFacts {
            exec_risk: segment_has_exec_risk_flag(inner),
            has_git: is_git_program(inner),
        },
    }
}

/// Read-only git query verbs, the SINGLE SOURCE for every git allow decision.
/// Both [`git_words_are_read_only_query`] and the `alias.<verb> = !cmd` shadowing check in the ambient config scan below read it.
/// Add a new read-only verb here and every consumer inherits it; do not grow per-consumer prefix lists.
pub const SAFE_GIT_SUBCOMMANDS: &[&str] = &[
    "status",
    "branch",
    "log",
    "diff",
    "ls-files",
    "show",
    "rev-parse",
    "blame",
    "grep",
    "describe",
    "merge-base",
    "check-ignore",
    "check-attr",
    "cat-file",
    "ls-tree",
    "show-ref",
    "for-each-ref",
    "rev-list",
    "name-rev",
    "count-objects",
    "shortlog",
];

/// Options that make an otherwise read-only git verb run content drivers or write arbitrary paths.
/// One table on every [`SAFE_GIT_SUBCOMMANDS`] verb so a new safe verb inherits the policy; `git grep`'s `-O<cmd>` is guarded separately.
const GIT_QUERY_UNSAFE_OPTIONS: &[&str] = &[
    "--filters",
    "--textconv",
    "--output",
    "--ext-diff",
    "--open-files-in-pager",
];

/// Git accepts uniquely-abbreviated long options, so any `--` word (pre-`=`, at least 3 chars) that prefixes a table entry fails closed.
/// That includes abbreviations a specific verb would resolve to a benign sibling (`git grep --text` collides with `--textconv` and prompts).
pub fn git_query_option_is_unsafe(word: &str) -> bool {
    let flag = word.split('=').next().unwrap_or(word);
    flag.len() > 2
        && GIT_QUERY_UNSAFE_OPTIONS
            .iter()
            .any(|full| full.starts_with(flag))
}

/// Subcommand index, skipping only benign globals (`-C` / `--no-pager` / `-P`); the ambient scan tracks the cwd `-C` retargets.
/// Every other pre-subcommand option fails closed: `-c`, `--git-dir`, `--exec-path`, and similar can change what executes or which config is read.
pub fn git_safe_query_verb_index(words: &[String]) -> Option<usize> {
    let mut i = 1;
    loop {
        let tok = words.get(i).map(String::as_str)?;
        if tok == "-" || tok == "--" {
            return None;
        }
        if !tok.starts_with('-') {
            return Some(i);
        }
        if tok == "-C" {
            i += 2;
            continue;
        }
        if attached_git_c_path(tok).is_some() || tok == "--no-pager" || tok == "-P" {
            i += 1;
            continue;
        }
        return None;
    }
}

/// True when a `git` invocation carries an option from [`GIT_QUERY_UNSAFE_OPTIONS`] or `git grep`'s short-attached `-O<cmd>`, whatever the verb.
/// Used by [`git_words_are_read_only_query`] and on its own, so a session whitelist-prefix grant cannot override a driver/write flag.
pub fn git_words_have_unsafe_query_option(words: &[String]) -> bool {
    if words.first().map(String::as_str) != Some("git") {
        return false;
    }
    if words.iter().skip(1).any(|w| git_query_option_is_unsafe(w)) {
        return true;
    }
    // `git grep -O<cmd>` / `-O <cmd>` executes <cmd>; the short-attached form is not a long-option abbreviation, so guard it verb-specifically
    matches!(git_safe_query_verb_index(words), Some(i) if words[i] == "grep")
        && words.iter().skip(1).any(|w| w.starts_with("-O"))
}

/// Single decision point for auto-approvable read-only `git` queries, shared by the manager safe lists and the auto-mode heuristic.
/// Callers pass wrapper-peeled words; `words[0]` must be literally `git` — path-qualified or case-variant binaries fail closed.
pub fn git_words_are_read_only_query(words: &[String]) -> bool {
    if words.first().map(String::as_str) != Some("git") {
        return false;
    }
    if git_has_exec_risk_global(words) {
        return false;
    }
    let Some(verb_idx) = git_safe_query_verb_index(words) else {
        return false;
    };
    if !SAFE_GIT_SUBCOMMANDS.contains(&words[verb_idx].as_str()) {
        return false;
    }
    !git_words_have_unsafe_query_option(words)
}
