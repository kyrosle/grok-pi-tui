import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";
import { findLoginProviderOptions, loginProviders } from "./providers.ts";
import { promptAuth, resolveRuntime } from "./runtime.ts";
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
     const method = await promptAuth(ctx, { type: "select", message: "Select authentication method", options: methods.map(label => ({ id: label, label })) });
     if (!method) return;
     const authType: AuthType = method === methods[1] ? "api_key" : "oauth";
     providers = providers.filter(provider => provider.authType === authType);
    }
    let provider: ProviderOption | undefined = providers[0];
    if (providers.length > 1) {
     const label = (item: ProviderOption) => `${item.name} · ${item.authType}${item.status ? " · configured" : ""}`;
     const selected = await promptAuth(ctx, { type: "select", message: "Select Pi provider", options: providers.map(item => ({ id: label(item), label: label(item) })) });
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
   } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message !== "Login cancelled") ctx.ui.notify(`Login failed: ${message}`, "error");
   }
  },
 });
}
