import { test, expect, mock } from "bun:test";
import { lstatSync, mkdtempSync, readFileSync, writeFileSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
mock.module("@earendil-works/pi-coding-agent", () => ({ getAgentDir: () => join(tmpdir(), "pi-auth-fixture") }));
const { registerLoginCommand } = await import("./login.ts");
import { registerLogoutCommand } from "./logout.ts";
import { configureRadiusMcp, promptAuth } from "./runtime.ts";
import { AUTH_DIALOG_DONE_STATUS, AUTH_DIALOG_PREFIX } from "./shared.ts";

test("native login handles provider method selection and copy code without custom TUI", async () => {
 const commands = new Map<string, any>();
 const statuses: unknown[] = [];
 const notices: string[] = [];
 const runtime = {
  getProviders: () => [{ id: "anthropic", name: "Anthropic", auth: { oauth: { login() {} } } }],
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
 await commands.get("login").handler("anthropic", ctx);
 expect(statuses.length).toBe(2);
 expect((statuses[0] as string[])[0]).toBe(AUTH_DIALOG_DONE_STATUS);
 expect(notices).toEqual(["Logged in to Anthropic"]);
});

test("cancelled authentication finishes its native dialog scope", async () => {
 const statuses: unknown[] = [];
 const ctx: any = { ui: { async input() { return undefined; }, setStatus(...args: unknown[]) { statuses.push(args); } } };
 await expect(promptAuth(ctx, { type: "manual_code", message: "Code" })).rejects.toThrow("Login cancelled");
 expect(statuses.length).toBe(1);
});

test("Radius config preserves other servers, unknown fields, and invalid source bytes", () => {
 const directory = mkdtempSync(join(tmpdir(), "pi-auth-config-"));
 const path = join(directory, "mcp.json");
 try {
  writeFileSync(path, JSON.stringify({ extra: 42, mcpServers: { radius: { url: "https://other.example/mcp" } } }));
  expect(configureRadiusMcp(path)).toBe(true);
  const saved = JSON.parse(readFileSync(path, "utf8"));
  expect(saved.extra).toBe(42);
  expect(saved.mcpServers.radius.url).toBe("https://other.example/mcp");
  expect(saved.mcpServers["radius-mcp-1"].auth).toEqual({ provider: "radius" });
  expect(configureRadiusMcp(path)).toBe(false);
  writeFileSync(path, "invalid JSON");
  expect(() => configureRadiusMcp(path)).toThrow("preserved");
  expect(readFileSync(path, "utf8")).toBe("invalid JSON");
 } finally { rmSync(directory, { recursive: true, force: true }); }
});

test("Radius config keeps an existing config symlink", () => {
 const directory = mkdtempSync(join(tmpdir(), "pi-auth-linked-config-"));
 const target = join(directory, "shared.json");
 const path = join(directory, "mcp.json");
 try {
  writeFileSync(target, "{}");
  symlinkSync(target, path);
  expect(configureRadiusMcp(path)).toBe(true);
  expect(lstatSync(path).isSymbolicLink()).toBe(true);
  expect(JSON.parse(readFileSync(target, "utf8")).mcpServers.radius.auth.provider).toBe("radius");
 } finally { rmSync(directory, { recursive: true, force: true }); }
});

test("logout uses native selection and Pi runtime", async () => {
 let handler: any; let loggedOut: string | undefined;
 registerLogoutCommand({ registerCommand(_name: string, command: any) { handler = command.handler; } } as any);
 await handler("", {
  modelRegistry: { runtime: { login() {}, async listCredentials() { return [{ providerId: "test", type: "oauth" }]; }, getProvider() { return { name: "Test" }; }, async logout(id: string) { loggedOut = id; } }, async refresh() {} },
  ui: { async select() { return "Test"; }, notify() {} },
 });
 expect(loggedOut).toBe("test");
});
