use agent_client_protocol as acp;

/// Stable id for the "enable always-approve mode" option prepended for TUI / Pager / Desktop clients.
/// Shell maps it to [`PromptOutcome::AllowOnce`] and persists nothing; the pager separately fires `set_yolo_mode(true)`.
/// Keeps the wire plain ACP. An unrecognized client still gets `AllowOnce`; worst case the current call is granted but the toggle does not flip.
pub const ENABLE_ALWAYS_APPROVE_OPTION_ID: &str = "enable-always-approve";

/// Canonical check for the "enable always-approve mode" option; match on this, not the label or position 0.
pub fn is_enable_always_approve_option(opt: &acp::PermissionOption) -> bool {
    opt.option_id.0.as_ref() == ENABLE_ALWAYS_APPROVE_OPTION_ID
}
