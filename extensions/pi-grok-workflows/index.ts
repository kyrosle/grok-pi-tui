/**
 * Pi spawn executor for upstream xai-workflow host (grok-pi).
 *
 * Does NOT interpret Rhai. The Rust host runs xai-workflow; this extension only
 * implements SpawnAgent via createAgentSession (same kernel as pi-grok-subagents).
 *
 * Bridge protocol (file-based, matches hidden command style):
 *   /__pi_workflow_spawn --request <path> --response <path>
 *   /__pi_workflow_cancel --run-id <id>
 */
import { randomUUID } from "node:crypto";
import { readFileSync, writeFileSync, mkdirSync, mkdtempSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  createAgentSession,
  DefaultResourceLoader,
  getAgentDir,
  ModelRuntime,
  SessionManager,
  SettingsManager,
  type AgentSession,
  type ExtensionAPI,
  type ExtensionContext,
} from "@earendil-works/pi-coding-agent";

const SPAWN_COMMAND = "__pi_workflow_spawn";
const CANCEL_COMMAND = "__pi_workflow_cancel";
const TRUST_COMMAND = "__pi_workflow_trust";
const BRIDGE_TYPE = "pi-grok-workflow/v1";

type CapabilityMode = "read-only" | "read-write" | "execute" | "all";

const CAPABILITY_TOOLS: Record<CapabilityMode, string[]> = {
  "read-only": ["read", "grep", "find", "ls"],
  "read-write": ["read", "grep", "find", "ls", "edit", "write"],
  execute: ["read", "bash", "grep", "find", "ls"],
  all: ["read", "grep", "find", "ls", "edit", "write", "bash"],
};

type SpawnRequest = {
  id: string;
  prompt: string;
  description?: string;
  subagent_type?: string;
  parent_session_id?: string;
  resume_from?: string;
  model?: string;
  reasoning_effort?: string;
  capability_mode?: string;
  isolation_worktree?: boolean;
  fork_context?: boolean;
  run_id: string;
};

type SpawnResponse = {
  success: boolean;
  output: string;
  error?: string;
  cancelled: boolean;
  child_session_id: string;
  total_tokens_used: number;
  duration_ms: number;
  backgrounded: boolean;
};

type WorkflowChild = { parentSessionId: string; session?: AgentSession; cancelled: boolean; done: Promise<void>; finish: () => void };
const activeByRun = new Map<string, Set<WorkflowChild>>();

async function drainWorkflowChildren(children: WorkflowChild[]): Promise<void> {
  for (const child of children) child.cancelled = true;
  await Promise.all(children.map(async child => {
    if (child.session) await child.session.abort();
    await child.done;
  }));
}

/** Preserve the legacy stock token; Pi owns model-specific clamping. */
export function normalizeWorkflowThinkingLevel(effort?: string | null):
  NonNullable<Parameters<typeof createAgentSession>[0]>["thinkingLevel"] {
  if (effort == null) return undefined;
  const level = effort === "none" ? "off" : effort;
  switch (level) {
    case "off": case "minimal": case "low": case "medium": case "high": case "xhigh": case "max":
      return level;
    default: throw new Error(`invalid workflow thinking level: ${effort}`);
  }
}

function parseArgs(args: string): Record<string, string> {
  if (args.trimStart().startsWith("{")) return JSON.parse(args) as Record<string, string>;
  const out: Record<string, string> = {};
  const tokens = args.trim().split(/\s+/).filter(Boolean);
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (!t.startsWith("--")) continue;
    const key = t.slice(2);
    const val = tokens[i + 1];
    if (val && !val.startsWith("--")) {
      out[key] = val;
      i++;
    } else {
      out[key] = "1";
    }
  }
  return out;
}

export function normalizeCapability(raw: string | undefined): CapabilityMode {
  const v = (raw ?? "all").toLowerCase();
  if (v === "read-only" || v === "read-write" || v === "execute" || v === "all") {
    return v;
  }
  throw new Error(`invalid workflow capability mode: ${raw}`);
}

/** Reuse public provider registrations with Pi 1.0's canonical SDK runtime. */
export async function createWorkflowModelRuntime(ctx: ExtensionContext, agentDir = getAgentDir()): Promise<ModelRuntime> {
  const runtime = await ModelRuntime.create({
    authPath: join(agentDir, "auth.json"),
    modelsPath: join(agentDir, "models.json"),
    modelsStorePath: join(agentDir, "models-store.json"),
    refreshOnCreate: false,
  });
  for (const id of ctx.modelRegistry.getRegisteredProviderIds()) {
    const native = ctx.modelRegistry.getRegisteredNativeProvider(id);
    if (native) runtime.registerNativeProvider(native);
    else {
      const config = ctx.modelRegistry.getRegisteredProviderConfig(id);
      if (config) runtime.registerProvider(id, config);
    }
  }
  return runtime;
}

function childSessionManager(request: SpawnRequest, ctx: ExtensionContext): { manager: SessionManager; cwd: string; worktree?: string } {
  if (!/^[a-zA-Z0-9_-]{1,128}$/.test(request.run_id)) throw new Error("invalid workflow run id");
  const sessionDir = join(ctx.sessionManager.getSessionDir(), "workflow-children", ctx.sessionManager.getSessionId(), request.run_id);
  mkdirSync(sessionDir, { recursive: true });
  if (request.resume_from) {
    const path = SessionManager.findById(ctx.cwd, request.resume_from, sessionDir);
    if (!path) throw new Error(`workflow child session not found in this run: ${request.resume_from}`);
    const manager = SessionManager.open(path, sessionDir);
    const cwd = manager.getCwd();
    return { manager, cwd, worktree: cwd !== ctx.cwd ? cwd : undefined };
  }
  const parent = ctx.sessionManager.getSessionFile();
  if (request.fork_context && !parent) throw new Error("workflow fork requires a persisted parent session");
  let cwd = ctx.cwd;
  let worktree: string | undefined;
  if (request.isolation_worktree) {
    const path = mkdtempSync(join(tmpdir(), "pi-workflow-worktree-"));
    worktree = join(path, "checkout");
    execFileSync("git", ["worktree", "add", "--detach", worktree, "HEAD"], { cwd, stdio: "pipe", timeout: 30000 });
    cwd = worktree;
  }
  if (request.fork_context) {
    return { manager: SessionManager.forkFrom(parent!, cwd, sessionDir), cwd, worktree };
  }
  return { manager: SessionManager.create(cwd, sessionDir, { parentSession: ctx.sessionManager.getSessionFile() }), cwd, worktree };
}

function selectedModel(ctx: ExtensionContext, key?: string) {
  if (!key) return ctx.model;
  const matches = ctx.modelRegistry.getAll().filter(model =>
    key === `${model.provider}/${model.id}` || key === `${model.provider}::${model.id}` || key === model.id);
  if (matches.length !== 1) throw new Error(`workflow model must match one available Pi model: ${key}`);
  return matches[0];
}

function lastAssistantText(session: AgentSession): string {
  const branch = session.messages ?? [];
  for (let i = branch.length - 1; i >= 0; i--) {
    const m = branch[i] as { role?: string; content?: unknown };
    if (m.role !== "assistant") continue;
    if (typeof m.content === "string") return m.content;
    if (Array.isArray(m.content)) {
      return m.content
        .map((b) => {
          if (typeof b === "string") return b;
          if (b && typeof b === "object" && "text" in b) {
            return String((b as { text?: string }).text ?? "");
          }
          return "";
        })
        .join("");
    }
  }
  return "";
}

function writeResponse(path: string, body: SpawnResponse): void {
  writeFileSync(path, JSON.stringify(body), "utf8");
}

export default function (pi: ExtensionAPI): void {
  if (process.env.PI_GROK !== "1" && process.env.PI_GROK_WORKFLOWS !== "1") {
    return;
  }

  let scopeClosing = false;
  pi.on("session_shutdown", async (_event, ctx) => {
    const parentSessionId = ctx.sessionManager.getSessionId();
    scopeClosing = true;
    pi.appendEntry(BRIDGE_TYPE, { version: 1, kind: "scope_closing", parentSessionId });
    const children = [...activeByRun.values()].flatMap(set => [...set]).filter(child => child.parentSessionId === parentSessionId);
    await drainWorkflowChildren(children);
  });

  pi.registerCommand(TRUST_COMMAND, {
    description: "Internal: report Pi's current project trust",
    hidden: true,
    handler: async (args, ctx) => {
      const { responsePath } = JSON.parse(args) as { responsePath: string };
      writeFileSync(responsePath, JSON.stringify({
        trusted: ctx.isProjectTrusted(), cwd: ctx.cwd, sessionId: ctx.sessionManager.getSessionId(),
      }), "utf8");
    },
  });

  pi.registerCommand(SPAWN_COMMAND, {
    description: "Internal: spawn workflow agent for xai-workflow host",
    hidden: true,
    handler: async (args, ctx) => {
      const parsed = parseArgs(args);
      const requestPath = parsed.request;
      const responsePath = parsed.response;
      if (!requestPath || !responsePath) {
        ctx.ui.notify("workflow spawn requires --request and --response", "error");
        return;
      }

      let request: SpawnRequest;
      try {
        request = JSON.parse(readFileSync(requestPath, "utf8")) as SpawnRequest;
      } catch (e) {
        writeResponse(responsePath, {
          success: false,
          output: "",
          error: `invalid request file: ${e instanceof Error ? e.message : String(e)}`,
          cancelled: false,
          child_session_id: "",
          total_tokens_used: 0,
          duration_ms: 0,
          backgrounded: false,
        });
        return;
      }

      const started = Date.now();
      const id = request.id || randomUUID();
      let finish!: () => void;
      const child: WorkflowChild = { parentSessionId: ctx.sessionManager.getSessionId(), cancelled: false, done: new Promise(resolve => { finish = resolve; }), finish: () => finish() };
      const set = activeByRun.get(request.run_id) ?? new Set<WorkflowChild>();
      set.add(child); activeByRun.set(request.run_id, set);

      try {
        if (request.parent_session_id && request.parent_session_id !== ctx.sessionManager.getSessionId()) throw new Error("workflow parent session changed before child startup");
        if (scopeClosing) throw new Error("workflow session scope is closing");
        const capabilityMode = normalizeCapability(request.capability_mode);
        const agentDir = getAgentDir();
        const modelRuntime = await createWorkflowModelRuntime(ctx, agentDir);
        if (scopeClosing || child.cancelled) throw new Error("workflow child cancelled during startup");
        const model = request.resume_from && !request.model ? undefined : selectedModel(ctx, request.model?.trim());
        if (!request.resume_from && !model) throw new Error("no Pi model is selected");
        const { manager, cwd, worktree } = childSessionManager(request, ctx);
        const settingsManager = SettingsManager.create(cwd, agentDir);
        const resourceLoader = new DefaultResourceLoader({
          cwd,
          agentDir,
          noExtensions: true,
          noSkills: true,
          noPromptTemplates: true,
          noThemes: true,
          noContextFiles: true,
          systemPromptOverride: () =>
            "You are a focused workflow worker. Complete the assigned prompt and stop.",
          appendSystemPromptOverride: () => [],
        });
        await resourceLoader.reload();

        const { session } = await createAgentSession({
          cwd,
          agentDir,
          sessionManager: manager,
          settingsManager,
          modelRuntime,
          model,
          thinkingLevel: normalizeWorkflowThinkingLevel(request.reasoning_effort),
          tools: [...CAPABILITY_TOOLS[capabilityMode]],
          resourceLoader,
        });
        await session.bindExtensions({});

        child.session = session;
        const initialTokens = session.getSessionStats().tokens.total;
        if (child.cancelled) throw new Error("workflow child cancelled during startup");
        try {
          await session.prompt(request.prompt);
        } catch (e) {
          writeResponse(responsePath, {
            success: false,
            output: "",
            error: e instanceof Error ? e.message : String(e),
            cancelled: child.cancelled,
            child_session_id: session.sessionId,
            total_tokens_used: 0,
            duration_ms: Date.now() - started,
            backgrounded: false,
          });
          return;
        }

        const output = lastAssistantText(session);
        const final = session.messages.findLast(message => message.role === "assistant");
        const failed = final?.role === "assistant" && (final.stopReason === "error" || final.stopReason === "aborted");
        const success = !child.cancelled && !failed;
        writeResponse(responsePath, {
          success,
          error: child.cancelled ? "workflow child cancelled" : failed && final?.role === "assistant" ? final.errorMessage ?? "workflow child failed" : undefined,
          output: worktree ? `${output}\n\nIsolated worktree: ${worktree}` : output,
          cancelled: child.cancelled,
          child_session_id: session.sessionId,
          total_tokens_used: session.getSessionStats().tokens.total - initialTokens,
          duration_ms: Date.now() - started,
          backgrounded: false,
        });

        // Notify parent bridge (appendEntry — never sendMessage during parent stream).
        pi.appendEntry(BRIDGE_TYPE, {
          version: 1,
          kind: "agent_finished",
          runId: request.run_id,
          agentId: id,
          childSessionId: session.sessionId,
          success,
        });
      } catch (e) {
        writeResponse(responsePath, {
          success: false,
          output: "",
          error: e instanceof Error ? e.message : String(e),
          cancelled: child.cancelled,
          child_session_id: "",
          total_tokens_used: 0,
          duration_ms: Date.now() - started,
          backgrounded: false,
        });
      } finally {
        child.session?.dispose();
        child.finish();
        set.delete(child);
        if (set.size === 0) activeByRun.delete(request.run_id);
      }
    },
  });

  pi.registerCommand(CANCEL_COMMAND, {
    description: "Internal: cancel workflow child agents",
    hidden: true,
    handler: async (args) => {
      const parsed = parseArgs(args);
      const runId = parsed["run-id"] ?? parsed.run_id;
      if (!runId) return;
      try {
        const children = [...(activeByRun.get(runId) ?? [])];
        await drainWorkflowChildren(children);
        if (parsed.response) writeFileSync(parsed.response, JSON.stringify({ drained: true }), "utf8");
      } catch (error) {
        if (parsed.response) writeFileSync(parsed.response, JSON.stringify({ drained: false, error: String(error) }), "utf8");
        else throw error;
      }
    },
  });

  // Model-facing entry: host owns Rhai; this tool waits for the terminal outcome
  // via a response file so the parent turn gets the real report (not fire-and-forget).
  pi.registerTool({
    name: "workflow",
    label: "Workflow",
    description:
      "Launch or manage an upstream-compatible Rhai workflow (host-owned). Prefer named project workflows under .grok-pi/workflows (or $GROK_PROJECT_DIR). Blocks until the run finishes and returns the report.",
    parameters: {
      type: "object",
      properties: {
        name: { type: "string", description: "Workflow name or op (pause|resume|stop)" },
        args: { type: "string", description: "JSON args or objective text" },
      },
      required: ["name"],
    } as never,
    async execute(_toolCallId, params, signal, _onUpdate, ctx: ExtensionContext) {
      const name = String((params as { name?: string }).name ?? "");
      const args = String((params as { args?: string }).args ?? "");
      const { join } = await import("node:path");
      const { tmpdir } = await import("node:os");
      const { existsSync, readFileSync, mkdirSync } = await import("node:fs");
      const dir = join(tmpdir(), "pi-grok-workflow-tool");
      try {
        mkdirSync(dir, { recursive: true });
      } catch {
        /* ignore */
      }
      const responsePath = join(dir, `tool-${randomUUID()}.resp.json`);

      pi.appendEntry(BRIDGE_TYPE, {
        version: 1,
        kind: "tool_request",
        name,
        args,
        responsePath,
        cwd: ctx.cwd,
        parentSessionId: ctx.sessionManager.getSessionId(),
        projectTrusted: ctx.isProjectTrusted(),
      });

      const started = Date.now();
      const maxMs = 4 * 60 * 60 * 1000; // 4h — deep-research can be long
      while (!existsSync(responsePath)) {
        if (signal?.aborted) {
          return {
            content: [
              {
                type: "text",
                text: `Workflow \`${name}\` aborted while waiting for host outcome.`,
              },
            ],
            details: { name, args, aborted: true },
          };
        }
        if (Date.now() - started > maxMs) {
          return {
            content: [
              {
                type: "text",
                text: `Workflow \`${name}\` timed out waiting for host outcome after ${maxMs}ms. Check /workflows or run status.`,
              },
            ],
            details: { name, args, timedOut: true },
          };
        }
        await new Promise((r) => setTimeout(r, 250));
      }

      let body: {
        text?: string;
        error?: string;
        runId?: string;
        outcome?: unknown;
        op?: string;
        ok?: boolean;
      } = {};
      try {
        body = JSON.parse(readFileSync(responsePath, "utf8")) as typeof body;
      } catch (e) {
        return {
          content: [
            {
              type: "text",
              text: `Workflow host response unreadable: ${e instanceof Error ? e.message : String(e)}`,
            },
          ],
          details: { name, args, responsePath },
        };
      }

      const text =
        body.text ??
        (body.error
          ? `Workflow failed: ${body.error}`
          : body.op
            ? `Workflow ${body.op}: ${JSON.stringify(body)}`
            : JSON.stringify(body, null, 2));

      return {
        content: [{ type: "text", text }],
        details: { name, args, runId: body.runId, outcome: body.outcome, responsePath },
      };
    },
  });
}
