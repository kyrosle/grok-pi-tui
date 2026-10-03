/** Real Pi pipeline fixture: deterministic provider, tool images and scoped dialogs. */
import { randomUUID } from "node:crypto";
import { appendFileSync } from "node:fs";
import { AssistantMessageEventStream, Type } from "@earendil-works/pi-ai";
import { promptAuth } from "../../../../../extensions/pi-grok-auth/runtime.ts";
import { AUTH_DIALOG_DONE_STATUS, AUTH_DIALOG_PREFIX } from "../../../../../extensions/pi-grok-auth/shared.ts";

export const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGMQTN74Hx9mGBkKAEYiiQEl1No/AAAAAElFTkSuQmCC";

export default function (pi: any) {
 let requests = 0;
 pi.on("before_agent_start", () => { requests = 0; });
 pi.registerTool({
  name: "fixture_note", label: "fixture_note", description: "Local native-render fixture",
  parameters: Type.Object({ text: Type.String() }), outputSchema: Type.Object({ value: Type.String(), image: Type.Any() }),
  async execute(_id: string, params: { text: string }) {
   if (process.env.PI_NATIVE_RENDER_TRACE) appendFileSync(process.env.PI_NATIVE_RENDER_TRACE, JSON.stringify(params) + "\n");
   return {
    content: [{ type: "text", text: "NATIVE_NESTED_OUTPUT:" + params.text }, { type: "image", data: PNG, mimeType: "image/png" }],
    structuredContent: { value: params.text, image: { type: "image", data: PNG, mimeType: "image/png" } },
    details: { structuredContent: { value: params.text }, resource: { uri: "fixture://render", mimeType: "text/plain", text: "NATIVE_RESOURCE" } },
   };
  },
 });
 pi.registerCommand("fixture-dialog", {
  description: "Exercise native scoped dialog cancellation without credentials",
  async handler(mode: string, ctx: any) {
   try {
    if (mode === "signal") {
     const controller = new AbortController();
     const timer = setTimeout(() => controller.abort(), 1200);
     try { await promptAuth(ctx, { type: "manual_code", message: "NATIVE_AUTH_SIGNAL", signal: controller.signal }); }
     finally { clearTimeout(timer); }
    } else {
     const scope = randomUUID();
     const title = AUTH_DIALOG_PREFIX + scope + ":NATIVE_AUTH_" + mode.toUpperCase();
     const timer = mode === "eof" ? setTimeout(() => process.exit(0), 1200) : undefined;
     try { await ctx.ui.input(title, "Fixture input", mode === "timeout" ? { timeout: 1200 } : undefined); }
     finally { if (timer) clearTimeout(timer); ctx.ui.setStatus(AUTH_DIALOG_DONE_STATUS, scope); }
    }
   } catch (error) {
    if (!String(error).includes("Login cancelled")) throw error;
   }
   ctx.ui.notify("NATIVE_AUTH_DONE:" + mode, "info");
  },
 });
 pi.registerProvider("pi-render", {
  baseUrl: "http://127.0.0.1:9", apiKey: "fixture", api: "openai-completions",
  models: [{ id: "local", name: "Native render fixture", reasoning: false, input: ["text", "image"], contextWindow: 32768, maxTokens: 4096,
   cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }],
  streamSimple(model: any, context: any) {
   const stream = new AssistantMessageEventStream();
   const first = requests++ === 0;
   const user = [...context.messages].reverse().find((message: any) => message.role === "user");
   const userText = typeof user?.content === "string" ? user.content : JSON.stringify(user?.content);
   const codemode = userText?.includes("render-codemode");
   const code = codemode
    ? `const result = await tools.fixture_note({text:"codemode"}); text("NATIVE_CODEMODE_OUTPUT"); text(result.value); image(result.image);`
    : `const result = await tool.fixture_note({text:"eval"}); console.log("NATIVE_EVAL_OUTPUT"); console.log(result.text);`;
   const content = first ? [{ type: "toolCall", id: codemode ? "fixture-codemode" : "fixture-eval", name: codemode ? "codemode" : "eval",
    arguments: codemode ? { code } : { language: "js", code, timeout: 10 } }]
    : [{ type: "text", text: "NATIVE_RENDER_DONE" }];
   const message: any = { role: "assistant", content, api: model.api, provider: model.provider, model: model.id,
    stopReason: first ? "toolUse" : "stop", timestamp: Date.now(),
    usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } };
   queueMicrotask(() => { stream.push({ type: "start", partial: message }); stream.push({ type: "done", reason: message.stopReason, message }); stream.end(); });
   return stream;
  },
 });
}
