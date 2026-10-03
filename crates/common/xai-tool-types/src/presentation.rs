//! Canonical byte-safe presentation helpers.

/// Truncate a string to at most `max_bytes` bytes at a valid UTF-8 boundary. Returns the original
/// string if it fits. No truncation marker is added. Walks back from `max_bytes` until a char
/// boundary is found. At most 3 steps back since UTF-8 multibyte sequences are at most 4 bytes.
pub fn truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
#[cfg(test)]
mod tests {
    #[test]
    fn byte_cap_never_splits_utf8() {
        for cap in 0..=8 {
            let text = super::truncate_str("中a文", cap);
            assert!(text.len() <= cap);
            assert!("中a文".starts_with(text));
        }
    }
}

use std::borrow::Cow;
pub const DEFAULT_SOFT_WRAP_WIDTH: usize = 2_000;
/// Soft-wrap a long line by inserting newlines every `wrap_width` characters. **All content is preserved** — nothing is discarded. Returns
/// `Cow::Borrowed` if the line is already within `wrap_width` (zero-copy). This is the correct strategy for bash and task_output, where the
/// total output is already size-bounded (30KB) and the model benefits from seeing all of it.
pub fn soft_wrap_line(line: &str, wrap_width: usize) -> Cow<'_, str> {
    // Fast path: same byte-length optimization as truncate_line (see comment there).
    if line.len() <= wrap_width {
        return Cow::Borrowed(line);
    }
    let char_count = line.chars().count();
    if char_count <= wrap_width {
        return Cow::Borrowed(line);
    }
    let num_wraps = char_count.saturating_sub(1) / wrap_width;
    let mut result = String::with_capacity(line.len() + num_wraps);
    let mut chars_on_current_line = 0;
    for ch in line.chars() {
        if chars_on_current_line >= wrap_width {
            result.push('\n');
            chars_on_current_line = 0;
        }
        result.push(ch);
        chars_on_current_line += 1;
    }
    Cow::Owned(result)
}

/// Apply soft-wrapping to every line in a multi-line string.
/// All content is preserved. Lines already within `wrap_width` are untouched.
pub fn soft_wrap_lines(text: &str, wrap_width: usize) -> String {
    let mut result = String::with_capacity(text.len() + 256);
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        match soft_wrap_line(line, wrap_width) {
            Cow::Borrowed(s) => result.push_str(s),
            Cow::Owned(s) => result.push_str(&s),
        }
    }
    if text.ends_with('\n') && !text.is_empty() {
        result.push('\n');
    }
    result
}

/// Human-readable size in powers of 1024: integral bytes (`512 B`), one
/// decimal above (`1.5 MB`). Every output fits nine columns.
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    const UNITS: &[&str] = &["KB", "MB", "GB", "TB", "PB"];
    let mut val = bytes as f64 / 1024.0;
    for unit in UNITS {
        if val < 1023.95 {
            return format!("{val:.1} {unit}");
        }
        val /= 1024.0;
    }
    format!("{val:.1} EB")
}
