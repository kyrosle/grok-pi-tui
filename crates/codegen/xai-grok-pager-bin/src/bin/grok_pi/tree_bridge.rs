use anyhow::{Context, Result};
use std::{fs::File, io::Write};
use tempfile::NamedTempFile;

/// Inject a headless Pi extension that exposes tree control over RPC without
/// modifying Pi source. Hidden from slash UI by adapter filtering.
pub(super) fn write_navigate_tree_extension() -> Result<NamedTempFile> {
    // NamedTempFile defaults to no suffix; force `.ts` so Pi's loader accepts it.
    let mut file = tempfile::Builder::new()
        .prefix("pi-grok-tree-bridge-")
        .suffix(".ts")
        .tempfile()
        .context("create tree bridge extension tempfile")?;
    // Official ExtensionCommandContext: navigateTree + setLabel (rpc-mode).
    const SOURCE: &str = r#"import { writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { CONFIG_DIR_NAME, DefaultPackageManager, SettingsManager, getAgentDir } from "@earendil-works/pi-coding-agent";

export default function (pi) {
  // Read declarations freshly; the registry below belongs to this running Pi,
  // rather than to a second resource loader that would execute extensions again.
  pi.registerCommand("__pi_package_snapshot", {
    description: "Internal Pi-Grok bridge: package declarations and live registry",
    handler: async (args, ctx) => {
      const { responsePath } = JSON.parse(args);
      if (typeof responsePath !== "string" || !responsePath) throw new Error("responsePath required");
      const agentDir = getAgentDir();
      const projectTrusted = ctx.isProjectTrusted();
      const settingsManager = SettingsManager.create(ctx.cwd, agentDir, { projectTrusted });
      const packageManager = new DefaultPackageManager({ cwd: ctx.cwd, agentDir, settingsManager });
      const packages = packageManager.listConfiguredPackages();
      let resolvedPaths;
      const resolutionErrors = [];
      try {
        // Never install a missing declaration while inspecting admission.
        resolvedPaths = await packageManager.resolve(async () => "skip");
      } catch (error) {
        resolutionErrors.push(String(error));
      }
      writeFileSync(responsePath, JSON.stringify({
        cwd: ctx.cwd, agentDir, projectTrusted,
        sessionId: ctx.sessionManager.getSessionId(),
        // Fresh effective settings, not getters for the live runtime switches.
        retryConfiguredEnabled: settingsManager.getRetryEnabled(),
        autoCompactionConfiguredEnabled: settingsManager.getCompactionEnabled(),
        settingsErrors: settingsManager.drainErrors().map(({ scope, path, error }) => ({ scope, path, error: error.message })),
        packages,
        packageSettingsBases: { user: resolve(ctx.cwd, agentDir), project: join(ctx.cwd, CONFIG_DIR_NAME) },
        resolvedPaths, resolutionErrors,
        commands: pi.getCommands(),
        tools: pi.getAllTools().map(({ name, source }) => ({ name, source })),
        // RPC does not expose the current resource loader's errors. Registry
        // entries above are live facts; their absence cannot prove load success.
        loaded: null, loadStatus: "unverified",
      }), { mode: 0o600 });
    },
  });

  pi.registerCommand("__pi_navigate_tree", {
    description: "Internal Pi-Grok bridge: navigate session tree leaf",
    handler: async (args, ctx) => {
      const raw = String(args ?? "").trim();
      if (!raw) throw new Error("entry id required");
      const summarize = /(?:^|\s)--summarize(?:\s|$)/.test(raw);
      let customInstructions;
      const instrMatch = raw.match(/(?:^|\s)--instructions\s+([\s\S]+)$/);
      if (instrMatch) customInstructions = instrMatch[1].trim();
      const entryId = raw
        .replace(/(?:^|\s)--summarize(?:\s|$)/g, " ")
        .replace(/(?:^|\s)--instructions\s+[\s\S]+$/, " ")
        .trim()
        .split(/\s+/)[0];
      if (!entryId) throw new Error("entry id required");
      const result = await ctx.navigateTree(entryId, {
        summarize,
        customInstructions: customInstructions || undefined,
      });
      if (result?.cancelled) throw new Error("tree navigation cancelled");
    },
  });

  pi.registerCommand("__pi_tree_label", {
    description: "Internal Pi-Grok bridge: set/clear session tree label",
    handler: async (args, ctx) => {
      const raw = String(args ?? "").trim();
      if (!raw) throw new Error("entry id required");
      const tokens = raw.split(/\s+/);
      const entryId = tokens[0];
      if (!entryId) throw new Error("entry id required");
      if (tokens.includes("--clear")) {
        ctx.setLabel(entryId, undefined);
        return;
      }
      const label = raw.slice(entryId.length).trim();
      ctx.setLabel(entryId, label || undefined);
    },
  });

  // Official ExtensionAPI: ctx.reload() reloads settings/resources/extensions.
  pi.registerCommand("__pi_reload", {
    description: "Internal Pi-Grok bridge: reload settings, extensions, skills, prompts, themes, context",
    handler: async (args, ctx) => {
      const { responsePath } = JSON.parse(args || "{}");
      try {
        await ctx.reload();
        if (responsePath) writeFileSync(responsePath, JSON.stringify({ ok: true }));
      } catch (error) {
        if (!responsePath) throw error;
        writeFileSync(responsePath, JSON.stringify({ ok: false, error: String(error) }));
      }
    },
  });
}
"#;
    file.write_all(SOURCE.as_bytes())
        .context("write tree bridge extension source")?;
    file.flush().context("flush tree bridge extension")?;
    // Ensure the file is durable before Pi spawns.
    File::open(file.path()).and_then(|f| f.sync_all()).ok();
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigate_tree_extension_source_is_valid_ts_module() {
        let file = write_navigate_tree_extension().expect("temp extension");
        let source = std::fs::read_to_string(file.path()).expect("read extension");
        assert!(source.contains("registerCommand(\"__pi_navigate_tree\""));
        assert!(source.contains("registerCommand(\"__pi_tree_label\""));
        assert!(source.contains("registerCommand(\"__pi_reload\""));
        assert!(source.contains("registerCommand(\"__pi_package_snapshot\""));
        assert!(source.contains("listConfiguredPackages()"));
        assert!(source.contains("ctx.isProjectTrusted()"));
        assert!(source.contains("pi.getCommands()"));
        assert!(source.contains("ctx.navigateTree"));
        assert!(source.contains("ctx.setLabel"));
        assert!(source.contains("ctx.reload"));
        assert!(file.path().extension().and_then(|e| e.to_str()) == Some("ts"));
    }
}
