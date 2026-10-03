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


def run_runtime_fixture(directory, mode):
    """Actual official executeTool and durable session paths, never FakeToolContext."""
    root = Path(directory) / mode
    root.mkdir()
    env = {key: value for key, value in os.environ.items() if not (
        key.endswith("_API_KEY") or key.endswith("_TOKEN") or key.startswith("AWS_"))}
    env.update(PI_CODING_AGENT_DIR=str(root / "pi"), GROK_HOME=str(root / "grok"),
               GROK_PROJECT_DIR=str(root / "project"), PI_GROK_EVAL_VERSION="v2",
               PI_GROK_EVAL_V2_ONLY="1", PI_GROK_EVAL_MCP="0", PI_OFFLINE="1",
               PI_TELEMETRY="0", PI_FIXTURE_RUNTIME_MODE=mode)
    command = [env.get("PI_BIN", "pi"), "--mode", "rpc", "--offline", "-ne", "-ns", "-np", "-nc",
               "--no-themes", "--no-approve", "--session-dir", str(root / "sessions"),
               "--model", "pi-fixture/local", "--extension", str(ROOT / "extensions/pi-grok-bash/index.ts"),
               "--extension", str(Path(__file__).parent / "fixtures/pi_native_tools.ts")]
    if mode == "policy": command.extend(["--exclude-tools", "fixture_note"])
    output, errors, all_events = queue.Queue(), [], []
    def start(extra=()):
        process = subprocess.Popen(command + list(extra), cwd=root, env=env, text=True, bufsize=1,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def stdout():
            for line in process.stdout:
                try: output.put(json.loads(line))
                except ValueError: pass
            output.put(None)
        def stderr(): errors.extend(process.stderr)
        reader = threading.Thread(target=stdout, daemon=True); reader.start()
        threading.Thread(target=stderr, daemon=True).start()
        return process, reader
    process, reader = start()
    def send(value):
        process.stdin.write(json.dumps(value) + "\n"); process.stdin.flush()
    def until(predicate, seconds=30):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            event = output.get(timeout=max(.1, deadline - time.monotonic()))
            assert event is not None, "Pi EOF: " + "".join(errors)
            all_events.append(event)
            if predicate(event): return event
        raise AssertionError("actual Pi runtime deadline: " + mode)
    def request(kind, **params):
        identity = f"request-{len(all_events)}"
        send({"id": identity, "type": kind, **params})
        response = until(lambda event: event.get("id") == identity)
        assert response.get("success"), response
        return response.get("data", {})
    def prompt():
        boundary = len(all_events)
        send({"id": f"prompt-{boundary}", "type": "prompt", "message": "Runtime " + mode})
        until(lambda event: event.get("type") == "agent_settled")
        events = all_events[boundary:]
        messages = [event["message"] for event in events if event.get("type") == "message_end"]
        text = [item.get("text", "") for message in messages for item in message.get("content", []) if item.get("type") == "text"]
        completion = next(json.loads(item) for item in text if item.startswith('{"fixtureComplete"'))
        assert completion["tools"] == ["eval"], completion
        results = [message for message in messages if message.get("role") == "toolResult" and message.get("toolName") == "eval"]
        assert results and all(not result.get("isError") for result in results), results
        return completion, results
    def stats():
        send({"id": "stats", "type": "prompt", "message": "/fixture-runtime-stats"})
        event = until(lambda event: event.get("type") == "extension_ui_request" and str(event.get("message", "")).startswith("PI_RUNTIME_STATS:"))
        return json.loads(event["message"].split(":", 1)[1])
    try:
        if mode == "abort":
            send({"id": "abort-prompt", "type": "prompt", "message": "Runtime abort"})
            start_event = until(lambda event: event.get("type") == "tool_execution_start" and event.get("toolName") == "fixture_parallel")
            assert start_event.get("parentToolCallId"), start_event
            send({"id": "abort", "type": "abort"})
            until(lambda event: event.get("type") == "agent_settled")
            state = stats()
            assert state["parallel"] == 0 and state["aborted"] == 1 and state["completed"] == 0, state
            terminal = [event for event in all_events if event.get("type") == "tool_execution_end" and event.get("toolCallId") == start_event["toolCallId"]]
            assert len(terminal) == 1 and terminal[0]["isError"], terminal
        elif mode == "concurrency":
            completion, _ = prompt(); state = completion["runtime"]
            assert state["maxParallel"] >= 2, state
            assert state["maxSequential"] == 1 and state["parallel"] == state["sequential"] == 0, state
            assert state["completed"] == 7, state
        elif mode == "background":
            _, results = prompt()
            assert len(results) == 2 and results[0]["details"]["background"], results
            first_id = results[0]["details"]["taskId"]
            text = "\n".join(item.get("text", "") for item in results[1]["content"])
            envelope = json.loads(text.split("RUNTIME_BACKGROUND_RESULT:", 1)[1])
            payload = json.loads(envelope["text"])
            assert payload["task_id"] == first_id and payload["status"] == "completed", payload
            assert "RUNTIME_BACKGROUND_DONE" in payload["output"], payload
        elif mode == "policy":
            snapshots = []
            def verify_policy():
                completion, results = prompt()
                text = "\n".join(item.get("text", "") for item in results[0]["content"])
                payload = json.loads(text.split("RUNTIME_POLICY:", 1)[1])
                assert "fixture_note" not in payload["declared"], payload
                assert "excluded" in payload["results"]["fixture_note"] or "non-callable" in payload["results"]["fixture_note"], payload
                assert "fixture-slow:policy" in str(payload["results"]["fixture_parallel"]), payload
                assert completion["executions"] == 0, completion
                snapshots.append({"declared": completion["tools"], "callable": payload["declared"]})
            verify_policy()
            parent = Path(request("get_state")["sessionFile"]); assert parent.is_file()
            request("new_session")
            request("set_model", provider="pi-fixture", modelId="local")
            verify_policy()
            request("switch_session", sessionPath=str(parent))
            verify_policy()
            process.terminate(); process.wait(timeout=5); reader.join(timeout=5)
            while not output.empty(): output.get_nowait()
            process, reader = start(["--session", str(parent)])
            verify_policy()
            assert all(snapshot == snapshots[0] for snapshot in snapshots), snapshots
        if path := os.environ.get("PI_EVAL_RUNTIME_CAPTURE"):
            directory = Path(path); directory.mkdir(parents=True, exist_ok=True)
            (directory / f"{mode}.json").write_text(json.dumps({"mode": mode, "events": all_events}, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps({"passed": True, "runtime": mode, "events": len(all_events)}))
    finally:
        process.terminate()
        try: process.wait(timeout=5)
        except subprocess.TimeoutExpired: process.kill(); process.wait(timeout=5)


def run_fixture(directory, eval_only, mcp_mode=None):
    env = {key: value for key, value in os.environ.items() if not (key.endswith("_API_KEY") or key.endswith("_TOKEN") or key.startswith("AWS_"))}
    env.update(PI_CODING_AGENT_DIR=str(Path(directory) / f"pi-{eval_only}"),
               GROK_HOME=str(Path(directory) / "grok"), GROK_PROJECT_DIR=str(Path(directory) / "project"), PI_GROK="1",
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
        for mode in ["abort", "concurrency", "background", "policy"]:
            run_runtime_fixture(directory, mode)
