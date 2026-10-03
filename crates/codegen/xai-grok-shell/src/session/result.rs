use serde::Serialize;
pub use xai_grok_shared::session::result::{Empty, ExtMethodError, ExtMethodResult};

pub(crate) fn terminal_error_result<T: Serialize>(
    err: crate::terminal::TerminalExtError,
) -> ExtMethodResult<T> {
    let data = err
        .terminal_id()
        .map(|id| serde_json::json!({ "terminalId": id }));
    ExtMethodResult {
        result: None,
        error: serde_json::to_value(ExtMethodError {
            code: err.code().to_string(),
            message: err.to_string(),
            data,
        })
        .ok(),
    }
}
