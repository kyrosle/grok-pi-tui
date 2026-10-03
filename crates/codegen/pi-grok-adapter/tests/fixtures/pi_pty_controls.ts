/** Native runtime/package proof observer and deterministic retry provider. */
import { appendFileSync } from "node:fs";
import { AssistantMessageEventStream } from "@earendil-works/pi-ai";

export default function (pi: any) {
 const trace = (value: any) => {
  if (process.env.PI_PTY_CONTROL_TRACE) appendFileSync(process.env.PI_PTY_CONTROL_TRACE, JSON.stringify(value) + "\n");
 };
 pi.registerCommand("fixture-registry", { description: "Observe the real live Pi registry", async handler(_args: string, ctx: any) {
  const commands = pi.getCommands().map((command: any) => command.name);
  const state = { commands, hasPackage: commands.includes("fixture-package"), idle: ctx.isIdle() };
  trace({ event:"registry", ...state });
  ctx.ui.notify("PTY_REGISTRY_HAS_PACKAGE:" + state.hasPackage, "info");
 } });
 pi.registerCommand("fixture-session-state", { description: "Observe public Pi session and active branch after resource reload", async handler(phase: string, ctx: any) {
  const state = { event:"session-state", phase:phase.trim(), sessionId:ctx.sessionManager.getSessionId(), sessionFile:ctx.sessionManager.getSessionFile(),
   leafId:ctx.sessionManager.getLeafId(), branchIds:ctx.sessionManager.getBranch().map((entry: any) => entry.id),
   provider:ctx.model?.provider, model:ctx.model?.id, thinking:pi.getThinkingLevel() };
  trace(state);
  ctx.ui.notify("PTY_SESSION_STATE:" + state.phase, "info");
 } });
 pi.registerProvider("pi-pty", {
  baseUrl:"http://127.0.0.1:9", apiKey:"fixture", api:"openai-completions",
  models:[{id:"local",name:"Native controls fixture",reasoning:false,input:["text"],contextWindow:32768,maxTokens:4096,
   cost:{input:0,output:0,cacheRead:0,cacheWrite:0}}],
  streamSimple(model: any, context: any) {
   const output = new AssistantMessageEventStream();
   const retry = JSON.stringify(context.messages).includes("pty-retry");
   const message: any = {role:"assistant",content:retry ? [] : [{type:"text",text:"PTY_CONTROL_DONE"}],api:model.api,provider:model.provider,model:model.id,
    stopReason:retry ? "error" : "stop",errorMessage:retry ? "429 Rate limit exceeded (native retry fixture)" : undefined,timestamp:Date.now(),
    usage:{input:0,output:0,cacheRead:0,cacheWrite:0,totalTokens:0,cost:{input:0,output:0,cacheRead:0,cacheWrite:0,total:0}}};
   trace({event:"model-request",retry});
   queueMicrotask(() => {
    if (retry) output.push({type:"error",reason:"error",error:message});
    else {output.push({type:"start",partial:message});output.push({type:"done",reason:"stop",message});}
    output.end();
   });
   return output;
  },
 });
}
