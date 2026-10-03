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
 pi.registerProvider("pi-fixture", {
  baseUrl: "http://127.0.0.1:9", apiKey: "fixture", api: "openai-completions",
  models: [{ id: "local", name: "Local fixture", reasoning: false, input: ["text"], contextWindow: 32768, maxTokens: 4096,
   cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }],
  streamSimple(model: any, context: any) {
   const stream = new AssistantMessageEventStream();
   const tools = getCurrentTools(context.messages).map((tool: any) => tool.name);
   const first = requests++ === 0;
   const runtimeMode = process.env.PI_FIXTURE_RUNTIME_MODE;
   const previous = [...context.messages].reverse().find((message: any) => message.role === "toolResult" && message.toolName === "eval");
   const taskId = previous?.details?.taskId;
   const runtimeCode: Record<string, string> = {
    concurrency: `await Promise.all([0,1,2,3].map(index => tool.fixture_parallel({text:String(index),delay:150}))); await Promise.all([0,1,2].map(index => tool.fixture_sequential({text:String(index),delay:60}))); console.log("RUNTIME_CONCURRENCY_DONE");`,
    abort: `await tool.fixture_parallel({text:"abort",delay:30000}); console.log("RUNTIME_ABORT_UNEXPECTED_DONE");`,
    background: taskId ? `const result = await tool.get_task_output({task_ids:[${JSON.stringify(taskId)}],timeout_ms:5000}); console.log("RUNTIME_BACKGROUND_RESULT:" + JSON.stringify(result));` : `await tool.fixture_parallel({text:"background",delay:200}); console.log("RUNTIME_BACKGROUND_DONE");`,
    policy: `const declared = tools.list().map(item => item.name); const results = {}; for (const name of ["fixture_note", "fixture_parallel"]) { try { results[name] = await tool[name]({text:"policy",delay:1}); } catch(error) { results[name] = String(error); } } console.log("RUNTIME_POLICY:" + JSON.stringify({declared,results}));`,
   };
   const code = runtimeMode ? runtimeCode[runtimeMode] : process.env.PI_FIXTURE_MCP_MODE ? `
    if (${JSON.stringify(process.env.PI_FIXTURE_MCP_MODE)} !== "disabled") {
      try { await tools.waitFor("^mcp__local__echo$", ${process.env.PI_FIXTURE_MCP_MODE === "excluded" ? 500 : 10000}); }
      catch(e) { if (${JSON.stringify(process.env.PI_FIXTURE_MCP_MODE)} !== "excluded") throw e; }
    }
    const discovered = tools.search("mcp__local").map(item => item.name);
    const blocked = !discovered.includes("mcp__local__echo");
    let result;
    if (!blocked) result = await tool.mcp__local__echo({value:"native"});
    let resource;
    if (!blocked) resource = await tool.read_mcp_resource({server:"local",uri:"fixture://document"});
    console.log(JSON.stringify({discovered,blocked,result,resource}));
   ` : `
    const result = await tool.fixture_note({text:"valid"});
    const failures = [];
    for (const [name,args] of [["fixture_blocked",{text:"blocked"}],["fixture_note",{}],["fixture_hidden",{text:"hidden"}]]) {
      try { await tool[name](args); } catch(e) { failures.push(String(e)); }
    }
    console.log(JSON.stringify({result,failures}));
   `;
   const message: any = {
    role: "assistant", content: first || (runtimeMode === "background" && requests === 2) ? [{ type: "toolCall", id: "fixture-eval-" + requests, name: "eval", arguments: { language: "js", code, timeout: runtimeMode === "abort" ? 60 : 10, is_background: runtimeMode === "background" && requests === 1 } }]
     : [{ type: "text", text: JSON.stringify({ fixtureComplete: true, executions, tools, runtime }) }],
    api: model.api, provider: model.provider, model: model.id, stopReason: first || (runtimeMode === "background" && requests === 2) ? "toolUse" : "stop", timestamp: Date.now(),
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
