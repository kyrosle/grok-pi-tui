//! Canonical typed ACP compaction cancellation discriminator.

use agent_client_protocol as acp;

/// Stable cancel payload; the pager matches it to route manual `/compact` to "Compaction cancelled." instead of a failure.
pub const COMPACT_CANCELLED_MSG: &str = "compact cancelled";

/// Cancel-vs-failure discriminator in the compact RPC error's `data` (`{"kind": …, "message": …}`).
/// The pager routes on this, never the message text (upstream bodies can echo the cancel phrase).
/// The protocol's `RequestCancelled` code is feature-gated unstable and cancel-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactErrorKind {
    Cancelled,
    Failed,
}

impl CompactErrorKind {
    pub fn wire(self) -> &'static str {
        match self {
            CompactErrorKind::Cancelled => "compact_cancelled",
            CompactErrorKind::Failed => "compact_failed",
        }
    }

    fn from_wire(s: &str) -> Option<Self> {
        match s {
            "compact_cancelled" => Some(Self::Cancelled),
            "compact_failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Read the typed discriminator back.
/// `None` for payloads from shells that predate it (bare strings) or for foreign shapes.
pub fn compact_error_kind(err: &acp::Error) -> Option<CompactErrorKind> {
    CompactErrorKind::from_wire(err.data.as_ref()?.get("kind")?.as_str()?)
}
