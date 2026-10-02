import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";
import { logoutProviders } from "./providers.ts";
import { promptAuth, resolveRuntime } from "./runtime.ts";

export function registerLogoutCommand(pi: ExtensionAPI): void {
 pi.registerCommand("logout", {
  description: "Remove a stored Pi provider credential",
  handler: async (_args: string, ctx: ExtensionCommandContext) => {
   try {
    const runtime = resolveRuntime(ctx);
    const providers = await logoutProviders(runtime);
    if (!providers.length) { ctx.ui.notify("No stored Pi credentials", "info"); return; }
    const selected = await promptAuth(ctx, { type: "select", message: "Log out of Pi provider", options: providers.map(provider => ({ id: provider.id, label: provider.name })) });
    const provider = providers.find(provider => provider.id === selected);
    if (!provider) return;
    await runtime.logout(provider.id);
    await ctx.modelRegistry.refresh();
    ctx.ui.notify(`Logged out of ${provider.name}`, "info");
   } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (message !== "Login cancelled") ctx.ui.notify(`Logout failed: ${message}`, "error");
   }
  },
 });
}
