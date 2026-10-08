import { AssistantMessageEventStream, getCurrentTools, Type } from "@earendil-works/pi-ai";

export default function (pi: any) {
 let requests = 0;
 let executions = 0;
 const runtime = { parallel: 0, sequential: 0, maxParallel: 0, maxSequential: 0, aborted: 0, completed: 0 };
 pi.on("before_agent_start", () => { requests = 0; executions = 0; });
 const schema = Type.Object({ text: Type.String() });
 for (const name of ["fixture_note", "fixture_blocked", "fixture_hidden"]) {
  pi.registerTool({
   name, label: name, description: "Local Pi pipeline fixture", parameters: schema,
   exposure: name === "fixture_hidden" ? "hidden" : "direct",
   async execute() { executions++; return { content: [{ type: "text", text: "fixture-ok" }], details: {} }; },
  });
 }
 for (const [name, mode] of [["fixture_parallel", "parallel"], ["fixture_sequential", "sequential"]] as const) {
  pi.registerTool({
   name, label: name, description: "Local abort/concurrency pipeline fixture", executionMode: mode,
   parameters: Type.Object({ text: Type.String(), delay: Type.Number() }),
   async execute(_id: string, params: { text: string; delay: number }, signal: AbortSignal) {
    runtime[mode]++;
    const max = mode === "parallel" ? "maxParallel" : "maxSequential";
    runtime[max] = Math.max(runtime[max], runtime[mode]);
    try {
     await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => { signal.removeEventListener("abort", aborted); resolve(); }, params.delay);
      const aborted = () => { clearTimeout(timer); signal.removeEventListener("abort", aborted); runtime.aborted++; reject(new Error("fixture-signal-aborted")); };
      if (signal.aborted) aborted();
      else signal.addEventListener("abort", aborted, { once: true });
     });
     runtime.completed++;
     return { content: [{ type: "text", text: "fixture-slow:" + params.text }], details: {} };
    } finally { runtime[mode]--; }
   },
  });
 }
 pi.registerCommand("fixture-runtime-stats", {
  description: "Inspect actual local tool callback state without another model turn",
  async handler(_args: string, ctx: any) { ctx.ui.notify("PI_RUNTIME_STATS:" + JSON.stringify(runtime), "info"); },
 });
 pi.on("tool_call", (event: any) => event.toolName === "fixture_blocked" ? { block: true, reason: "fixture-policy" } : undefined);
 pi.registerCommand("fixture-tools", { handler: async (_args: string, ctx: any) => ctx.ui.notify("FIXTURE_TOOLS:" + JSON.stringify(pi.getAllTools().map((tool: any) => tool.name)), "info") });
 pi.registerProvider("pi-fixture", {
  baseUrl: "http://127.0.0.1:9", apiKey: "fixture", api: "openai-completions",
  models: [{ id: "local", name: "Local fixture", reasoning: false, input: ["text"], contextWindow: 32768, maxTokens: 4096,
   cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }],
  streamSimple(model: any, context: any) {
   const stream = new AssistantMessageEventStream();
   const tools = getCurrentTools(context.messages).map((tool: any) => tool.name);
   const first = requests++ === 0;
   const code = process.env.PI_FIXTURE_MCP_MODE ? `
    const discovered = ALL_TOOLS.filter(item => item.name.startsWith("mcp__local")).map(item => item.name);
    const blocked = !discovered.includes("mcp__local__echo");
    let result, resource;
    if (!blocked) { result = await tools.mcp__local__echo({value:"native"}); resource = await tools.read_mcp_resource({server:"local",uri:"fixture://document"}); }
    text(JSON.stringify({discovered,blocked,result,resource}));
   ` : `
    const result = await tools.fixture_note({text:"valid"});
    const failures = [];
    for (const [name,args] of [["fixture_blocked",{text:"blocked"}],["fixture_note",{}],["fixture_hidden",{text:"hidden"}]]) {
      try { await tools[name](args); } catch(e) { failures.push(String(e)); }
    }
    text(JSON.stringify({result,failures}));
   `;
   const message: any = {
    role: "assistant", content: first ? [{ type: "toolCall", id: "fixture-codemode-" + requests, name: "codemode", arguments: { code } }]
     : [{ type: "text", text: JSON.stringify({ fixtureComplete: true, executions, tools, runtime }) }],
    api: model.api, provider: model.provider, model: model.id, stopReason: first ? "toolUse" : "stop", timestamp: Date.now(),
    usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } },
   };
   queueMicrotask(() => {
    stream.push({ type: "start", partial: message });
    stream.push({ type: "done", reason: message.stopReason, message });
    stream.end();
   });
   return stream;
  },
 });
}
