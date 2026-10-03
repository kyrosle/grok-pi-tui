//! Grok agent configuration UI and its canonical tab data.

/// Which tab is active in the agents modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentsTab {
    Agents,
    Personas,
}
impl AgentsTab {
    /// All tabs in display order.
    pub const ALL: &[Self] = &[Self::Agents, Self::Personas];
    /// Display label for the tab bar.
    pub fn label(self) -> &'static str {
        match self {
            Self::Agents => "Agents",
            Self::Personas => "Personas",
        }
    }
    /// Next tab (wraps around).
    pub fn next(self) -> Self {
        match self {
            Self::Agents => Self::Personas,
            Self::Personas => Self::Agents,
        }
    }
    /// Previous tab (wraps around).
    pub fn prev(self) -> Self {
        match self {
            Self::Agents => Self::Personas,
            Self::Personas => Self::Agents,
        }
    }
}
#[cfg(feature = "stock-runtime")]
#[path = "agents_modal_stock.rs"]
mod stock;
#[cfg(feature = "stock-runtime")]
pub use stock::*;
