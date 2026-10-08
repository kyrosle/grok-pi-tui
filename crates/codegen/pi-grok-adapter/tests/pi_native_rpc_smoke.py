#!/usr/bin/env python3
"""Installed Pi Codemode/tool/MCP pipeline with local synthetic provider; no inference."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[4]


def run_fixture(directory, mode):
    root = Path(directory) / mode
    root.mkdir()
    env = {k: v for k, v in os.environ.items() if not (k.endswith("_API_KEY") or k.endswith("_TOKEN") or k.startswith("AWS_"))}
    env.update(PI_CODING_AGENT_DIR=str(root / "pi"), GROK_HOME=str(root / "grok"), PI_OFFLINE="1", PI_TELEMETRY="0")
    # Old overrides must never reactivate the retired host execution environment.
    env.update(PI_GROK_EVAL_VERSION="v2", PI_GROK_EVAL_V2_ONLY="1", PI_GROK_EVAL_MCP="1")
    command = [env.get("PI_BIN", "pi"), "--mode", "rpc", "--offline", "-ne", "-ns", "-np", "-nc", "--no-themes", "--no-session", "--no-approve", "--model", "pi-fixture/local",
               "--extension", str(ROOT / "extensions/pi-grok-bash/index.ts"), "--extension", "builtin:codemode",
               "--extension", str(Path(__file__).parent / "fixtures/pi_native_tools.ts")]
    home = Path(env["PI_CODING_AGENT_DIR"])
    home.mkdir(parents=True, exist_ok=True)
    (home / "settings.json").write_text(json.dumps({"defaultTools": ["+codemode"]}))
    trace = root / "mcp.log"
    if mode != "normal":
        env["PI_FIXTURE_MCP_MODE"] = mode
        home = Path(env["PI_CODING_AGENT_DIR"])
        server = {"command": "python3", "args": [str(Path(__file__).parent / "fixtures/local_mcp.py")], "env": {"PI_FIXTURE_MCP_LOG": str(trace)}, "toolExposure": {"hidden": "hidden"}}
        if mode == "deferred": server["toolExposure"]["echo"] = "deferred"
        (home / "mcp.json").write_text(json.dumps({"mcpServers": {"local": server}}))
        if mode != "disabled": command.extend(["--extension", "builtin:mcp", "--extension", "builtin:tool-search"])
        if mode == "excluded": command.extend(["--exclude-tools", "mcp__local__echo"])
        if mode == "search-excluded": command.extend(["--exclude-tools", "tool_search"])
    process = subprocess.Popen(command, cwd=root, env=env, text=True, bufsize=1, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    output, errors, events = queue.Queue(), [], []
    def stdout():
        for line in process.stdout:
            try: output.put(json.loads(line))
            except ValueError: pass
        output.put(None)
    def stderr(): errors.extend(process.stderr)
    threading.Thread(target=stdout, daemon=True).start()
    threading.Thread(target=stderr, daemon=True).start()
    def send(value): process.stdin.write(json.dumps(value) + "\n"); process.stdin.flush()
    def until(predicate):
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            event = output.get(timeout=25)
            assert event is not None, "Pi closed: " + "".join(errors)
            events.append(event)
            if predicate(event): return event
        raise AssertionError("Pi deadline: " + mode)
    try:
        if mode not in ["normal", "disabled"]:
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                send({"type": "prompt", "message": "/fixture-tools"})
                event = until(lambda e: e.get("type") == "extension_ui_request" and str(e.get("message", "")).startswith("FIXTURE_TOOLS:"))
                names = json.loads(event["message"].split(":", 1)[1])
                if "read_mcp_resource" in names and (mode == "excluded" or "mcp__local__echo" in names): break
                time.sleep(0.05)
            else: raise AssertionError("MCP registry did not connect: " + mode)
        send({"type": "prompt", "message": "Native Codemode " + mode})
        until(lambda e: e.get("type") == "agent_settled")
        messages = [e["message"] for e in events if e.get("type") == "message_end"]
        result = next(m for m in messages if m.get("role") == "toolResult" and m.get("toolName") == "codemode")
        body = "\n".join(c.get("text", "") for c in result.get("content", []))
        assert not result.get("isError"), body
        payload = next(json.loads(line) for line in body.splitlines() if line.startswith("{"))
        completion = next(json.loads(c["text"]) for m in messages for c in m.get("content", []) if c.get("text", "").startswith('{"fixtureComplete"'))
        assert "codemode" in completion["tools"] and "eval" not in completion["tools"], completion
        assert "fixture_hidden" not in completion["tools"]
        if mode == "normal":
            assert completion["executions"] == 1 and len(payload["failures"]) == 3
            assert "fixture-policy" in body
        elif mode in ["disabled", "excluded"]:
            assert payload["blocked"] and (not trace.exists() or "tools/call" not in trace.read_text())
        else:
            assert "local-mcp:native" in body and "local resource" in body, body
            assert "mcp__local__hidden" not in payload["discovered"]
        print(json.dumps({"mode": mode, "passed": True, "tools": completion["tools"]}))
    finally:
        process.terminate()
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired: process.kill(); process.wait(timeout=5)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="pi-native-rpc-") as directory:
        for mode in ["normal", "enabled", "deferred", "search-excluded", "excluded", "disabled"]:
            run_fixture(directory, mode)
