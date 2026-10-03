/**
 * grok-pi RPC compatibility facade (no Pi source edits).
 *
 * 1. Opt-in: present Remote TUI host as `tui` mode to third-party extensions
 *    (`PI_GROK_EXTENSION_TUI_COMPAT=1`).
 * 2. Always (when this extension loads): capture ExtensionRunner, snapshot
 *    extension `getArgumentCompletions("")` results, and enrich `get_commands`
 *    RPC stdout so Pager can render arg dropdowns (e.g. /gapp list|open|…).
 *
 * Pi stays in JSONL RPC. All patches are runtime host-module hooks.
 *
 * Completion enrichment runs at the child stdout Writable sink. Pi's existing
 * stdout guard and backpressure remain intact; no output-guard module clone or
 * frozen ESM export is patched.
 */

import { basename, dirname } from "node:path";
import { pathToFileURL } from "node:url";
import { realpathSync } from "node:fs";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import {
  hasRemoteTuiHost, hasRpcUiBridge, installRpcUiBridge, resetRpcUiBridge,
  setWorkingActive, uiCapabilityReport,
} from "./ui.ts";

type ArgCompletion = { value: string; label: string; description?: string };

type ExtensionCommand = {
  invocationName: string;
  name?: string;
  getArgumentCompletions?: (
    argumentPrefix: string,
  ) => ArgCompletion[] | null | Promise<ArgCompletion[] | null>;
};

type ExtensionRunnerLike = {
  setUIContext: (uiContext: unknown, mode?: string) => void;
  getRegisteredCommands: () => ExtensionCommand[];
  getCommand?: (name: string) => ExtensionCommand | undefined;
};

type ExtensionRunnerConstructor = {
  prototype: ExtensionRunnerLike & {
    __piGrokTuiModeFacade?: boolean;
    __piGrokRunnerCapture?: boolean;
    __piGrokGetCommandsCapture?: boolean;
  };
};

const PROCESS_MARK = "__piGrokGetCommandsStdoutWrap" as const;
const QUEUE_STATUS_KEY = "__pi_grok_queue_enqueue__" as const;

/** invocationName → empty-prefix completions (sync providers + settled async). */
const completionCache = new Map<string, ArgCompletion[]>();

function hostUrl(relativePath: string): string {
  const entryDir = dirname(realpathSync(process.argv[1]!));
  if (
    basename(entryDir) === "bundle" &&
    (relativePath === "core/extensions/runner.js" || relativePath.startsWith("modes/interactive/components/"))
  ) {
    return new URL("index.js", pathToFileURL(`${entryDir}/`)).href;
  }
  const hostDistDir = basename(entryDir) === "bundle" ? dirname(entryDir) : entryDir;
  return new URL(relativePath, pathToFileURL(`${hostDistDir}/`)).href;
}

function normalizeCompletions(items: ArgCompletion[]): ArgCompletion[] {
  return items.map((item) => ({
    value: item.value,
    label: item.label,
    ...(item.description ? { description: item.description } : {}),
  }));
}

/** Snapshot completions for one command; sync fills cache immediately. */
function snapshotCommandCompletions(command: ExtensionCommand): void {
  const getCompletions = command.getArgumentCompletions;
  if (!getCompletions) return;
  const name = command.invocationName;
  try {
    const result = getCompletions("");
    if (result && typeof (result as Promise<unknown>).then === "function") {
      void (result as Promise<ArgCompletion[] | null>)
        .then((items) => {
          if (Array.isArray(items) && items.length > 0) {
            completionCache.set(name, normalizeCompletions(items));
          }
        })
        .catch(() => {});
      return;
    }
    if (Array.isArray(result) && result.length > 0) {
      completionCache.set(name, normalizeCompletions(result));
    }
  } catch {
    // Completions are best-effort.
  }
}

function snapshotAllCompletions(runner: ExtensionRunnerLike): void {
  try {
    for (const command of runner.getRegisteredCommands()) {
      snapshotCommandCompletions(command);
    }
  } catch {
    // ignore
  }
}

async function loadExtensionRunnerPrototype(): Promise<
  | (ExtensionRunnerLike & {
      __piGrokTuiModeFacade?: boolean;
      __piGrokRunnerCapture?: boolean;
      __piGrokGetCommandsCapture?: boolean;
    })
  | null
> {
  const module = (await import(hostUrl("core/extensions/runner.js"))) as {
    ExtensionRunner?: ExtensionRunnerConstructor;
  };
  return module.ExtensionRunner?.prototype ?? null;
}

export function installRunnerHooksOn(prototype: ExtensionRunnerConstructor["prototype"]): void {
  if (typeof prototype.setUIContext !== "function" || typeof prototype.getRegisteredCommands !== "function") {
    throw new Error("Pi ExtensionRunner contract changed (setUIContext/getRegisteredCommands)");
  }
  // One scoped hook: never relabel native/print mode, and never advertise TUI
  // when the actual custom() host is absent or disabled.
  if (!prototype.__piGrokRunnerCapture) {
    const original = prototype.setUIContext;
    prototype.setUIContext = function setUIContext(
      this: ExtensionRunnerLike,
      uiContext: unknown,
      mode = "print",
    ): void {
      let projectedMode = mode;
      if (process.env.PI_GROK === "1" && mode === "rpc") {
        installRpcUiBridge(uiContext);
        const remoteFlag = process.env.PI_GROK_REMOTE_TUI?.toLowerCase();
        const remoteEnabled = !["0", "false", "off", "no"].includes(remoteFlag ?? "");
        if (process.env.PI_GROK_EXTENSION_TUI_COMPAT === "1" && remoteEnabled) {
          const host = globalThis as typeof globalThis & {
            __piGrokEnsureRemoteTuiHost?: (ui: unknown) => void;
          };
          host.__piGrokEnsureRemoteTuiHost?.(uiContext);
          if (hasRemoteTuiHost(uiContext)) projectedMode = "tui";
        }
      }
      original.call(this, uiContext, projectedMode);
      snapshotAllCompletions(this);
    };
    prototype.__piGrokRunnerCapture = true;
  }

  // get_commands always calls getRegisteredCommands first — snapshot there so
  // stdout enrich can apply on the same turn (sync providers).
  if (!prototype.__piGrokGetCommandsCapture) {
    const originalGet = prototype.getRegisteredCommands;
    prototype.getRegisteredCommands = function getRegisteredCommands(
      this: ExtensionRunnerLike,
    ): ExtensionCommand[] {
      const commands = originalGet.call(this);
      for (const command of commands) {
        snapshotCommandCompletions(command);
      }
      return commands;
    };
    prototype.__piGrokGetCommandsCapture = true;
  }
}

async function installRunnerHooks(): Promise<void> {
  const prototype = await loadExtensionRunnerPrototype();
  if (!prototype) throw new Error("Pi ExtensionRunner is unavailable for grok-pi RPC compatibility");
  installRunnerHooksOn(prototype);
}

function isGetCommandsSuccessLine(obj: unknown): obj is {
  type: "response";
  command: "get_commands";
  success: true;
  data?: { commands?: Array<Record<string, unknown>> };
  id?: string;
} {
  if (!obj || typeof obj !== "object") return false;
  const row = obj as Record<string, unknown>;
  return row.type === "response" && row.command === "get_commands" && row.success === true;
}

function enrichGetCommandsLine(line: string): string {
  let parsed: unknown;
  try {
    parsed = JSON.parse(line);
  } catch {
    return line;
  }
  if (!isGetCommandsSuccessLine(parsed)) return line;
  const commands = parsed.data?.commands;
  if (!Array.isArray(commands) || commands.length === 0 || completionCache.size === 0) {
    return line;
  }

  let changed = false;
  const next = commands.map((command) => {
    if (command.argumentCompletions != null) return command;
    if (command.source !== "extension") return command;
    const name = typeof command.name === "string" ? command.name : "";
    if (!name) return command;
    const argumentCompletions = completionCache.get(name);
    if (!argumentCompletions) return command;
    changed = true;
    return { ...command, argumentCompletions };
  });
  if (!changed) return line;
  return JSON.stringify({
    ...parsed,
    data: { ...(parsed.data ?? {}), commands: next },
  });
}

function maybeEnrichStdoutText(text: string): string {
  if (!text.includes('"get_commands"') || !text.includes('"success":true')) {
    return text;
  }
  const lines = text.split("\n");
  return lines.map((line) => (line ? enrichGetCommandsLine(line) : line)).join("\n");
}

/** Pi writes each serialized RPC line in order and awaits the Writable callback. */
async function installGetCommandsStdoutIntercept(): Promise<void> {
  const proc = process as NodeJS.Process & { [PROCESS_MARK]?: boolean };
  if (proc[PROCESS_MARK]) return;

  type Write = (chunk: string | Uint8Array, encoding: BufferEncoding, callback: (error?: Error | null) => void) => void;
  const output = process.stdout as typeof process.stdout & { _write: Write };
  if (typeof output._write !== "function") {
    console.error("[pi-grok-rpc-compat] completion enrichment unavailable: stdout Writable sink missing");
    return;
  }
  const previous = output._write;
  let warned = false;
  output._write = ((chunk, encoding, callback) => {
    const text = typeof chunk === "string" ? chunk : Buffer.from(chunk).toString("utf8");
    let enriched = text;
    try {
      enriched = maybeEnrichStdoutText(text);
    } catch (error) {
      if (!warned) console.error("[pi-grok-rpc-compat] completion enrichment failed; preserving official RPC output:", error);
      warned = true;
    }
    // Pass unrelated bytes through unchanged, including binary/string identity.
    const next = enriched === text ? chunk : typeof chunk === "string" ? enriched : Buffer.from(enriched, "utf8");
    previous.call(output, next, encoding, callback);
  }) as Write;
  proc[PROCESS_MARK] = true;
}

export default async function (pi: ExtensionAPI): Promise<void> {
  // This bridge may be loaded manually in native Pi. Leave that host untouched.
  if (process.env.PI_GROK !== "1") return;
  const hookErrors: string[] = [];
  try {
    await installRunnerHooks();
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    hookErrors.push(detail);
    // Private internals can change on a Pi update; keep official RPC usable.
    console.error(`[pi-grok-rpc-compat] private host hook unavailable: ${detail}`);
  }
  try {
    await installGetCommandsStdoutIntercept();
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    hookErrors.push(detail);
    console.error(`[pi-grok-rpc-compat] completion hook unavailable: ${detail}`);
  }

  const prepareUi = (ctx: { mode: string; ui: unknown }) => {
    if (ctx.mode === "rpc" || hasRpcUiBridge(ctx.ui)) installRpcUiBridge(ctx.ui);
  };
  pi.on("session_start", (_event, ctx) => {
    prepareUi(ctx);
    resetRpcUiBridge(ctx.ui);
  });
  pi.on("agent_start", (_event, ctx) => { prepareUi(ctx); setWorkingActive(ctx.ui, true); });
  pi.on("agent_end", (_event, ctx) => { setWorkingActive(ctx.ui, false); });
  pi.registerCommand("pi-ui-capabilities", {
    description: "Pi extension UI: official RPC, native mappings, experimental host and unsupported calls",
    handler: async (_args, ctx) => {
      prepareUi(ctx);
      ctx.ui.notify([
        uiCapabilityReport(ctx.ui, ctx.mode),
        ...hookErrors.map((error) => `Private hook unavailable: ${error}`),
      ].join("\n"), "info");
    },
  });

  // User extensions such as loop.ts call sendUserMessage(), which re-enters
  // AgentSession.prompt with source="extension". Capture it before any later
  // extension handler, hand it to the Rust adapter, and mark it handled so it
  // does not enter Pi's private follow-up queue. The adapter owns ordering and
  // only sends the message back through source="rpc" when it is promoted.
  pi.on("input", (event, ctx) => {
    if (event.source !== "extension") return;
    ctx.ui.setStatus(
      QUEUE_STATUS_KEY,
      JSON.stringify({
        text: event.text,
        images: event.images ?? [],
        streamingBehavior: event.streamingBehavior ?? "followUp",
      }),
    );
    return { action: "handled" } as const;
  });
}
