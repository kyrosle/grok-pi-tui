import { test, expect, mock } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
mock.module("@earendil-works/pi-coding-agent", () => ({ getAgentDir: () => process.env.PI_CODING_AGENT_DIR ?? join(tmpdir(), "pi-auth-fixture") }));
const { registerLoginCommand } = await import("./login.ts");
import { registerLogoutCommand } from "./logout.ts";
import { promptAuth } from "./runtime.ts";
import { AUTH_DIALOG_DONE_STATUS, AUTH_DIALOG_PREFIX } from "./shared.ts";

test("native login handles provider method selection and copy code without custom TUI", async () => {
 const commands = new Map<string, any>();
 const statuses: unknown[] = [];
 const notices: string[] = [];
 const runtime = {
  getProviders: () => ["anthropic", "other"].map(id => ({ id, name: id === "anthropic" ? "Anthropic" : "Other", auth: { oauth: { login() {} } } })),
  getProviderAuthStatus: () => ({ configured: false }),
  async login(provider: string, method: string, interaction: any) {
   expect(provider).toBe("anthropic"); expect(method).toBe("oauth");
   expect(await interaction.prompt({ type: "select", message: "Method", options: [{ id: "copy", label: "Copy code" }] })).toBe("copy");
   expect(await interaction.prompt({ type: "manual_code", message: "Paste code" })).toBe("fixture-code");
  },
 };
 const ctx: any = {
  modelRegistry: { runtime, async refresh() {} },
  ui: {
   async select(title: string, options: string[]) { expect(title.startsWith(AUTH_DIALOG_PREFIX)).toBe(true); return options[0]; },
   async input(title: string) { expect(title.endsWith(":Paste code")).toBe(true); return "fixture-code"; },
   setStatus(key: string, value: string) { statuses.push([key, value]); },
   notify(message: string) { notices.push(message); },
  },
 };
 registerLoginCommand({ registerCommand(name: string, command: any) { commands.set(name, command); } } as any);
 await commands.get("login").handler("", ctx);
 expect(statuses.length).toBe(4);
 expect((statuses[0] as string[])[0]).toBe(AUTH_DIALOG_DONE_STATUS);
 expect(notices).toEqual(["Logged in to Anthropic"]);
});

test("cancelled authentication finishes its native dialog scope", async () => {
 const statuses: unknown[] = [];
 const ctx: any = { ui: { async input() { return undefined; }, setStatus(...args: unknown[]) { statuses.push(args); } } };
 await expect(promptAuth(ctx, { type: "manual_code", message: "Code" })).rejects.toThrow("Login cancelled");
 expect(statuses.length).toBe(1);
});

test("Radius delegates through the generic OAuth provider picker without changing MCP configuration", async () => {
 const directory = mkdtempSync(join(tmpdir(), "pi-auth-config-"));
 const path = join(directory, "mcp.json");
 const previous = process.env.PI_CODING_AGENT_DIR;
 process.env.PI_CODING_AGENT_DIR = directory;
 let handler: any, confirmations = 0, reloads = 0, refreshed = 0;
 const calls: unknown[] = [];
 const selections: string[][] = [];
 const notices: string[] = [];
 const original = JSON.stringify({ extra: 42, mcpServers: { radius: { url: "https://other.example/mcp" } } });
 try {
  writeFileSync(path, original);
  registerLoginCommand({ registerCommand(_name: string, command: any) { handler = command.handler; } } as any);
  await handler("", {
   modelRegistry: {
    runtime: {
     getProviders: () => ["anthropic", "radius"].map(id => ({ id, name: id === "radius" ? "Radius" : "Anthropic", auth: { oauth: { login() {} } } })),
     getProviderAuthStatus: () => ({ configured: false }),
     async login(provider: string, method: string, interaction: any) { calls.push([provider, method]); interaction.notify({ type: "info", message: "Pi OAuth completed" }); },
    },
    async refresh() { refreshed++; },
   },
   async reload() { reloads++; },
   ui: {
    async select(_title: string, options: string[]) { selections.push(options); return options.find(option => option.startsWith("Radius")) ?? options[0]; },
    async confirm() { confirmations++; return true; },
    setStatus() {}, notify(message: string) { notices.push(message); },
   },
  });
  expect(selections[0]).toEqual(["Sign in with an account", "Sign in with an API key"]);
  expect(selections[1]).toContain("Radius · oauth");
  expect(calls).toEqual([["radius", "oauth"]]);
  expect(refreshed).toBe(1);
  expect(confirmations).toBe(0); expect(reloads).toBe(0);
  expect(notices).toEqual(["Pi OAuth completed", "Logged in to Radius"]);
  expect(readFileSync(path, "utf8")).toBe(original);
 } finally {
  if (previous === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = previous;
  rmSync(directory, { recursive: true, force: true });
 }
});

test("explicit Radius OAuth login neither creates MCP configuration nor asks to reload it", async () => {
 const directory = mkdtempSync(join(tmpdir(), "pi-auth-linked-config-"));
 const path = join(directory, "mcp.json");
 const previous = { agent: process.env.PI_CODING_AGENT_DIR, mcp: process.env.PI_GROK_MCP };
 process.env.PI_CODING_AGENT_DIR = directory; process.env.PI_GROK_MCP = "1";
 let handler: any, confirmations = 0, reloads = 0;
 const calls: unknown[] = [];
 try {
  registerLoginCommand({ registerCommand(_name: string, command: any) { handler = command.handler; } } as any);
  await handler("radius", {
   modelRegistry: { runtime: {
    getProviders: () => [{ id: "radius", name: "Radius", auth: { oauth: { login() {} } } }],
    getProviderAuthStatus: () => ({ configured: false }),
    async login(provider: string, method: string) { calls.push([provider, method]); },
   }, async refresh() {} },
   async reload() { reloads++; },
   ui: { async confirm() { confirmations++; return true; }, setStatus() {}, notify() {} },
  });
  expect(calls).toEqual([["radius", "oauth"]]);
  expect(confirmations).toBe(0); expect(reloads).toBe(0);
  expect(existsSync(path)).toBe(false);
 } finally {
  if (previous.agent === undefined) delete process.env.PI_CODING_AGENT_DIR; else process.env.PI_CODING_AGENT_DIR = previous.agent;
  if (previous.mcp === undefined) delete process.env.PI_GROK_MCP; else process.env.PI_GROK_MCP = previous.mcp;
  rmSync(directory, { recursive: true, force: true });
 }
});

test("logout uses a scoped native selection and Pi runtime", async () => {
 let handler: any; let loggedOut: string | undefined;
 const statuses: unknown[] = [];
 registerLogoutCommand({ registerCommand(_name: string, command: any) { handler = command.handler; } } as any);
 await handler("", {
  modelRegistry: { runtime: { login() {}, async listCredentials() { return [{ providerId: "test", type: "oauth" }]; }, getProvider() { return { name: "Test" }; }, async logout(id: string) { loggedOut = id; } }, async refresh() {} },
  ui: { async select(title: string) { expect(title.startsWith(AUTH_DIALOG_PREFIX)).toBe(true); return "Test"; }, setStatus(...args: unknown[]) { statuses.push(args); }, notify() {} },
 });
 expect(loggedOut).toBe("test");
 expect(statuses).toHaveLength(1);
});
