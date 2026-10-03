//! Canonical session title control filtering and scalar bounds.

use std::borrow::Cow;

/// Maximum Unicode scalars in a session title (`/rename`, dashboard editor, and the `x.ai/session/rename` ext boundary).
/// Counted after control-strip and trim.
pub const MAX_TITLE_SCALARS: usize = 100;

/// UTF-8 byte ceiling before we bother stripping controls.
/// 4 bytes/scalar plus slack so a handful of C0 bytes that will be stripped don't trip a false reject.
/// Anything larger is already over the scalar cap.
pub const MAX_TITLE_BYTES: usize = MAX_TITLE_SCALARS * 4 + 64;

/// C0/C1 plus the bidi/format overrides the dashboard rename editor already rejects.
/// Shared by the persist path (drops these chars) and the display path (replaces them with U+FFFD) so the character class cannot drift.
#[inline]
pub fn is_forbidden_title_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
}

/// Drop C0/C1 and bidi/format controls, then trim.
/// A title therefore cannot carry terminal escapes or RTL overrides into `display_name` / `summary.json`.
/// Already-clean input is borrowed (trim is a subslice); only a title that actually contains forbidden chars allocates.
pub fn sanitize_rename_title(title: &str) -> Cow<'_, str> {
    if title.chars().any(is_forbidden_title_char) {
        let mut cleaned: String = title
            .chars()
            .filter(|c| !is_forbidden_title_char(*c))
            .collect();
        let trimmed = cleaned.trim();
        if trimmed.len() != cleaned.len() {
            cleaned = trimmed.to_string();
        }
        Cow::Owned(cleaned)
    } else {
        Cow::Borrowed(title.trim())
    }
}

/// Sanitize then cap. `None` when the result is blank.
/// Overlong titles are truncated (ingest/pull defense); the ext rename path rejects instead.
pub fn sanitize_and_cap_title(title: &str) -> Option<String> {
    let cleaned = sanitize_rename_title(title);
    if cleaned.is_empty() {
        return None;
    }
    if cleaned.chars().count() <= MAX_TITLE_SCALARS {
        Some(cleaned.into_owned())
    } else {
        Some(cleaned.chars().take(MAX_TITLE_SCALARS).collect())
    }
}
