import type { ExtensionAPI, ExtensionCommandContext } from "@earendil-works/pi-coding-agent";
import { logoutProviders } from "./providers.ts";
import { resolveRuntime } from "./runtime.ts";

export function registerLogoutCommand(pi: ExtensionAPI): void {
 pi.registerCommand("logout", {
  description: "Remove a stored Pi provider credential",
  handler: async (_args: string, ctx: ExtensionCommandContext) => {
   try {
    const runtime = resolveRuntime(ctx);
    const providers = await logoutProviders(runtime);
    if (!providers.length) { ctx.ui.notify("No stored Pi credentials", "info"); return; }
    const selected = await ctx.ui.select("Log out of Pi provider", providers.map(provider => provider.name));
    const provider = providers.find(provider => provider.name === selected);
    if (!provider) return;
    await runtime.logout(provider.id);
    await ctx.modelRegistry.refresh();
    ctx.ui.notify(`Logged out of ${provider.name}`, "info");
   } catch (error) {
    ctx.ui.notify(`Logout failed: ${error instanceof Error ? error.message : String(error)}`, "error");
   }
  },
 });
}
