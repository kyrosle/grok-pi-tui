/** Real grok-pi PTY + installed xterm screen model; no inference/OAuth/clipboard. */
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, createReadStream, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
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
 const controls = mode === "runtime" || mode === "packages";
 const codemode = mode === "codemode" || mode === "models";
 const grok = join(directory, "grok");
 mkdirSync(grok, { recursive: true });
 const configPath = join(grok, "config.toml");
 if (mode !== "settings-reopen" && mode !== "settings-rollback")
  writeFileSync(configPath, `[ui]\npi_subagents = false\npi_todo = false\npi_workflows = false\npi_bash = false\npi_eval = "v2"\npi_eval_v2_only = ${evalOnly}\ngroup_tool_verbs = false\n[ui.pi_builtin_tools]\ncodemode = ${codemode}\n`);
 if (mode === "product-surface")
  writeFileSync(configPath, readFileSync(configPath, "utf8").replace("[ui]\n", "[ui]\nvoice_keybind_enabled = true\nvoice_stt_language = 'en'\n"));
 const fixture = mode === "models" ? "pi_models.ts" : controls ? "pi_pty_controls.ts" : "pi_render_tools.ts";
 const provider = mode === "models" ? "pi-router-fixture" : controls ? "pi-pty" : "pi-render";
 const argv = [binary, "--offline", "--no-approve", "--no-session", "--no-extensions", "--no-skills", "--no-context-files",
  "--provider", provider, "--model", mode === "models" ? "auto" : "local", "--extension", join(ROOT, "crates/codegen/pi-grok-adapter/tests/fixtures", fixture)];
 if (codemode) argv.push("--extension", "builtin:codemode");
 if (mode === "models") argv.push("--thinking", "high");
 if (mode === "minimal") argv.push("--minimal");
 const env = { ...process.env, GROK_HOME: grok, GROK_PROJECT_DIR: ".grok-pi", PI_CODING_AGENT_DIR: join(directory, "pi"),
  PI_OFFLINE: "1", PI_TELEMETRY: "0", GROK_CONTEXTUAL_HINTS: "0", PI_GROK_REMOTE_TUI: mode === "remote-ui" ? "1" : "0", PI_GROK_NATIVE_COMMANDS: "0", PI_GROK_EVAL_VERSION: "v2",
  PI_GROK_EVAL_V2_ONLY: evalOnly ? "1" : "0", PI_GROK_EVAL_MCP: "0", PI_GROK_RPC_WATCHDOG: "0", PI_NATIVE_RENDER_TRACE: join(directory, "render-tools.jsonl"),
  TERM: "xterm-256color", TERM_PROGRAM: "xterm", COLORTERM: "truecolor" };
 env["PI_PTY_CONTROL_TRACE"] = join(directory, "control-registry.jsonl");
 if (mode === "product-surface") env["GROK_VOICE_MODE"] = "1";
 const packageSource = join(directory, "local-package");
 if (mode === "packages") {
  // The host recomputes admission on reload. Persist Pi's session so an official
  // respawn can retain history and the active leaf when extension paths change.
  argv.splice(argv.indexOf("--no-extensions"), 1);
  argv.splice(argv.indexOf("--no-session"), 1);
  argv.push("--session-dir", join(directory, "sessions"));
  mkdirSync(join(packageSource, "extensions"), { recursive:true });
  writeFileSync(join(packageSource, "package.json"), JSON.stringify({ name:"pty-local-package", pi:{extensions:["extensions/index.ts"]} }));
  writeFileSync(join(packageSource, "extensions/index.ts"), `import {appendFileSync} from "node:fs"; export default function(pi) {
   appendFileSync(process.env.PI_PTY_CONTROL_TRACE, JSON.stringify({event:"package-factory"})+"\\n");
   pi.registerCommand("fixture-package",{description:"Native local package fixture",handler:async (_args,ctx)=>{
    appendFileSync(process.env.PI_PTY_CONTROL_TRACE, JSON.stringify({event:"package-command"})+"\\n");ctx.ui.notify("PTY_PACKAGE_COMMAND", "info");}});
  }`);
  const wrapper = join(directory, "pi-package-wrapper.py");
  writeFileSync(wrapper, readFileSync(join(ROOT, "crates/codegen/pi-grok-adapter/tests/fixtures/pi_pty_package_cli.py")));
  chmodSync(wrapper, 0o700);
  argv.push("--pi-bin", wrapper);
  env["PI_BIN"] = wrapper;
  env["PI_PTY_ACTUAL_PI"] = pi;
  env["PI_PTY_PACKAGE_TRACE"] = join(directory, "package-cli.jsonl");
 }
 if (mode === "remote-ui") {
  const path = join(ROOT, "extensions/pi-grok-rpc-compat/rpc.test.py");
  const source = execFileSync("python3", ["-c", "import ast,sys; tree=ast.parse(open(sys.argv[1]).read()); print(next(ast.literal_eval(node.value) for node in tree.body if isinstance(node,ast.Assign) and any(isinstance(target,ast.Name) and target.id=='FIXTURE' for target in node.targets)))", path], {encoding:"utf8"});
  const remoteFixture = join(directory, "remote-fixture.ts");
  writeFileSync(remoteFixture, source);
  argv.push("--extension", remoteFixture);
 }
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
 const trace = (path: string) => existsSync(path) ? readFileSync(path, "utf8").trim().split("\n").filter(Boolean).map(line => JSON.parse(line)) : [];
 const save = (name: string, text: string) => writeFileSync(join(artifacts, `${mode}-${name}.txt`), text);
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
  if (mode === "product-surface") {
   send("/new\r");
   await wait(text => text.includes("Native render fixture") && !text.includes("New worktree"), "Pi session for product-surface checks");
   const before = readFileSync(configPath, "utf8");
   const searchSettings = async (query: string) => {
    send("\x1bOQ");
    await wait(text => text.includes("Settings"), "native F2 open");
    send("/");
    await wait(text => text.includes("type to filter"), "native F2 search");
    send("\x1b[200~" + query + "\x1b[201~");
   };
   for (const query of ["voice", "retention"]) {
    await searchSettings(query);
    save("no-" + query, await wait(text => text.includes("No matches for") && text.includes(query), "removed product setting " + query));
    send("\x1bOQ");
    await wait(text => !text.includes("Settings"), "native F2 close");
   }
   await searchSettings("group tool calls");
   save("ui-retained", await wait(text => text.includes("Group tool calls") && !text.includes("No matches for"), "retained terminal UI setting"));
   send("\x1bOQ");
   await wait(text => !text.includes("Settings"), "native F2 close");
   if (readFileSync(configPath, "utf8") !== before) throw new Error("Product surface inspection changed existing configuration");
   if (existsSync(join(grok, "auth.json"))) throw new Error("External Pi startup created Grok auth state");
  } else if (controls) {
   send("/new\r");
   let readySince = 0;
   await wait(text => {
    const ready = text.includes("Native controls fixture") && /\b0\s*\/\s*32K\b/.test(text)
     && !text.includes("New worktree") && !text.includes("Starting session");
    if (!ready) { readySince = 0; return false; }
    readySince ||= Date.now();
    return Date.now() - readySince >= 250;
   }, "native control session ready and stable");
   if (mode === "runtime") {
    const latest = (text: string) => text.slice(text.lastIndexOf("Pi runtime"));
    for (const [command, compaction, retry] of [["", "on", "on"], ["retry off", "on", "off"], ["compaction off", "off", "off"],
      ["retry on", "off", "on"], ["compaction on", "on", "on"]]) {
     send(`/pi-runtime ${command}\r`);
     save(command.replaceAll(" ", "-") || "status", await wait(text => latest(text).includes(`auto compaction: ${compaction}`)
      && latest(text).includes(`retry policy (configured): ${retry}`), "native runtime " + command));
    }
    const settings = JSON.parse(readFileSync(join(directory, "pi/settings.json"), "utf8"));
    if (settings.retry?.enabled !== true || settings.compaction?.enabled !== true) throw new Error("Runtime RPC settings did not persist: " + JSON.stringify(settings));
    send("pty-retry\r");
    save("retry-active", await wait(text => text.includes("Retrying") && trace(env["PI_PTY_CONTROL_TRACE"]).some(row => row.event === "model-request" && row.retry), "actual native retry"));
    send("/pi-runtime cancel-retry\r");
    save("cancel-retry", await wait(text => latest(text).includes("streaming: false") && latest(text).includes("retry policy (configured): on"), "native retry cancellation"));
   } else {
    terminal.resize(200, 40);
    bridge.stdin.write(JSON.stringify({type:"resize",cols:200,rows:40})+"\n");
    await wait(text => text.split("\n").some(line => line.includes("╭") && (line.match(/─/g) ?? []).length > 150), "wide native prompt after resize");
    send("\x1b[200~pty-history\x1b[201~");
    await wait(text => text.includes("pty-history"), "native composer contains persistent-history prompt");
    send("\r");
    save("history-before", await wait(text => text.includes("PTY_CONTROL_DONE") && text.includes("Worked for"), "persistent native history"));
    const sessionState = async (phase: string) => {
     send(`/fixture-session-state ${phase}\r`);
     await wait(text => text.includes("PTY_SESSION_STATE:" + phase)
      && trace(env["PI_PTY_CONTROL_TRACE"]).some(row => row.event === "session-state" && row.phase === phase), "actual Pi session state " + phase);
     return trace(env["PI_PTY_CONTROL_TRACE"]).find(row => row.event === "session-state" && row.phase === phase);
    };
    const before = await sessionState("before-install");
    if (!before.sessionFile || !existsSync(before.sessionFile) || !before.leafId) throw new Error("Package reload fixture did not persist its history and leaf");
    const assertSession = async (phase: string) => {
     const after = await sessionState(phase);
     for (const key of ["sessionId", "sessionFile", "leafId", "branchIds", "provider", "model", "thinking"])
      if (JSON.stringify(after[key]) !== JSON.stringify(before[key])) throw new Error("Resource reload changed Pi " + key + ": " + JSON.stringify({before,after}));
     if (!screen().includes("PTY_CONTROL_DONE")) throw new Error("Resource reload removed native conversation history");
    };
    const open = async () => {
     send("/pi-config\r");
     return wait(text => text.includes("Live Pi registry") && !text.includes("Checking live Pi"), "native live package trust snapshot");
    };
    const close = async () => { send("\x1b"); await wait(text => !text.includes("Pi resources"), "native package pane close"); };
    const install = async (source: string) => {
     send("i"); await wait(text => text.includes("Install in Global"), "native install source editor");
     send("\x1b[200~" + source + "\x1b[201~\r");
    };
    save("initial", await open());
    await install(packageSource);
    save("installed", await wait(text => text.includes("Load unverified") && trace(env["PI_PTY_CONTROL_TRACE"]).some(row => row.event === "package-factory"), "official local install and registry reload", 30));
    await close();
    await assertSession("after-install");
    send("/fixture-package\r");
    save("command", await wait(text => text.includes("PTY_PACKAGE_COMMAND") && trace(env["PI_PTY_CONTROL_TRACE"]).some(row => row.event === "package-command"), "installed live Pi command"));
    await open(); send("/local-package\r");
    await wait(text => text.includes("local-package"), "native search selects the installed package source");
    send("d");
    await wait(text => text.includes("Remove declaration"), "native package remove review");
    send("\r"); await wait(text => text.includes("Enter confirm"), "native package remove confirmation");
    send("\r");
    save("removed", await wait(text => text.includes("Load unverified") && trace(env["PI_PTY_PACKAGE_TRACE"]).some(row => row.event === "finished" && row.argv[0] === "remove" && row.exit === 0), "official remove and registry reload", 30));
    await close(); await assertSession("after-remove"); send("/fixture-registry\r");
    save("registry-removed", await wait(text => text.includes("PTY_REGISTRY_HAS_PACKAGE:false") && trace(env["PI_PTY_CONTROL_TRACE"]).some(row => row.event === "registry" && row.hasPackage === false), "removed command absent from actual registry"));
    if (!existsSync(join(packageSource, "extensions/index.ts"))) throw new Error("Local package removal deleted its source");
    await open(); await install("-invalid-source");
    save("failure", await wait(text => text.includes("Pi operation failed") && text.includes("package source"), "native package validation failure"));
    await install(join(directory, "cancel-package"));
    save("busy", await wait(text => text.includes("Running official Pi package command") && trace(env["PI_PTY_PACKAGE_TRACE"]).some(row => row.event === "started" && row.argv[1]?.endsWith("cancel-package")), "native cancellable package subprocess"));
    send("\x1b");
    save("cancelled", await wait(text => text.includes("Cancelled") && text.includes("declarations were reread"), "native cancellation retains pane"));
    const cancelledProcess = trace(env["PI_PTY_PACKAGE_TRACE"]).find(row => row.event === "started" && row.argv[1]?.endsWith("cancel-package"));
    let stillAlive = false;
    try { process.kill(cancelledProcess.pid, 0); stillAlive = true; } catch {}
    if (stillAlive) throw new Error("Cancelled package subprocess is still alive");
    writeFileSync(join(artifacts, "packages-cancelled-process.json"), JSON.stringify({ ...cancelledProcess, aliveAfterCancellation:false }));
    await close();
    const launchers = trace(env["PI_PTY_PACKAGE_TRACE"]).filter(row => row.event === "launcher" && row.argv.includes("rpc"));
    if (launchers.length < 3 || launchers.some(row => !row.argv.includes("--no-extensions")))
     throw new Error("Extension changes did not use fresh policy-controlled Pi RPC respawns: " + JSON.stringify(launchers));
    const canonicalSource = realpathSync(packageSource);
    if (!launchers.some(row => row.argv.some((argument: string) => existsSync(argument)
     && (realpathSync(argument) === canonicalSource || realpathSync(argument).startsWith(canonicalSource + "/")))))
     throw new Error("Installed package was not added to the admitted Pi launch arguments");
    writeFileSync(join(artifacts, "packages-registry.json"), JSON.stringify(trace(env["PI_PTY_CONTROL_TRACE"]), null, 2));
    writeFileSync(join(artifacts, "packages-cli.json"), JSON.stringify(trace(env["PI_PTY_PACKAGE_TRACE"]), null, 2));
   }
  } else if (mode === "remote-ui") {
   send("/fixture-ui interactive\r");
   save("open", await wait(text => text.includes("UI_FRAME:120:120:40"), "native remote custom component"));
   terminal.resize(96, 34);
   bridge.stdin.write(JSON.stringify({type:"resize",cols:96,rows:34})+"\n");
   save("resize", await wait(text => text.includes("UI_FRAME:96:96:34"), "native resize reaches live Pi component"));
   send("x");
   save("completed", await wait(text => text.replace(/\s/g, "").includes('"result":"picked","disposed":1'), "focused remote input/dispose"));
  } else if (mode === "models") {
   send("codemode success\r");
   let final = await wait(text => text.includes("MODEL_FIXTURE_DONE") && text.includes("pi-router-fixture/auto")
    && text.includes("pi-model-fixture/small"), "native selected and dispatched model", 30);
   const row = final.split("\n").findIndex(line => line.includes("Codemode"));
   if (row < 0) throw new Error("Generated-image Codemode native card missing");
   send("\t"); await wait(text => text.includes("Space:prompt"), "native model card focus");
   const col = Math.max(1, final.split("\n")[row].indexOf("Codemode") + 2);
   send(`\x1b[<0;${col};${row + 1}M\x1b[<0;${col};${row + 1}m\x1b[C`);
   final = await wait(text => text.includes("MODEL_CLASSIFIER") && text.includes("2 calls") && text.includes("$0.00033"), "native classifier and model-call cost");
   save("image-classifier-top", final);
   // The full script and provider results exceed a 40-row viewport. Navigate
   // the native expanded card instead of removing output from the fixture.
   for (let step = 0; step < 16 && !screen().includes("[Open Image]"); step++) {
    send("\x1b[<65;80;20M");
    await new Promise(done => setTimeout(done, 100));
    await writes;
   }
   final = await wait(text => text.includes("[Open Image]"), "native generated-image affordance after normal wheel scrolling");
   save("image-classifier-bottom", final); send(" ");
  } else if (mode.startsWith("settings-")) {
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
  results.push({ mode, passed: true, nativeExit, stateDirectory: directory,
   ...(mode === "packages" ? { proof:"actual Pi local install/remove and live registry; explicit wrapper subprocess cancellation and backend validation failure" } : {}) });
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
if (modes.some(mode => ["runtime", "packages", "models", "remote-ui", "product-surface"].includes(mode))) {
 if (!process.env.PI_NATIVE_EXPECTED_SHA256 || process.env.PI_NATIVE_EXPECTED_SHA256 !== binarySha256)
  throw new Error("New Pi deep-adaptation PTY cases require the root-verified fresh binary SHA in PI_NATIVE_EXPECTED_SHA256");
}
for (const mode of modes) {
 if (mode === "settings") {
  const directory = await run("settings-save");
  await run("settings-reopen", directory);
  await run("settings-rollback", directory);
  continue;
 }
 if (!["eval", "codemode", "signal", "timeout", "eof", "minimal", "runtime", "packages", "models", "remote-ui", "product-surface"].includes(mode)) throw new Error("Unknown PTY fixture: " + mode);
 await run(mode);
}
const finalHash = createHash("sha256");
for await (const chunk of createReadStream(binary)) finalHash.update(chunk);
if (finalHash.digest("hex") !== binarySha256) throw new Error("Native binary changed during PTY cases; rerun against one build");
writeFileSync(join(artifacts, "report.json"), JSON.stringify({ binary, binarySha256,
 pi, screenEmulator: require.resolve("@xterm/headless"), results }, null, 2) + "\n");
console.log(JSON.stringify({ passed: true, artifacts, cases: results.length }));
