/** Real grok-pi PTY + installed xterm screen model; no inference/OAuth/clipboard. */
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, createReadStream, mkdirSync, mkdtempSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const binary = realpathSync(process.env.GROK_PI_BINARY ?? join(ROOT, "target/debug/grok-pi"));
const pi = realpathSync(process.env.PI_BIN ?? execFileSync("which", ["pi"], { encoding: "utf8" }).trim());
const require = createRequire(pi);
const { Terminal } = require("@xterm/headless");
const artifacts = process.env.PI_NATIVE_PTY_ARTIFACTS ?? mkdtempSync(join(tmpdir(), "grok-pi-native-pty-"));
mkdirSync(artifacts, { recursive: true });
const binaryHash = createHash("sha256");
for await (const chunk of createReadStream(binary)) binaryHash.update(chunk);
const binarySha256 = binaryHash.digest("hex");
const results: unknown[] = [];

async function run(mode: string, directory = mkdtempSync(join(tmpdir(), "grok-pi-pty-state-"))) {
 const evalOnly = mode === "eval" || mode === "minimal";
 const grok = join(directory, "grok");
 mkdirSync(grok, { recursive: true });
 const configPath = join(grok, "config.toml");
 if (mode !== "settings-reopen" && mode !== "settings-rollback")
  writeFileSync(configPath, `[ui]\npi_subagents = false\npi_todo = false\npi_workflows = false\npi_bash = false\npi_eval = "v2"\npi_eval_v2_only = ${evalOnly}\ngroup_tool_verbs = false\n[ui.pi_builtin_tools]\ncodemode = ${mode === "codemode"}\n`);
 const argv = [binary, "--offline", "--no-approve", "--no-session", "--no-extensions", "--no-skills", "--no-context-files",
  "--provider", "pi-render", "--model", "local", "--extension", join(ROOT, "crates/codegen/pi-grok-adapter/tests/fixtures/pi_render_tools.ts")];
 if (mode === "codemode") argv.push("--extension", "builtin:codemode");
 if (mode === "minimal") argv.push("--minimal");
 const env = { ...process.env, GROK_HOME: grok, GROK_PROJECT_DIR: ".grok-pi", PI_CODING_AGENT_DIR: join(directory, "pi"),
  PI_OFFLINE: "1", PI_TELEMETRY: "0", GROK_CONTEXTUAL_HINTS: "0", PI_GROK_REMOTE_TUI: "0", PI_GROK_NATIVE_COMMANDS: "0", PI_GROK_EVAL_VERSION: "v2",
  PI_GROK_EVAL_V2_ONLY: evalOnly ? "1" : "0", PI_GROK_EVAL_MCP: "0", PI_GROK_RPC_WATCHDOG: "0", PI_NATIVE_RENDER_TRACE: join(directory, "render-tools.jsonl"),
  TERM: "xterm-256color", TERM_PROGRAM: "xterm", COLORTERM: "truecolor" };
 for (const key of Object.keys(env)) if (key.endsWith("_API_KEY") || key.endsWith("_TOKEN") || key === "AWS_ACCESS_KEY_ID" || key === "AWS_SECRET_ACCESS_KEY") delete env[key];
 const bridge = spawn("python3", ["-u", join(ROOT, "crates/codegen/pi-grok-adapter/tests/fixtures/pty_bridge.py"),
  JSON.stringify({ argv, cwd: directory, rows: 40, cols: 120 })], { env, stdio: ["pipe", "pipe", "pipe"] });
 bridge.stdin.on("error", () => {}); // Cleanup may race the already-reaped native child.
 const terminal = new Terminal({ cols: 120, rows: 40, allowProposedApi: true });
 const raw: Buffer[] = [];
 let diagnostics = "";
 let nativeExit: number | undefined;
 let failed = false;
 let writes = Promise.resolve();
 const send = (data: string) => bridge.stdin.write(JSON.stringify({ type: "write", data: Buffer.from(data).toString("base64") }) + "\n");
 terminal.onData(send);
 bridge.stderr.on("data", bytes => { diagnostics += bytes.toString(); });
 const lines = createInterface({ input: bridge.stdout });
 lines.on("line", line => {
  const event = JSON.parse(line);
  if (event.type === "exit") nativeExit = event.status;
  if (event.type === "output") {
   const bytes = Buffer.from(event.data, "base64"); raw.push(bytes);
   writes = writes.then(() => new Promise<void>(done => terminal.write(bytes, done)));
  }
 });
 const screen = () => Array.from({ length: 40 }, (_, row) => terminal.buffer.active.getLine(terminal.buffer.active.viewportY + row)?.translateToString(true) ?? "").join("\n");
 const wait = async (predicate: (text: string) => boolean, label: string, seconds = 20) => {
  const deadline = Date.now() + seconds * 1000;
  while (Date.now() < deadline) {
   await writes;
   const text = screen();
   if (predicate(text)) return text;
   if (nativeExit !== undefined) throw new Error(`${mode}: native exit ${nativeExit} before ${label}\n${text}\n${diagnostics}`);
   await new Promise(done => setTimeout(done, 25));
  }
  throw new Error(`${mode}: timeout ${label}\n${screen()}\n${diagnostics}`);
 };
 try {
  await wait(text => text.includes("grok-pi") || text.includes("local") || text.includes("What"), "native startup");
  await new Promise(done => setTimeout(done, 1000));
  if (mode.startsWith("settings-")) {
   send("/new\r");
   await wait(text => text.includes("Native render fixture") && !text.includes("New worktree"), "native session for F2 settings");
   const rowValue = (text: string, value: string) => text.split("\n").some(line => line.includes("Group tool calls") && new RegExp(`\\b${value}\\b`).test(line));
   const openSetting = async (value: string) => {
    send("\x1bOQ");
    await wait(text => text.includes("Settings"), "native F2 settings");
    send("/");
    await wait(text => text.includes("type to filter"), "native F2 search focus");
    for (const char of "group tool calls") {
     send(char);
     await new Promise(done => setTimeout(done, 30));
    }
    const focused = (text: string) => text.split("\n").some(line => line.includes("› Group tool calls"));
    await wait(text => focused(text) && rowValue(text, value), `native F2 focused boolean ${value}`);
    send("\r");
    return wait(text => focused(text) && text.includes("Space toggle"), "native F2 search commit");
   };
   const before = readFileSync(configPath, "utf8");
   await openSetting(mode === "settings-save" ? "off" : "on");
   if (mode === "settings-save") {
    send(" ");
    await wait(text => rowValue(text, "on") && /group_tool_verbs\s*=\s*true/.test(readFileSync(configPath, "utf8")), "native F2 save and disk readback");
    send("\x1bOQ");
    await wait(text => !text.includes("Settings"), "native F2 close");
    writeFileSync(join(artifacts, `${mode}-reopen.txt`), await openSetting("on"));
   } else if (mode === "settings-rollback") {
    chmodSync(grok, 0o500);
    send(" ");
    send("\x1bOQ");
    const rolledBack = await wait(text => text.includes("Could not save group_tool_verbs") && !text.includes("Settings"), "native F2 failed save notice");
    if (readFileSync(configPath, "utf8") !== before) throw new Error("Failed native F2 save changed config.toml");
    writeFileSync(join(artifacts, `${mode}-rollback.txt`), rolledBack);
    writeFileSync(join(artifacts, `${mode}-reopen.txt`), await openSetting("on"));
    chmodSync(grok, 0o700);
   } else if (!/group_tool_verbs\s*=\s*true/.test(before)) throw new Error("F2 boolean was not persisted across native processes");
   send("\x1bOQ");
   await wait(text => !text.includes("Settings"), "native F2 close before quit");
  } else if (mode === "eval" || mode === "codemode" || mode === "minimal") {
   send(`render-${mode === "minimal" ? "eval" : mode}\r`);
   let final = await wait(text => text.includes("NATIVE_RENDER_DONE") && text.includes("Worked for"), "native turn completion", 30);
   const calls = readFileSync(join(directory, "render-tools.jsonl"), "utf8").trim().split("\n").map(line => JSON.parse(line));
   if (calls.length !== 1 || calls[0].text !== (mode === "codemode" ? "codemode" : "eval"))
    throw new Error("Native tool did not execute exactly once: " + JSON.stringify(calls));
   // The turn-complete task restores prompt focus after its last text frame.
   await new Promise(done => setTimeout(done, 250));
   if (mode === "codemode") {
    // Select the actual native card, then expand its regular scrollback body.
    const row = final.split("\n").findIndex(line => line.includes("Codemode"));
    if (row < 0) throw new Error("native Codemode card missing\n" + final);
    send("\t");
    await wait(text => text.includes("Space:prompt"), "native scrollback focus");
    const col = Math.max(1, final.split("\n")[row].indexOf("Codemode") + 2);
    send(`\x1b[<0;${col};${row + 1}M\x1b[<0;${col};${row + 1}m\x1b[C`);
    final = await wait(text => text.includes("NATIVE_CODEMODE_OUTPUT"), "native expanded Codemode output");
    if (!final.includes("[Open Image]")) throw new Error("native image open affordance missing\n" + final);
   } else if ((final.match(/fixture_note/g) ?? []).length !== 1) throw new Error("native nested card missing/duplicated\n" + final);
   if (mode === "minimal") {
    for (const cols of [64, 120]) {
     terminal.resize(cols, 40);
     bridge.stdin.write(JSON.stringify({ type: "resize", rows: 40, cols }) + "\n");
     await new Promise(done => setTimeout(done, 500));
     final = await wait(text => text.includes("NATIVE_RENDER_DONE") && text.includes("minimal · /help"), `native minimal history after resize ${cols}`);
     if ((final.match(/NATIVE_RENDER_DONE/g) ?? []).length !== 1) throw new Error("Minimal resize duplicated committed history\n" + final);
     writeFileSync(join(artifacts, `${mode}-resize-${cols}.txt`), final);
    }
   }
   writeFileSync(join(artifacts, `${mode}-complete.txt`), final);
   if (mode === "codemode") send(" "); // Native scrollback focus hint returns to the prompt.
  } else {
   send(`/fixture-dialog ${mode}\r`);
   const title = "NATIVE_AUTH_" + mode.toUpperCase();
   const opened = await wait(text => text.includes(title), "native auth QuestionView");
   writeFileSync(join(artifacts, `${mode}-open.txt`), opened);
   const dismissed = await wait(text => !text.includes(title) && (mode === "eof" || text.includes("NATIVE_AUTH_DONE:" + mode)), "native auth dialog retraction");
   writeFileSync(join(artifacts, `${mode}-dismissed.txt`), dismissed);
  }
  send("/quit\r");
  const deadline = Date.now() + 10000;
  while (nativeExit === undefined && Date.now() < deadline) await new Promise(done => setTimeout(done, 25));
  if (nativeExit !== 0) throw new Error(`${mode}: native quit status ${nativeExit}\n${screen()}`);
  results.push({ mode, passed: true, nativeExit, stateDirectory: directory });
  return directory;
 } catch (error) {
  failed = true;
  writeFileSync(join(artifacts, `${mode}-failure.txt`), String(error));
  throw error;
 } finally {
  chmodSync(grok, 0o700);
  writeFileSync(join(artifacts, `${mode}.ansi`), Buffer.concat(raw));
  writeFileSync(join(artifacts, `${mode}-final.txt`), screen());
  bridge.stdin.write(JSON.stringify({ type: "close" }) + "\n");
  bridge.stdin.end();
  await new Promise(done => { if (bridge.exitCode !== null) done(undefined); else bridge.once("exit", done); });
  terminal.dispose();
  if (failed) console.error("PTY artifacts:", artifacts);
 }
}

const selected = process.argv.slice(2);
const modes = selected.length ? selected : ["eval", "codemode", "signal", "timeout", "eof"];
for (const mode of modes) {
 if (mode === "settings") {
  const directory = await run("settings-save");
  await run("settings-reopen", directory);
  await run("settings-rollback", directory);
  continue;
 }
 if (!["eval", "codemode", "signal", "timeout", "eof", "minimal"].includes(mode)) throw new Error("Unknown PTY fixture: " + mode);
 await run(mode);
}
const finalHash = createHash("sha256");
for await (const chunk of createReadStream(binary)) finalHash.update(chunk);
if (finalHash.digest("hex") !== binarySha256) throw new Error("Native binary changed during PTY cases; rerun against one build");
writeFileSync(join(artifacts, "report.json"), JSON.stringify({ binary, binarySha256,
 pi, screenEmulator: require.resolve("@xterm/headless"), results }, null, 2) + "\n");
console.log(JSON.stringify({ passed: true, artifacts, cases: results.length }));
