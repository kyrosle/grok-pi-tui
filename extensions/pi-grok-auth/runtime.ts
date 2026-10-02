import { randomUUID } from "node:crypto";
import { existsSync, lstatSync, mkdirSync, readFileSync, realpathSync, renameSync, rmdirSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import type { ExtensionCommandContext } from "@earendil-works/pi-coding-agent";
import { AUTH_DIALOG_PREFIX, AUTH_DIALOG_DONE_STATUS, type AuthPrompt, type ModelRegistryLike, type ModelRuntimeLike } from "./shared.ts";

export function resolveRuntime(ctx: ExtensionCommandContext): ModelRuntimeLike {
 const runtime = (ctx.modelRegistry as unknown as ModelRegistryLike).runtime;
 if (!runtime || typeof runtime.login !== "function") throw new Error("grok-pi requires Pi >= 1.0.0 ModelRuntime");
 return runtime;
}

export async function promptAuth(ctx: ExtensionCommandContext, prompt: AuthPrompt): Promise<string> {
 const scope = randomUUID();
 const title = `${AUTH_DIALOG_PREFIX}${scope}:${prompt.message}`;
 try {
  let value: string | undefined;
  if (prompt.type === "select") {
   const labels = prompt.options.map(option => option.label);
   const selected = await ctx.ui.select(title, labels, { signal: prompt.signal });
   value = prompt.options.find(option => option.label === selected)?.id;
  } else {
   value = await ctx.ui.input(title, prompt.placeholder, { signal: prompt.signal });
  }
  if (value === undefined) throw new Error("Login cancelled");
  return value;
 } finally {
  ctx.ui.setStatus(AUTH_DIALOG_DONE_STATUS, scope);
 }
}

/** Edit the documented Pi mcp.json format without touching credentials. */
export function configureRadiusMcp(path: string): boolean {
 const requestedPath = path;
 let existing;
 try { existing = lstatSync(path); }
 catch (error) { if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error; }
 // Resolve existing links before locking/writing so atomic replacement keeps
 // the user's link. A dangling link fails instead of replacing it.
 if (existing) path = realpathSync(path);
 mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
 const lock = `${path}.grok-pi-lock`;
 mkdirSync(lock, { mode: 0o700 });
 try {
  const before = existsSync(path) ? readFileSync(path, "utf8") : undefined;
  let root: Record<string, unknown>;
  try { root = before === undefined ? {} : JSON.parse(before); }
  catch { throw new Error("Pi mcp.json contains invalid JSON; configuration was preserved"); }
  if (!root || typeof root !== "object" || Array.isArray(root)) throw new Error("Pi mcp.json must be an object");
  const servers = root.mcpServers ?? {};
  if (!servers || typeof servers !== "object" || Array.isArray(servers)) throw new Error("Pi mcpServers must be an object");
  const entries = servers as Record<string, any>;
  const endpoint = "https://radius.pi.dev/mcp";
  let name = Object.keys(entries).find(key => typeof entries[key]?.url === "string" && entries[key].url.replace(/\/+$/u, "") === endpoint);
  if (name && entries[name].auth?.provider === "radius") return false;
  if (!name) {
   name = "radius";
   for (let ordinal = 1; Object.hasOwn(entries, name); ordinal++) name = `radius-mcp-${ordinal}`;
  }
  const server = { ...entries[name], url: endpoint, auth: { provider: "radius" } };
  delete server.oauth;
  entries[name] = server;
  root.mcpServers = entries;
  const temporary = `${lock}/mcp.json`;
  writeFileSync(temporary, JSON.stringify(root, null, 2) + "\n", { mode: 0o600 });
  const current = existsSync(path) ? readFileSync(path, "utf8") : undefined;
  if (current !== before) throw new Error("Pi mcp.json changed while configuring Radius; retry");
  if (existing && realpathSync(requestedPath) !== path) throw new Error("Pi mcp.json link changed while configuring Radius; retry");
  renameSync(temporary, path);
  return true;
 } finally {
  try { unlinkSync(`${lock}/mcp.json`); } catch { /* already renamed or not written */ }
  rmdirSync(lock);
 }
}
