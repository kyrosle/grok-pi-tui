#!/usr/bin/env python3
"""Installed Pi 1.x RPC + temporary UI fixture; no network or real user state."""
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = r'''
import { AssistantMessageEventStream } from "@earendil-works/pi-ai";
export default function(pi: any) {
 pi.registerCommand("fixture-ui", {
  getArgumentCompletions: () => [{value:"probe",label:"Probe"}],
  async handler(args: string, ctx: any) {
   if (args === "bad-completion") {
    pi.registerCommand("fixture-bad-completion", {getArgumentCompletions:()=>[{value:1n,label:"invalid"}],handler:async()=>{}});
    ctx.ui.notify("UI_BAD_COMPLETION:registered", "info"); return;
   }
   if (args === "dialogs") {
    const values = [await ctx.ui.select("select", ["first", "second"]),
     await ctx.ui.confirm("confirm", "message"), await ctx.ui.input("input", "placeholder"),
     await ctx.ui.editor("editor", "prefill\nline")];
    ctx.ui.notify("UI_DIALOGS:" + JSON.stringify(values), "info"); return;
   }
   if (args === "guarded") {
    const allowed = ctx.mode === "tui" && !process.argv.includes("--mode");
    const result = allowed ? await ctx.ui.custom((_tui: any, _theme: any, _keys: any, done: any) => {
     done("guarded"); return {render:()=>[],invalidate(){}};
    }) : undefined;
    ctx.ui.notify("UI_GUARDED:" + JSON.stringify({allowed,result:result??null}), "info"); return;
   }
   if (args === "interactive") {
    let disposed = 0, invalidated = 0;
    const result = await ctx.ui.custom((tui: any, _theme: any, _keys: any, done: any) => {
     tui.setFocus({render:()=>[],invalidate(){},handleInput(data: string){if(data === "x") done("picked");}});
     return {render:(width: number)=>["UI_FRAME:"+width+":"+tui.terminal.columns+":"+tui.terminal.rows],invalidate(){invalidated++;},
      handleInput(){throw new Error("root stole focused child input");},dispose(){disposed++;}};
    });
    ctx.ui.notify("UI_INTERACTIVE:" + JSON.stringify({result:result??null,disposed,invalidated}), "info"); return;
   }
   let factoryRuns = 0, disposed = 0;
   ctx.ui.setWidget("fixture-above", ["above"], {placement:"aboveEditor"});
   ctx.ui.setWidget("fixture-below", ["below"], {placement:"belowEditor"});
   ctx.ui.setWidget("fixture-above", undefined);
   ctx.ui.setStatus("fixture-status", "status"); ctx.ui.setTitle("fixture-title");
   ctx.ui.setEditorText("first"); ctx.ui.pasteToEditor("replacement");
   const readback = ctx.ui.getEditorText(); ctx.ui.getEditorText();
   ctx.ui.setWidget("fixture-factory", () => {factoryRuns++; return {render:()=>[]};});
   ctx.ui.setHeader(() => {factoryRuns++; return {render:()=>[]};});
   const unsubscribe = ctx.ui.onTerminalInput(() => {factoryRuns++;}); unsubscribe();
   const result = await ctx.ui.custom((_tui: any, _theme: any, _keys: any, done: any) => {
    factoryRuns++; done("closed");
    return {render:()=>["fixture-frame"],invalidate(){},dispose(){disposed++;}};
   });
   await new Promise(resolve => setImmediate(resolve));
   ctx.ui.notify("UI_RESULT:" + JSON.stringify({mode:ctx.mode,readback,result:result??null,factoryRuns,disposed}), "info");
  }
 });
 pi.on("before_agent_start", (_event: any, ctx: any) => {
  ctx.ui.setWorkingMessage("fixture-working"); ctx.ui.setWorkingIndicator({frames:["●"]});
 });
 pi.registerProvider("pi-ui-fixture", {
  baseUrl:"http://127.0.0.1:9", apiKey:"fixture", api:"openai-completions",
  models:[{id:"local",name:"UI fixture",reasoning:false,input:["text"],contextWindow:32768,maxTokens:128,
   cost:{input:0,output:0,cacheRead:0,cacheWrite:0}}],
  streamSimple(model: any) {
   const stream = new AssistantMessageEventStream();
   const message = {role:"assistant",content:[{type:"text",text:"fixture-ui-done"}],
    api:model.api,provider:model.provider,model:model.id,stopReason:"stop",timestamp:Date.now(),
    usage:{input:0,output:0,cacheRead:0,cacheWrite:0,totalTokens:0,cost:{input:0,output:0,cacheRead:0,cacheWrite:0,total:0}}};
   queueMicrotask(()=>{stream.push({type:"start",partial:message});stream.push({type:"done",reason:"stop",message});stream.end();});
   return stream;
  }
 });
}
'''


def run(directory, mode):
    root = Path(directory) / mode
    root.mkdir()
    # Mirror the injector's relative-import closure into a disposable directory.
    compat = root / "compat"
    compat.mkdir()
    for name in ("index.ts", "ui.ts"):
        shutil.copy2(ROOT / "extensions/pi-grok-rpc-compat" / name, compat / name)
    remote = root / "remote"
    remote.mkdir()
    for name in ("index.ts", "shared.ts", "env.ts", "layout.ts", "transport.ts", "host.ts", "demo.ts"):
        shutil.copy2(ROOT / "extensions/pi-grok-remote-tui" / name, remote / name)
    fixture = root / "fixture.ts"
    fixture.write_text(FIXTURE)
    env = {key: value for key, value in os.environ.items() if not (
        key.endswith("_API_KEY") or key.endswith("_TOKEN") or key.startswith("AWS_")
        or key in ("PI_GROK", "PI_GROK_EXTENSION_TUI_COMPAT", "PI_GROK_REMOTE_TUI"))}
    env.update(PI_CODING_AGENT_DIR=str(root / "pi"), GROK_HOME=str(root / "grok"),
               PI_OFFLINE="1", PI_TELEMETRY="0", PI_GROK_REMOTE_TUI_META=str(root / "active.json"))
    if mode != "native":
        env.update(PI_GROK="1", PI_GROK_EXTENSION_TUI_COMPAT="1", PI_GROK_REMOTE_TUI="1" if mode == "on" else "0")
    pi_bin = env.get("PI_BIN", "pi")
    cli = Path(shutil.which(pi_bin) or pi_bin).resolve()
    rpc_entry = cli.with_name("rpc-entry.js")
    command = ["node", str(rpc_entry)] if rpc_entry.is_file() else [pi_bin, "--mode", "rpc"]
    command += ["--offline", "-ne", "-ns", "-np", "-nc",
               "--no-themes", "--no-approve", "--session-dir", str(root / "sessions"),
               "--model", "pi-ui-fixture/local", "-e", str(compat / "index.ts")]
    if mode == "on":
        command.extend(["-e", str(remote / "index.ts")])
    command.extend(["-e", str(fixture)])
    process = subprocess.Popen(command, cwd=root, env=env, text=True, bufsize=1,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    output, errors, events = queue.Queue(), [], []
    def read_stdout():
        for line in process.stdout:
            try: output.put(json.loads(line))
            except ValueError: pass
        output.put(None)
    def read_stderr():
        errors.extend(process.stderr)
    threading.Thread(target=read_stdout, daemon=True).start()
    threading.Thread(target=read_stderr, daemon=True).start()
    def send(value):
        process.stdin.write(json.dumps(value) + "\n")
        process.stdin.flush()
    def until(predicate, seconds=25):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            try: event = output.get(timeout=max(.1, deadline - time.monotonic()))
            except queue.Empty: break
            assert event is not None, "Pi EOF: " + "".join(errors)
            events.append(event)
            if event.get("type") == "extension_ui_request" and event.get("method") in ("select", "confirm", "input", "editor"):
                reply = {"type": "extension_ui_response", "id": event["id"]}
                if event["method"] == "confirm": reply["confirmed"] = True
                else: reply["value"] = {"select": "first", "input": "typed", "editor": "edited\nline"}[event["method"]]
                send(reply)
            if predicate(event): return event
        raise AssertionError(f"Pi UI deadline ({mode}): " + "".join(errors))
    def request(kind, **params):
        identity = f"request-{len(events)}"
        send({"id": identity, "type": kind, **params})
        response = until(lambda event: event.get("id") == identity)
        assert response.get("success"), response
        return response.get("data", {})
    def command_result(message, prefix):
        send({"type": "prompt", "message": message})
        return until(lambda event: event.get("type") == "extension_ui_request" and str(event.get("message", "")).startswith(prefix))
    try:
        commands = request("get_commands")["commands"]
        fixture_command = next(item for item in commands if item.get("name") == "fixture-ui")
        if mode != "native":
            assert fixture_command.get("argumentCompletions") == [{"value": "probe", "label": "Probe"}], fixture_command
            assert any(item.get("name") == "pi-ui-capabilities" for item in commands)
        else:
            assert not any(item.get("name") == "pi-ui-capabilities" for item in commands)
        assert json.loads(command_result("/fixture-ui dialogs", "UI_DIALOGS:")["message"].split(":", 1)[1]) == ["first", True, "typed", "edited\nline"]
        result = json.loads(command_result("/fixture-ui probe", "UI_RESULT:")["message"].split(":", 1)[1])
        assert result == {"mode": "tui" if mode == "on" else "rpc", "readback": "", "result": "closed" if mode == "on" else None,
                          "factoryRuns": 1 if mode == "on" else 0, "disposed": 1 if mode == "on" else 0}, result
        guarded = json.loads(command_result("/fixture-ui guarded", "UI_GUARDED:")["message"].split(":", 1)[1])
        assert guarded == {"allowed": mode == "on", "result": "guarded" if mode == "on" else None}, guarded
        if mode == "on":
            for operation in ("input", "cancel"):
                send({"type": "prompt", "message": "/fixture-ui interactive"})
                until(lambda event: event.get("widgetKey") == "remote_tui" and str(event.get("widgetLines", [""])[0]).startswith("UI_FRAME:"))
                metadata = json.loads((root / "active.json").read_text())
                with open(metadata["keysPath"], "a") as keys:
                    keys.write(json.dumps({"id": "stale", "op": "resize", "columns": 200, "rows": 80}) + "\n")
                    keys.write(json.dumps({"id": metadata["id"], "op": "resize", "columns": 112, "rows": 33}) + "\n")
                until(lambda event: event.get("widgetKey") == "remote_tui" and event.get("widgetLines") == ["UI_FRAME:112:112:33"])
                with open(metadata["keysPath"], "a") as keys:
                    keys.write(json.dumps({"id": metadata["id"], "op": operation, "data": "x"}) + "\n")
                interactive = json.loads(until(lambda event: event.get("type") == "extension_ui_request" and str(event.get("message", "")).startswith("UI_INTERACTIVE:"))["message"].split(":", 1)[1])
                assert interactive == {"result": "picked" if operation == "input" else None, "disposed": 1, "invalidated": 1}, interactive
                assert not (root / "active.json").exists()
        ui_events = [event for event in events if event.get("type") == "extension_ui_request"]
        assert any(event.get("widgetKey") == "fixture-below" and event.get("widgetPlacement") == "belowEditor" for event in ui_events)
        assert any(event.get("widgetKey") == "fixture-above" and "widgetLines" not in event for event in ui_events)
        assert [event["text"] for event in ui_events if event.get("method") == "set_editor_text"] == ["first", "replacement"]
        if mode != "native":
            assert sum("getEditorText:" in event.get("message", "") for event in ui_events) == 1
            report = command_result("/pi-ui-capabilities", "Pi extension UI capabilities")["message"]
            assert f"Remote TUI={'active' if mode == 'on' else 'inactive'}" in report, report
            assert "unsupported" in report and "experimental" in report and "mapped" in report
            request("prompt", message="fixture working lifecycle")
            until(lambda event: event.get("type") == "agent_settled")
            working = [event for event in events if event.get("statusKey") == "__pi_grok_working__"]
            assert any(event.get("statusText") == "● fixture-working" for event in working), working
            assert "statusText" not in working[-1], working
            command_result("/fixture-ui bad-completion", "UI_BAD_COMPLETION:")
            fallback_commands = request("get_commands")["commands"]
            assert any(item.get("name") == "fixture-bad-completion" for item in fallback_commands)
            assert request("get_state"), "official RPC remains usable after malformed private completion metadata"
        assert not any("Failed to load extension" in line or "private host hook unavailable" in line for line in errors), "".join(errors)
        checks = "dialogs, editor/widget, unchanged non-grok registration" if mode == "native" else "dialogs, editor/widget, working lifecycle, diagnostics, completion enrichment/failure fallback"
        if mode == "on": checks += ", guarded factory, focus/input/resize/cancel/dispose"
        print(f"PASS actual Pi UI {mode}: {checks}")
    finally:
        if process.poll() is None:
            process.stdin.close()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=5)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="pi-ui-rpc-test-") as directory:
        for mode in ("off", "on", "native"):
            run(directory, mode)
