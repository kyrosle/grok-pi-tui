use anyhow::{Context, Result};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempDir};

/// Private paths shared by the grok-pi composition binary, the injected
/// Bash extension bundle, and the headless adapter. The Bash control
/// metadata file is process-unique; it avoids a global tmp-file collision
/// between concurrent grok-pi sessions.
pub(super) struct BashExtension {
    _source_dir: TempDir,
    source_path: PathBuf,
    control_meta: NamedTempFile,
}

impl BashExtension {
    pub(super) fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub(super) fn control_meta_path(&self) -> &Path {
        self.control_meta.path()
    }
}

fn write_source_file(dir: &Path, name: &str, source: &str) -> Result<PathBuf> {
    let path = dir.join(name);
    let mut file =
        File::create(&path).with_context(|| format!("create Pi Bash extension module {name}"))?;
    file.write_all(source.as_bytes())
        .with_context(|| format!("write Pi Bash extension module {name}"))?;
    file.flush()
        .with_context(|| format!("flush Pi Bash extension module {name}"))?;
    file.sync_all().ok();
    Ok(path)
}

/// Materialize the private grok-pi Bash bundle and Bash control metadata.
/// The source directory remains alive for the Pi process lifetime so relative
/// imports between the authored TypeScript modules continue to resolve.
pub(super) fn write_bash_extension() -> Result<BashExtension> {
    let source_dir = tempfile::Builder::new()
        .prefix("pi-grok-bash-")
        .tempdir()
        .context("create Pi Bash extension source directory")?;
    let source_path = write_source_file(
        source_dir.path(),
        "index.ts",
        include_str!("../../../../../../extensions/pi-grok-bash/index.ts"),
    )?;
    write_source_file(
        source_dir.path(),
        "bash-tasks.ts",
        include_str!("../../../../../../extensions/pi-grok-bash/bash-tasks.ts"),
    )?;
    write_source_file(
        source_dir.path(),
        "prompts.ts",
        include_str!("../../../../../../extensions/pi-grok-bash/prompts.ts"),
    )?;
    write_source_file(
        source_dir.path(),
        "shared.ts",
        include_str!("../../../../../../extensions/pi-grok-bash/shared.ts"),
    )?;

    let control_meta = tempfile::Builder::new()
        .prefix("pi-grok-bash-control-")
        .suffix(".json")
        .tempfile()
        .context("create Pi Bash control metadata tempfile")?;
    Ok(BashExtension {
        _source_dir: source_dir,
        source_path,
        control_meta,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bash_extension_materializes_only_its_import_closure() {
        let extension = write_bash_extension().expect("write extension");
        let dir = extension.source_path().parent().unwrap();
        let source = std::fs::read_to_string(extension.source_path()).unwrap();
        for name in ["bash-tasks.ts", "prompts.ts", "shared.ts"] {
            assert!(dir.join(name).is_file(), "missing {name}");
        }
        assert!(source.contains("from \"./bash-tasks.ts\""));
        assert!(source.contains("name: \"get_task_output\""));
        assert!(source.contains("name: \"wait_tasks\""));
        assert!(source.contains("name: \"kill_task\""));
        assert!(!source.contains("PersistentEvalKernel"));
        assert!(!dir.join("eval.ts").exists());
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 4);
    }
}
