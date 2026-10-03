/** Public Pi provider/router and Codemode fixture. No network or real credentials. */
import { AssistantMessageEventStream, createProvider } from "@earendil-works/pi-ai";

export const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAEklEQVR4nGMQTN74Hx9mGBkKAEYiiQEl1No/AAAAAElFTkSuQmCC";
const providerId = "pi-model-fixture";
const usage = (input: number, output: number, cost: number) => ({ input, output, cacheRead: 0, cacheWrite: 0, totalTokens: input + output,
 cost: { input: cost, output: 0, cacheRead: 0, cacheWrite: 0, total: cost } });
const base = { provider: providerId, baseUrl: "http://127.0.0.1:9", input: ["text", "image"], cost: { input: 1, output: 1, cacheRead: 0, cacheWrite: 0 } };

export default function (pi: any) {
 let requests = 0;
 let context: any;
 pi.on("before_agent_start", (_event: any, ctx: any) => { requests = 0; context = ctx; });
 const wait = async (mode: string, signal?: AbortSignal) => {
  if (mode === "error") throw new Error("MODEL_FIXTURE_ERROR");
  if (mode !== "cancel") return;
  context?.ui.setStatus("fixture_models", "MODEL_FIXTURE_WAITING");
  await new Promise<void>((resolve, reject) => {
   const abort = () => { clearTimeout(timer); signal?.removeEventListener("abort", abort); reject(new Error("MODEL_FIXTURE_ABORTED")); };
   const timer = setTimeout(() => { signal?.removeEventListener("abort", abort); resolve(); }, 20_000);
   signal?.addEventListener("abort", abort, { once: true });
   if (signal?.aborted) abort();
  });
 };
 const stream = (model: any, transcript: any) => {
  const output = new AssistantMessageEventStream();
  const first = requests++ === 0;
  const user = [...transcript.messages].reverse().find((message: any) => message.role === "user");
  const text = typeof user?.content === "string" ? user.content : JSON.stringify(user?.content);
  const mode = text.includes("cancel") ? "cancel" : text.includes("error") ? "error" : "success";
  const code = `const classifier = await models.getModelOfType("classifier", "${providerId}", "judge");
const decision = await models.classify(classifier, {state:{mode:"${mode}"},questions:{approved:{type:"bool",instructions:"fixture",criteria:{true:"yes",false:"no"}}}});
text("MODEL_CLASSIFIER:" + JSON.stringify(decision));
if (decision.stopReason !== "stop") throw new Error(decision.errorMessage);
const painter = await models.getModelOfType("image", "${providerId}", "painter");
const result = await models.generateImages(painter, {input:[{type:"text",text:"${mode}"}]});
text("MODEL_IMAGE:" + JSON.stringify({stopReason:result.stopReason,usage:result.usage,errorMessage:result.errorMessage}));
if (result.stopReason !== "stop") throw new Error(result.errorMessage);
for (const block of result.output) if (block.type === "image") image(block);`;
  const content = first && text.includes("codemode")
   ? [{ type: "toolCall", id: "fixture-model-codemode", name: "codemode", arguments: { code } }]
   : [{ type: "text", text: "MODEL_FIXTURE_DONE" }];
  const message: any = { role: "assistant", content, api: model.api, provider: model.provider, model: model.id,
   stopReason: content[0].type === "toolCall" ? "toolUse" : "stop", timestamp: Date.now(), usage: usage(40, 30, 0.00007) };
  queueMicrotask(() => { output.push({ type: "start", partial: message }); output.push({ type: "done", reason: message.stopReason, message }); output.end(); });
  return output;
 };
 pi.registerProvider(createProvider({
  id: providerId,
  auth: { apiKey: { name: "Local fixture", async resolve() { return { auth: { apiKey: "fixture" } }; } } },
  models: [
   ...["small", "large"].map(id => ({ ...base, id, name: "Fixture " + id, api: "fixture-chat", reasoning: true,
    contextWindow: id === "small" ? 32768 : 65536, maxTokens: 4096 })),
   { ...base, id: "painter", name: "Fixture image", api: "fixture-images", type: "image", output: ["image"] },
   { ...base, id: "judge", name: "Fixture classifier", api: "fixture-classify", type: "classifier", contextWindow: 4096 },
  ],
  api: { stream, streamSimple: stream },
  images: { "fixture-images": { async generateImages(model: any, input: any, options: any) {
   await wait(input.input.find((block: any) => block.type === "text")?.text, options?.signal);
   return { api: model.api, provider: model.provider, model: model.id, output: [{ type: "image", data: PNG, mimeType: "image/png" }],
    stopReason: "stop", timestamp: Date.now(), usage: usage(100, 200, 0.0003) };
  } } },
  classifiers: { "fixture-classify": { async classify(model: any, input: any, options: any) {
   await wait(input.state.mode, options?.signal);
   return { api: model.api, provider: model.provider, model: model.id, answers: { approved: { type: "bool", probability: 0.9 } },
    stopReason: "stop", timestamp: Date.now(), usage: usage(20, 10, 0.00003) };
  } } },
 }));
 pi.registerVirtualModel({ provider: "pi-router-fixture", id: "auto", name: "Fixture router", thinkingLevels: ["low", "high"],
  contextWindow: 16384, maxTokens: 4096,
  route(request: any, ctx: any) {
   const sticky = request.failed ?? request.previous;
   if (request.reason !== "user" && sticky) return { model: sticky.model, thinkingLevel: sticky.thinkingLevel ?? "medium" };
   const large = JSON.stringify(request.messages).includes("route-large");
   return { model: ctx.modelRegistry.find(providerId, large ? "large" : "small")!, thinkingLevel: large ? "high" : "medium",
    state: { physical: large ? "large" : "small" } };
  },
 });
 pi.registerCommand("fixture-model-tree", { description: "Public tree navigation for router-state proof", async handler(target: string, ctx: any) {
  await ctx.navigateTree(target.trim(), { summarize: false });
 } });
 pi.registerCommand("fixture-model-sdk", { description: "Public image/classifier ModelRuntime paths", async handler(_args: string, ctx: any) {
  const painter = ctx.modelRegistry.findOfType("image", providerId, "painter");
  const judge = ctx.modelRegistry.findOfType("classifier", providerId, "judge");
  const results = [];
  for (const mode of ["success", "error", "cancel"]) {
   const controller = new AbortController();
   if (mode === "cancel") controller.abort();
   const image = await ctx.modelRegistry.generateImages(painter, { input: [{ type: "text", text: mode }] }, { signal: controller.signal });
   const classify = await ctx.modelRegistry.classify(judge, { state: { mode }, questions: { approved: { type: "bool", instructions: "fixture", criteria: { true: "yes", false: "no" } } } }, { signal: controller.signal });
   results.push({ mode, image: image.stopReason, classify: classify.stopReason, imageError: image.errorMessage, classifyError: classify.errorMessage,
    images: image.output.length, probability: classify.answers.approved?.probability, imageUsage: image.usage, classifierUsage: classify.usage });
  }
  ctx.ui.notify("MODEL_SDK:" + JSON.stringify(results), "info");
 } });
}
