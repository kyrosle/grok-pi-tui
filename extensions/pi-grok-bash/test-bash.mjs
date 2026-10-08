import assert from "node:assert/strict";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import register from "./index.ts";

const cwd = await mkdtemp(join(tmpdir(), "pi-grok-bash-regression-"));
const text = (result) => result.content.filter((item) => item.type === "text").map((item) => item.text).join("\n");
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
function harness() {
  const tools = new Map();
  const handlers = new Map();
  register({ events: { emit() {} }, sendMessage() {}, appendEntry() {},
    registerTool(tool) { tools.set(tool.name, tool); },
    on(name, fn) { handlers.set(name, fn); } });
  const ctx = { cwd, ui: { setStatus() {} }, sessionManager: { getSessionId: () => "bash-regression", getSessionFile: () => undefined } };
  return { tools,
    run(name, args, signal) { return tools.get(name).execute(name + Date.now(), args, signal, undefined, ctx); },
    close() { handlers.get("session_shutdown")?.(); } };
}
process.env.PI_GROK_BASH = "1";
process.env.PI_GROK_EVAL_VERSION = "v2";
process.env.PI_GROK_EVAL_V2_ONLY = "1";
process.env.PI_GROK_EVAL_MCP = "1";
delete process.env.PI_GROK_BUILTIN_TOOLS;
delete process.env.PI_GROK_EXCLUDE_TOOLS;
delete process.env.PI_GROK_BASH_CONTROL_META;
process.env.PI_GROK_BASH_MAX_WAIT_MINS = "0";
let h;
try {
  h = harness();
  assert.deepEqual([...h.tools.keys()], ["bash", "get_task_output", "wait_tasks", "kill_task"]);
  assert(!h.tools.get("bash").description.toLowerCase().includes("eval"));
  assert(text(await h.run("bash", { command: "printf BASH_OK", task_name: "执行测试" })).includes("BASH_OK"));
  const task = JSON.parse(text(await h.run("bash", { command: "sleep 0.05; printf BACKGROUND_OK", task_name: "后台测试", is_background: true })));
  const waited = JSON.parse(text(await h.run("wait_tasks", { task_ids: [task.task_id, "missing"], mode: "wait_all", timeout_ms: 1000 })));
  assert.deepEqual(waited.task_not_found, ["missing"]);
  assert.equal(waited.results[0].status, "completed");
  assert(waited.results[0].output.includes("BACKGROUND_OK"));
  const live = JSON.parse(text(await h.run("bash", { command: "sleep 30 & echo $! > nested.pid; wait", task_name: "取消测试", is_background: true })));
  let childPid;
  for (let i = 0; i < 100 && !childPid; i++) {
    try { childPid = Number(await readFile(join(cwd, "nested.pid"), "utf8")); } catch { await sleep(10); }
  }
  assert(childPid > 0);
  assert.equal(JSON.parse(text(await h.run("kill_task", { task_id: live.task_id }))).outcome, "killed");
  assert.equal(JSON.parse(text(await h.run("get_task_output", { task_ids: [live.task_id], timeout_ms: 1000 }))).status, "cancelled");
  for (let i = 0; i < 100; i++) {
    try { process.kill(childPid, 0); await sleep(10); } catch { break; }
  }
  assert.throws(() => process.kill(childPid, 0), /ESRCH/);

  await assert.rejects(h.run("bash", { command: "sleep 5", task_name: "超时测试", timeout: 0.05 }), /timed out|timeout/i);
  h.close();
  process.env.PI_GROK_BASH_MAX_WAIT_MINS = "0.0005";
  h = harness();
  const promoted = JSON.parse(text(await h.run("bash", { command: "sleep 0.12; printf PROMOTED_OK", task_name: "转后台测试" })));
  assert.equal(promoted.status, "running");
  const done = JSON.parse(text(await h.run("wait_tasks", { task_ids: [promoted.task_id], mode: "wait_all", timeout_ms: 1000 })));
  assert.equal(done.results[0].status, "running"); // blocking wait is capped by the configured window
  h.close();
  process.env.PI_GROK_BASH = "0";
  assert.equal(harness().tools.size, 0);
  process.env.PI_GROK_BASH = "1";
  process.env.PI_GROK_EXCLUDE_TOOLS = "bash";
  assert.equal(harness().tools.size, 0);
  console.log("PASS: Bash foreground/background/missing IDs/cancel/timeout/auto-background/CLI exclusion; retired Eval flags have no effect");
} finally {
  h?.close();
  await rm(cwd, { recursive: true, force: true });
}
