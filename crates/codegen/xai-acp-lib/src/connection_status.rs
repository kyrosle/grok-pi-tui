/// Status of a reconnection attempt, observable by callers (e.g., TUI banner).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Connected to the leader.
    /// `generation` is 0 for the initial connection and increments on every successful reconnect. Observers compare it against the last generation they handled, so a fast `Reconnecting -> Connected` flip coalesced by the watch channel still registers as a reconnect.
    Connected { generation: u64 },
    /// Attempting to reconnect (includes current attempt number).
    Reconnecting { attempt: u32 },
    /// Reconnection failed permanently.
    Failed { error: String },
}
