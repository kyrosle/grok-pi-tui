import { randomUUID } from "node:crypto";
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
