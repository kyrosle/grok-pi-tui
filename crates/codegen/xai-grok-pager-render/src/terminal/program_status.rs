//! OSC 7501 root status. The native Pager owns emission; Pi RPC owns no TTY.
//! Protocol: https://www.superlogical.com/rex/docs/build/program-status

use std::sync::OnceLock;

static SUPPORTED: OnceLock<bool> = OnceLock::new();
pub const CLEAR: &str = "\x1b]7501;state=clear:app=grok-pi\x1b\\";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Working,
    Permission,
    Question,
    Auth,
    Done,
    Error,
    Clear,
}

impl State {
    /// Constant reports carry no prompt, response, credential or session name.
    pub fn escape(self) -> &'static str {
        match self {
            Self::Idle => "\x1b]7501;state=idle:app=grok-pi\x1b\\",
            Self::Working => "\x1b]7501;state=working:app=grok-pi\x1b\\",
            Self::Permission => "\x1b]7501;state=blocked:app=grok-pi:kind=permission\x1b\\",
            Self::Question => "\x1b]7501;state=blocked:app=grok-pi:kind=question\x1b\\",
            Self::Auth => "\x1b]7501;state=blocked:app=grok-pi:kind=auth\x1b\\",
            Self::Done => "\x1b]7501;state=done:app=grok-pi\x1b\\",
            Self::Error => "\x1b]7501;state=error:app=grok-pi\x1b\\",
            Self::Clear => CLEAR,
        }
    }
}

pub fn supported() -> bool {
    SUPPORTED.get().copied().unwrap_or(false)
}

fn override_support(value: Option<&str>) -> Option<bool> {
    match value {
        Some("1") => Some(true),
        Some("0") => Some(false),
        _ => None,
    }
}

/// Shares the bounded native raw-fd probe, before any reader or XTVERSION
/// query is started. DA1 is the fence: a feature reply after it is too late.
/// Like the other timed native probes, typeahead inside this short read is
/// consumed; do not invoke this after the input-reader thread exists.
pub fn probe_at_startup() {
    use std::io::IsTerminal;
    SUPPORTED.get_or_init(|| {
        if let Some(enabled) = override_support(std::env::var("PI_PROGRAM_STATUS").ok().as_deref())
        {
            return enabled;
        }
        let ctx = super::terminal_context();
        if !std::io::stdin().is_terminal()
            || ctx.multiplexer.intercepts_csi_queries()
            || matches!(
                ctx.brand,
                super::TerminalName::AppleTerminal
                    | super::TerminalName::JetBrains
                    | super::TerminalName::GrokDesktop
            )
        {
            return false;
        }
        query_and_read()
    });
}

#[cfg(unix)]
fn query_and_read() -> bool {
    if !super::probe::write_query(b"\x1b]7501;?\x1b\\\x1b[c") {
        return false;
    }
    super::probe::read_tty_reply(std::time::Duration::from_millis(150), |bytes, byte| {
        byte == b'c' && bytes.windows(3).any(|window| window == b"\x1b[?")
    })
    .is_some_and(|reply| supports_reply(&reply))
}

#[cfg(not(unix))]
fn query_and_read() -> bool {
    false
}

fn supports_reply(reply: &[u8]) -> bool {
    let end = reply
        .windows(3)
        .position(|w| w == b"\x1b[?")
        .unwrap_or(reply.len());
    let reply = &reply[..end];
    let Some(start) = reply.windows(8).position(|w| w == b"\x1b]7501;?") else {
        return false;
    };
    let body = &reply[start + 8..];
    let Some(term) = body.iter().position(|byte| matches!(byte, 7 | 27)) else {
        return false;
    };
    body[term] == 7 || body.get(term..term + 2) == Some(b"\x1b\\")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detection_requires_a_complete_reply_before_da1_and_honors_overrides() {
        for bytes in [
            b"\x1b]7501;?\x1b\\\x1b[?62c".as_slice(),
            b"\x1b]7501;?:future=1\x07\x1b[?62c",
        ] {
            assert!(supports_reply(bytes));
        }
        for bytes in [
            b"\x1b[?62c\x1b]7501;?\x07".as_slice(),
            b"\x1b]7501;?",
            b"\x1b]7501;?\x1bX",
            b"\x1b]7501;state=idle\x07",
            b"\x1b[?62c",
        ] {
            assert!(!supports_reply(bytes));
        }
        assert_eq!(override_support(Some("1")), Some(true));
        assert_eq!(override_support(Some("0")), Some(false));
        assert_eq!(override_support(Some("auto")), None);
        assert!(State::Auth.escape().contains("kind=auth"));
        assert_eq!(State::Clear.escape(), CLEAR);
    }
}
