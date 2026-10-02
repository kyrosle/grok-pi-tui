import { getAgentDir } from "@earendil-works/pi-coding-agent";
import { join } from "node:path";
import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";
import { findLoginProviderOptions, loginProviders } from "./providers.ts";
import { configureRadiusMcp, promptAuth, resolveRuntime } from "./runtime.ts";
import type { AuthType, ProviderOption } from "./shared.ts";

export function registerLoginCommand(pi: ExtensionAPI): void {
 pi.registerCommand("login", {
  description: "Log in to a Pi model provider",
  handler: async (args: string, ctx: ExtensionCommandContext) => {
   try {
    const runtime = resolveRuntime(ctx);
    let providers = args.trim() ? findLoginProviderOptions(runtime, args.trim()) : loginProviders(runtime);
    if (!providers.length) { ctx.ui.notify("No matching login provider", "warning"); return; }
    if (!args.trim()) {
     const methods = ["Sign in with an account", "Sign in with an API key"];
     const radius = providers.find(provider => provider.id === "radius" && provider.authType === "oauth");
     if (radius) methods.push("Sign in with Radius");
     const method = await ctx.ui.select("Select authentication method", methods);
     if (!method) return;
     const authType: AuthType = method === methods[1] ? "api_key" : "oauth";
     providers = method === "Sign in with Radius" ? [radius!] : providers.filter(provider => provider.authType === authType);
    }
    let provider: ProviderOption | undefined = providers[0];
    if (providers.length > 1) {
     const label = (item: ProviderOption) => `${item.name} · ${item.authType}${item.status ? " · configured" : ""}`;
     const selected = await ctx.ui.select("Select Pi provider", providers.map(label));
     provider = providers.find(item => label(item) === selected);
    }
    if (!provider) return;
    if (provider.method && !provider.method.login) {
     ctx.ui.notify(`${provider.name} is configured outside Pi`, "info"); return;
    }
    await runtime.login(provider.id, provider.authType, {
     prompt: prompt => promptAuth(ctx, prompt),
     notify: event => {
      if (event.type === "auth_url") ctx.ui.notify(`${event.instructions ?? "Open the sign-in link"}\n${event.url}`, "info");
      else if (event.type === "device_code") ctx.ui.notify(`${event.verificationUri ?? ""}\nCode: ${event.userCode ?? ""}`, "info");
      else if (event.type === "info" || event.type === "progress") ctx.ui.notify(event.message, "info");
     },
    });
    await ctx.modelRegistry.refresh();
    ctx.ui.notify(`Logged in to ${provider.name}`, "info");
    if (provider.id === "radius" && await ctx.ui.confirm("Radius MCP", "Configure Radius MCP in your Pi mcp.json?")) {
     configureRadiusMcp(join(getAgentDir(), "mcp.json"));
     if (process.env.PI_GROK_MCP === "1") await ctx.reload();
     else ctx.ui.notify("Radius MCP configured. Enable Pi MCP in F2 and restart to connect.", "info");
    }
   } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message !== "Login cancelled") ctx.ui.notify(`Login failed: ${message}`, "error");
   }
  },
 });
}
