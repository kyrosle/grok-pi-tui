//! Canonical locked settings writer; stock callers keep their original path.
pub use xai_grok_shared::config::persist::*;

#[cfg(test)]
#[path = "persist_tests.rs"]
mod tests;
