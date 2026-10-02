import { AssistantMessageEventStream, getCurrentTools, Type } from "@earendil-works/pi-ai";

export default function (pi: any) {
 let requests = 0;
 let executions = 0;
 pi.on("before_agent_start", () => { requests = 0; executions = 0; });
 const schema = Type.Object({ text: Type.String() });
 for (const name of ["fixture_note", "fixture_blocked", "fixture_hidden"]) {
  pi.registerTool({
   name, label: name, description: "Local Pi pipeline fixture", parameters: schema,
   exposure: name === "fixture_hidden" ? "hidden" : "direct",
   async execute() { executions++; return { content: [{ type: "text", text: "fixture-ok" }], details: {} }; },
  });
 }
 pi.on("tool_call", (event: any) => event.toolName === "fixture_blocked" ? { block: true, reason: "fixture-policy" } : undefined);
 pi.registerProvider("pi-fixture", {
  baseUrl: "http://127.0.0.1:9", apiKey: "fixture", api: "openai-completions",
  models: [{ id: "local", name: "Local fixture", reasoning: false, input: ["text"], contextWindow: 32768, maxTokens: 4096,
   cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }],
  streamSimple(model: any, context: any) {
   const stream = new AssistantMessageEventStream();
   const tools = getCurrentTools(context.messages).map((tool: any) => tool.name);
   const first = requests++ === 0;
   const code = process.env.PI_FIXTURE_MCP_MODE ? `
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
    role: "assistant", content: first ? [{ type: "toolCall", id: "fixture-eval", name: "eval", arguments: { language: "js", code, timeout: 10 } }]
     : [{ type: "text", text: JSON.stringify({ fixtureComplete: true, executions, tools }) }],
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
