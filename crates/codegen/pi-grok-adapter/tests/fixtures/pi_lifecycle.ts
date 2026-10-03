import { AssistantMessageEventStream } from "@earendil-works/pi-ai";

export default function (pi: any) {
 pi.on("input", (event: any) => event.text === "fixture-handled" ? { action: "handled" } : { action: "continue" });
 pi.registerCommand("fixture-no-run", { description: "Consumed command fixture", handler: async () => {} });
 pi.registerProvider("pi-lifecycle", {
  api: "openai-completions", apiKey: "fixture", baseUrl: "http://127.0.0.1:9",
  models: [{ id: "local", name: "Lifecycle fixture", reasoning: false, input: ["text"], contextWindow: 32768,
   maxTokens: 1024, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }],
  streamSimple(model: any) {
   const stream = new AssistantMessageEventStream();
   const message: any = { role: "assistant", content: [{ type: "text", text: "LIFECYCLE_COMPLETE" }],
    api: model.api, provider: model.provider, model: model.id, stopReason: "stop", timestamp: Date.now(),
    usage: { input: 2, output: 2, cacheRead: 0, cacheWrite: 0, totalTokens: 4,
     cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } };
   setTimeout(() => { stream.push({ type: "start", partial: message });
    stream.push({ type: "done", reason: "stop", message }); stream.end(); }, 160);
   return stream;
  },
 });
}
