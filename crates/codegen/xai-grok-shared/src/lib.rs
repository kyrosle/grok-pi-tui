//! Shared utilities used by both `xai-grok-shell` and its downstream clients (e.g. `xai-grok-pager-render`).
//! This crate sits upstream of the tools and shell; keep client utilities independent of their runtimes.

pub mod clipboard;
mod formatting;
pub mod host_features;
pub use formatting::format_bytes;
pub mod permissions;
pub mod envrc_budget;
pub mod placeholder_images;
pub mod session;
pub mod stderr;
pub mod ui_config;

#[cfg(test)]
mod placeholder_image_format_tests;

pub mod config;
