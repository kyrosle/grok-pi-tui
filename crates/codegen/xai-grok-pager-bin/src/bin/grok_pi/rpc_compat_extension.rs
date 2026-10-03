use anyhow::{Context, Result};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

pub(super) struct RpcCompatExtension {
    _source_dir: TempDir,
    source_path: PathBuf,
}

impl RpcCompatExtension {
    pub(super) fn path(&self) -> &Path {
        &self.source_path
    }
}

/// Materialize the RPC compatibility extension before user extensions load.
///
/// Pi remains in JSONL RPC mode. Runtime monkey-patches only:
/// - optional ExtensionRunner mode rewrite (`rpc` → `tui` when opted in)
/// - capture runner + enrich `get_commands` with extension argument completions
pub(super) fn write_rpc_compat_extension() -> Result<RpcCompatExtension> {
    let source_dir = tempfile::Builder::new()
        .prefix("pi-grok-rpc-compat-")
        .tempdir()
        .context("create Pi RPC compatibility extension source directory")?;
    for (name, source) in [
        (
            "index.ts",
            include_str!("../../../../../../extensions/pi-grok-rpc-compat/index.ts"),
        ),
        (
            "ui.ts",
            include_str!("../../../../../../extensions/pi-grok-rpc-compat/ui.ts"),
        ),
    ] {
        let mut file = File::create(source_dir.path().join(name))
            .with_context(|| format!("create Pi RPC compatibility module {name}"))?;
        file.write_all(source.as_bytes())
            .with_context(|| format!("write Pi RPC compatibility module {name}"))?;
        file.flush()
            .with_context(|| format!("flush Pi RPC compatibility module {name}"))?;
        file.sync_all().ok();
    }
    let source_path = source_dir.path().join("index.ts");
    Ok(RpcCompatExtension {
        _source_dir: source_dir,
        source_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_compat_extension_materializes_ui_and_scopes_private_hooks() {
        let file = write_rpc_compat_extension().expect("write extension");
        let source = std::fs::read_to_string(file.path()).expect("read extension");
        let ui = std::fs::read_to_string(file.path().parent().unwrap().join("ui.ts"))
            .expect("read UI bridge module");
        assert!(source.contains("from \"./ui.ts\""));
        assert!(ui.contains("WORKING_STATUS_KEY"));
        assert!(ui.contains("UI_CAPABILITIES"));
        assert!(ui.contains("setWorkingMessage"));
        assert!(ui.contains("getEditorText"));
        assert!(source.contains("pi-ui-capabilities"));
        assert!(source.contains("PI_GROK_EXTENSION_TUI_COMPAT"));
        assert!(source.contains("core/extensions/runner.js"));
        assert!(source.contains("setUIContext"));
        assert!(source.contains("process.env.PI_GROK === \"1\" && mode === \"rpc\""));
        assert!(source.contains("hasRemoteTuiHost(uiContext)"));
        assert!(source.contains("private host hook unavailable"));
        assert!(source.contains("getArgumentCompletions"));
        assert!(source.contains("argumentCompletions"));
        assert!(source.contains("__pi_grok_queue_enqueue__"));
        assert!(source.contains("event.source !== \"extension\""));
        assert!(source.contains("action: \"handled\""));
        // Keep Pi's existing stdout guard/backpressure; enrich at the child
        // Writable sink without importing a separate output-guard module copy.
        assert!(source.contains("output._write"));
        assert!(source.contains("previous.call(output"));
        assert!(!source.contains("core/output-guard.js"));
        assert!(!source.contains("takeOverStdout"));
        assert!(!source.contains("module.writeRawStdout"));
        assert!(!source.contains("process.argv ="));
        // Must not edit Pi sources; only host-module runtime hooks.
        assert!(!source.contains("rpc-mode.ts"));
        assert!(!source.contains("modes/rpc/rpc-mode"));
    }
}
