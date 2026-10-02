import { test, expect } from "bun:test";
import { mkdtempSync, readFileSync, writeFileSync, realpathSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import type { AssistantMessage } from "@earendil-works/pi-ai";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";

// Resolve the actual installed Pi package. Never replace the SDK with a fixture module.
function systemPiPackage(): string {
  if (process.env.PI_PACKAGE_DIR) return process.env.PI_PACKAGE_DIR;
  const binary = execFileSync(process.platform === "win32" ? "where" : "which", ["pi"], { encoding: "utf8" }).trim().split(/\r?\n/)[0];
  for (let path = dirname(realpathSync(binary)); path !== dirname(path); path = dirname(path)) {
    const manifest = join(path, "package.json");
    if (existsSync(manifest) && JSON.parse(readFileSync(manifest, "utf8")).name === "@earendil-works/pi-coding-agent") return path;
  }
  throw new Error("Installed Pi package not found; set PI_PACKAGE_DIR to the real SDK package");
}
const sdkRoot = systemPiPackage();
const sdkEntry = (specifier: string) => Bun.resolveSync(specifier, sdkRoot);
const { AssistantMessageEventStream, getCurrentTools } = await import(sdkEntry("@earendil-works/pi-ai"));
const { ModelRegistry, ModelRuntime, SessionManager } = await import(sdkEntry("@earendil-works/pi-coding-agent"));
const modulePath = join(mkdtempSync(join(tmpdir(), "pi-workflow-test-module-")), "index.mjs");
const source = readFileSync(join(import.meta.dir, "index.ts"), "utf8").replaceAll('"@earendil-works/pi-coding-agent"', JSON.stringify(sdkEntry("@earendil-works/pi-coding-agent")));
writeFileSync(modulePath, new Bun.Transpiler({ loader: "ts", target: "bun" }).transformSync(source));
const { default: register, normalizeCapability, normalizeWorkflowThinkingLevel } = await import(modulePath);

test("workflow capability and thinking tokens preserve restrictions", () => {
  for (const mode of ["read-only", "read-write", "execute", "all"] as const) expect(normalizeCapability(mode)).toBe(mode);
  expect(() => normalizeCapability("unsafe")).toThrow();
  expect(normalizeWorkflowThinkingLevel("none")).toBe("off");
  expect(normalizeWorkflowThinkingLevel("high")).toBe("high");
  expect(normalizeWorkflowThinkingLevel(null)).toBeUndefined();
});

test("real Pi Workflow SDK honors trust, model, capabilities, resume, fork, isolation and cancel drain", async () => {
  const directory = mkdtempSync(join(tmpdir(), "pi-workflow-sdk-"));
  const previous = { agentDir: process.env.PI_CODING_AGENT_DIR, workflows: process.env.PI_GROK_WORKFLOWS };
  process.env.PI_CODING_AGENT_DIR = directory;
  process.env.PI_GROK_WORKFLOWS = "1";
  try {
    const parentRuntime = await ModelRuntime.create({ authPath: join(directory, "auth.json"), modelsPath: null, refreshOnCreate: false });
    const registry = new ModelRegistry(parentRuntime);
    let calls = 0;
    let names: string[] = [];
    const contexts: string[] = [];
    let cancellationStarted!: () => void;
    const cancellationReady = new Promise<void>(resolve => { cancellationStarted = resolve; });
    registry.registerProvider("workflow-fixture", {
      baseUrl: "http://127.0.0.1:9", api: "openai-completions", apiKey: "fixture",
      models: ["local", "parent"].map(id => ({ id, name: "Workflow fixture", reasoning: true, input: ["text"], contextWindow: 32768, maxTokens: 4096,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } })),
      streamSimple(model, context, options) {
        calls++;
        expect(model.id).toBe("local");
        if (calls === 2) expect(JSON.stringify(context.messages)).toContain("sdk-child-ok");
        names = getCurrentTools(context.messages).map(tool => tool.name);
        const content = JSON.stringify(context.messages);
        contexts.push(content);
        const writes = content.includes("WORKTREE_WRITE") && !context.messages.some(message => message.role === "toolResult");
        const stream = new AssistantMessageEventStream();
        const message: AssistantMessage = {
          role: "assistant", content: writes ? [{ type: "toolCall", id: "workflow-write", name: "write", arguments: { path: "seed.txt", content: "child-change" } }] : [{ type: "text", text: calls === 1 ? "sdk-child-ok" : "sdk-child-resumed" }], api: model.api, provider: model.provider,
          model: model.id, stopReason: writes ? "toolUse" : "stop", timestamp: Date.now(),
          usage: { input: 2, output: 3, cacheRead: 0, cacheWrite: 0, totalTokens: 5,
            cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } },
        };
        if (content.includes("WAIT_FOR_CANCEL")) {
          options?.signal?.addEventListener("abort", () => {
            const error: AssistantMessage = { ...message, stopReason: "aborted", errorMessage: "fixture cancelled" };
            stream.push({ type: "error", reason: "aborted", error }); stream.end();
          }, { once: true });
          cancellationStarted();
        } else queueMicrotask(() => { stream.push({ type: "start", partial: message }); stream.push({ type: "done", reason: message.stopReason as "stop" | "toolUse", message }); stream.end(); });
        return stream;
      },
    });
    const commands = new Map<string, { handler: (args: string, ctx: ExtensionContext) => Promise<void> }>();
    register({ registerCommand(name, command) { commands.set(name, command as never); }, registerTool() {}, appendEntry() {}, on() {} } as unknown as ExtensionAPI);
    const parentManager = SessionManager.create(directory, join(directory, "parents"), { id: "12345678-1234-1234-1234-123456789abc" });
    parentManager.appendMessage({ role: "user", content: "Parent fixture context", timestamp: Date.now() });
    const ctx = { cwd: directory, modelRegistry: registry, model: registry.find("workflow-fixture", "parent"),
      isProjectTrusted: () => false, sessionManager: parentManager } as unknown as ExtensionContext;
    const trustPath = join(directory, "trust response.json");
    await commands.get("__pi_workflow_trust")!.handler(JSON.stringify({ responsePath: trustPath }), ctx);
    expect(JSON.parse(readFileSync(trustPath, "utf8"))).toEqual({ trusted: false, cwd: directory, sessionId: parentManager.getSessionId() });
    const requestPath = join(directory, "request.json");
    const responsePath = join(directory, "response.json");
    writeFileSync(requestPath, JSON.stringify({ id: "fixture-child", prompt: "Return the fixture response", run_id: "fixture-run",
      model: "workflow-fixture/local", capability_mode: "read-only", reasoning_effort: "high" }));
    await commands.get("__pi_workflow_spawn")!.handler(`--request ${requestPath} --response ${responsePath}`, ctx);
    const response = JSON.parse(readFileSync(responsePath, "utf8"));
    expect(response.success).toBe(true);
    expect(response.output).toBe("sdk-child-ok");
    expect(response.total_tokens_used).toBe(5);
    expect(calls).toBe(1);
    expect(names.sort()).toEqual(["find", "grep", "ls", "read"]);
    const parentBefore = readFileSync(parentManager.getSessionFile()!, "utf8");
    writeFileSync(requestPath, JSON.stringify({ id: "fixture-retry", prompt: "Correct the structured result", run_id: "fixture-run",
      resume_from: response.child_session_id, capability_mode: "read-only", reasoning_effort: "high" }));
    await commands.get("__pi_workflow_spawn")!.handler(`--request ${requestPath} --response ${responsePath}`, ctx);
    const resumed = JSON.parse(readFileSync(responsePath, "utf8"));
    expect(resumed.success).toBe(true);
    expect(resumed.output).toBe("sdk-child-resumed");
    expect(resumed.child_session_id).toBe(response.child_session_id);
    expect(resumed.total_tokens_used).toBe(5);
    expect(calls).toBe(2);
    expect(readFileSync(parentManager.getSessionFile()!, "utf8")).toBe(parentBefore);
    writeFileSync(requestPath, JSON.stringify({ id: "fixture-cross-run", prompt: "Must not execute", run_id: "other-run",
      resume_from: response.child_session_id, capability_mode: "read-only" }));
    await commands.get("__pi_workflow_spawn")!.handler(`--request ${requestPath} --response ${responsePath}`, ctx);
    const rejected = JSON.parse(readFileSync(responsePath, "utf8"));
    expect(rejected.success).toBe(false);
    expect(rejected.error).toContain("not found in this run");
    expect(calls).toBe(2);
    writeFileSync(requestPath, JSON.stringify({ id: "fixture-fork", prompt: "FORK_CONTEXT", run_id: "fixture-run",
      fork_context: true, model: "workflow-fixture/local", capability_mode: "read-only" }));
    await commands.get("__pi_workflow_spawn")!.handler(`--request ${requestPath} --response ${responsePath}`, ctx);
    expect(JSON.parse(readFileSync(responsePath, "utf8")).success).toBe(true);
    expect(contexts.at(-1)).toContain("Parent fixture context");
    expect(readFileSync(parentManager.getSessionFile()!, "utf8")).toBe(parentBefore);

    // The product Git worktree lifecycle runs only inside this disposable fixture repository.
    const git = (args: string[]) => execFileSync("git", args, { cwd: directory, stdio: "pipe", timeout: 30000 });
    git(["init"]);
    writeFileSync(join(directory, "seed.txt"), "head-content");
    git(["add", "seed.txt"]);
    git(["-c", "user.name=Workflow fixture", "-c", "user.email=fixture@example.invalid", "commit", "-m", "fixture"]);
    writeFileSync(join(directory, "seed.txt"), "parent-dirty");
    writeFileSync(requestPath, JSON.stringify({ id: "fixture-isolated", prompt: "WORKTREE_WRITE", run_id: "fixture-isolation",
      isolation_worktree: true, model: "workflow-fixture/local", capability_mode: "all" }));
    await commands.get("__pi_workflow_spawn")!.handler(`--request ${requestPath} --response ${responsePath}`, ctx);
    const isolated = JSON.parse(readFileSync(responsePath, "utf8"));
    expect(isolated.success).toBe(true);
    const worktree = isolated.output.split("Isolated worktree: ")[1];
    expect(readFileSync(join(worktree, "seed.txt"), "utf8")).toBe("child-change");
    expect(readFileSync(join(directory, "seed.txt"), "utf8")).toBe("parent-dirty");
    expect(isolated.total_tokens_used).toBe(10);
    expect(git(["worktree", "list", "--porcelain"]).toString()).toContain(worktree);

    writeFileSync(requestPath, JSON.stringify({ id: "fixture-cancel", prompt: "WAIT_FOR_CANCEL", run_id: "fixture-cancel",
      model: "workflow-fixture/local", capability_mode: "read-only" }));
    const pending = commands.get("__pi_workflow_spawn")!.handler(`--request ${requestPath} --response ${responsePath}`, ctx);
    await cancellationReady;
    await commands.get("__pi_workflow_cancel")!.handler(JSON.stringify({ "run-id": "fixture-cancel", response: trustPath }), ctx);
    expect(JSON.parse(readFileSync(trustPath, "utf8"))).toEqual({ drained: true });
    await pending;
    const cancelled = JSON.parse(readFileSync(responsePath, "utf8"));
    expect(cancelled.cancelled).toBe(true);
    expect(cancelled.success).toBe(false);
  } finally {
    if (previous.agentDir === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = previous.agentDir;
    if (previous.workflows === undefined) delete process.env.PI_GROK_WORKFLOWS; else process.env.PI_GROK_WORKFLOWS = previous.workflows;
  }
}, 30000);
