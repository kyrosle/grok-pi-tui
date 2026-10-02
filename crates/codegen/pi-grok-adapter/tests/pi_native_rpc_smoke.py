#!/usr/bin/env python3
"""Exercise the installed Pi tool pipeline with a local synthetic provider; no inference."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[4]


def run_fixture(directory, eval_only, mcp_mode=None):
    env = dict(os.environ)
    env.update(PI_CODING_AGENT_DIR=str(Path(directory) / f"pi-{eval_only}"),
               GROK_HOME=str(Path(directory) / "grok"), PI_GROK="1",
               PI_GROK_EVAL_VERSION="v2", PI_GROK_EVAL_V2_ONLY=str(eval_only),
               PI_GROK_EVAL_MCP="0", PI_OFFLINE="1", PI_TELEMETRY="0")
    command = [env.get("PI_BIN", "pi"), "--mode", "rpc", "--offline", "-ne", "-ns", "-np",
               "-nc", "--no-themes", "--no-session", "--no-approve", "--model", "pi-fixture/local",
               "--extension", str(ROOT / "extensions/pi-grok-bash/index.ts"),
               "--extension", str(Path(__file__).parent / "fixtures/pi_native_tools.ts")]
    trace = Path(directory) / f"mcp-{eval_only}-{mcp_mode}.log"
    if mcp_mode:
        env["PI_FIXTURE_MCP_MODE"] = mcp_mode
        env["PI_GROK_MCP"] = "0" if mcp_mode == "disabled" else "1"
        state = Path(env["PI_CODING_AGENT_DIR"])
        state.mkdir(parents=True, exist_ok=True)
        config = {"mcpServers": {"local": {"command": "python3", "args": [str(Path(__file__).parent / "fixtures/local_mcp.py")],
                   "env": {"PI_FIXTURE_MCP_LOG": str(trace)}, "toolExposure": {"hidden": "hidden"}}}}
        if mcp_mode == "deferred":
            config["mcpServers"]["local"]["exposure"] = "deferred"
        (state / "mcp.json").write_text(json.dumps(config))
        if mcp_mode != "disabled":
            command.extend(["--extension", "builtin:codemode", "--extension", "builtin:mcp", "--extension", "builtin:tool-search"])
        if mcp_mode == "excluded":
            command.extend(["--exclude-tools", "mcp__local__echo"])
        if mcp_mode == "search-excluded":
            command.extend(["--exclude-tools", "tool_search"])
    process = subprocess.Popen(command, cwd=directory, env=env, text=True, bufsize=1,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    output, errors = queue.Queue(), []

    def read_stdout():
        for line in process.stdout:
            try:
                output.put(json.loads(line))
            except ValueError:
                pass
        output.put(None)

    def read_stderr():
        errors.extend(process.stderr)

    threading.Thread(target=read_stdout, daemon=True).start()
    threading.Thread(target=read_stderr, daemon=True).start()
    events = []
    try:
        process.stdin.write(json.dumps({"id": "fixture", "type": "prompt", "message": "Run local fixture"}) + "\n")
        process.stdin.flush()
        deadline = time.monotonic() + 30
        while True:
            event = output.get(timeout=max(0.1, deadline - time.monotonic()))
            assert event is not None, "Pi closed stdout: " + "".join(errors)
            events.append(event)
            if event.get("type") == "agent_settled":
                break
            assert time.monotonic() < deadline, "Pi fixture did not settle"
        messages = [event["message"] for event in events if event.get("type") == "message_end"]
        texts = [item.get("text", "") for message in messages for item in message.get("content", []) if item.get("type") == "text"]
        completion = next(json.loads(text) for text in texts if text.startswith('{"fixtureComplete"'))
        result = next(message for message in messages if message.get("role") == "toolResult" and message.get("toolName") == "eval")
        body = "\n".join(item.get("text", "") for item in result.get("content", []))
        assert not result.get("isError"), (body, completion)
        if mcp_mode:
            payload = json.loads(body)
            if mcp_mode in ["disabled", "excluded"]:
                assert payload["blocked"], payload
                assert not trace.exists() or "tools/call" not in trace.read_text(), payload
            else:
                assert "local-mcp:native" in body, body
                assert payload["result"]["structuredContent"]["structuredContent"] == {"value": "native"}, payload
                assert "local resource" in str(payload["resource"]), payload
                assert "mcp__local__hidden" not in payload["discovered"], payload
        else:
            assert completion["executions"] == 1, (completion, body)
            assert "fixture-policy" in body, body
        if eval_only:
            assert completion["tools"] == ["eval"], completion
        assert "fixture_hidden" not in completion["tools"], completion
        nested = result.get("nestedCalls", [])
        assert nested or mcp_mode in ["disabled", "excluded"], "Pi did not record official nested calls"
        process.stdin.write(json.dumps({"id": "entries", "type": "get_entries"}) + "\n")
        process.stdin.flush()
        while True:
            event = output.get(timeout=5)
            assert event is not None, "Pi closed during replay snapshot check"
            if event.get("id") == "entries":
                entries = event["data"]["entries"]
                break
        replay = [entry for entry in entries if entry.get("customType") == "pi-grok-eval-tool/v1"]
        if nested:
            calls = nested.get("calls", [])
            assert len(replay) == 2 * len(calls), (len(replay), calls)
            assert all(entry["data"]["replayOnly"] for entry in replay)
            assert [entry["data"]["toolCallId"] for entry in replay[::2]] == [call["id"] for call in calls]
        print(json.dumps({"eval_only": bool(eval_only), "executions": completion["executions"],
                          "tools": completion["tools"], "nested_calls": nested}, ensure_ascii=False))
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="pi-native-rpc-") as directory:
        run_fixture(directory, 0)
        run_fixture(directory, 1)
        for mode in ["enabled", "deferred", "search-excluded", "excluded", "disabled"]:
            run_fixture(directory, 0, mode)
        run_fixture(directory, 1, "enabled")
