#!/usr/bin/env python3
"""Installed public Pi RPC/ModelRuntime/Codemode proof; temporary synthetic models only."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time


class Pi:
    def __init__(self, root, session=None):
        env = {key: value for key, value in os.environ.items() if not (
            key.endswith("_API_KEY") or key.endswith("_TOKEN") or key.startswith("AWS_"))}
        env.update(PI_CODING_AGENT_DIR=str(root / "pi"), PI_OFFLINE="1", PI_TELEMETRY="0")
        argv = [env.get("PI_BIN", "pi"), "--mode", "rpc", "--offline", "-ne", "-ns", "-np", "-nc", "--no-themes",
                "--no-approve", "--session-dir", str(root / "sessions"), "--model", "pi-router-fixture/auto", "--thinking", "high",
                "--extension", str(Path(__file__).parent / "fixtures/pi_models.ts"), "--extension", "builtin:codemode", "--tools", "codemode"]
        if session:
            argv += ["--session", session]
        self.process = subprocess.Popen(argv, cwd=root, env=env, text=True, bufsize=1,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.output, self.events, self.errors = queue.Queue(), [], []
        def reader():
            for line in self.process.stdout:
                self.output.put(json.loads(line))
            self.output.put(None)
        threading.Thread(target=reader, daemon=True).start()
        threading.Thread(target=lambda: self.errors.extend(self.process.stderr), daemon=True).start()
        self.serial = 0

    def send(self, kind, **params):
        self.serial += 1
        identity = "models-" + str(self.serial)
        self.process.stdin.write(json.dumps({"id": identity, "type": kind, **params}) + "\n")
        self.process.stdin.flush()
        return identity

    def until(self, predicate, seconds=30):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            event = self.output.get(timeout=max(0.1, deadline - time.monotonic()))
            assert event is not None, "Pi exited: " + "".join(self.errors)
            self.events.append(event)
            if predicate(event):
                return event
        raise AssertionError("Pi model fixture timed out")

    def request(self, kind, **params):
        identity = self.send(kind, **params)
        event = self.until(lambda event: event.get("id") == identity)
        assert event.get("success"), event
        return event.get("data", {})

    def prompt(self, message):
        start = len(self.events)
        self.send("prompt", message=message)
        self.until(lambda event: event.get("type") == "agent_settled")
        return self.events[start:]

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)


def run():
    with tempfile.TemporaryDirectory(prefix="pi-model-proof-") as directory:
        root = Path(directory)
        pi = Pi(root)
        try:
            catalog = pi.request("get_available_models")["models"]
            assert any(model["provider"] == "pi-router-fixture" for model in catalog), catalog
            assert all(model.get("type", "chat") == "chat" for model in catalog), catalog
            assert not any(model["id"] in ("painter", "judge") for model in catalog), catalog
            pi.send("prompt", message="/fixture-model-sdk")
            report = pi.until(lambda event: event.get("type") == "extension_ui_request" and event.get("message", "").startswith("MODEL_SDK:"))
            sdk = json.loads(report["message"].split(":", 1)[1])
            assert sdk[0]["image"] == sdk[0]["classify"] == "stop", sdk
            assert sdk[0]["images"] == 1 and sdk[0]["probability"] == 0.9, sdk
            assert sdk[1]["image"] == sdk[1]["classify"] == "error", sdk
            assert sdk[2]["image"] == sdk[2]["classify"] == "aborted", sdk
            small = pi.prompt("route-small")
            physical = [event["message"] for event in small if event.get("type") == "message_end" and event["message"].get("role") == "assistant"]
            assert physical[-1]["provider"] == "pi-model-fixture" and physical[-1]["model"] == "small", physical
            assert physical[-1]["thinkingLevel"] == "medium", physical
            state = pi.request("get_state")
            assert state["model"]["provider"] == "pi-router-fixture" and state["thinkingLevel"] == "high", state
            assert pi.request("get_session_stats")["contextUsage"]["contextWindow"] == 32768
            first_leaf = pi.request("get_entries")["leafId"]
            success = pi.prompt("codemode success")
            tool = next(event["message"] for event in success if event.get("type") == "message_end" and event["message"].get("role") == "toolResult")
            assert not tool.get("isError") and any(block["type"] == "image" for block in tool["content"]), tool
            assert abs(tool["usage"]["cost"]["total"] - 0.00033) < 1e-12, tool
            all_entries = pi.request("get_entries")["entries"]
            replay_tool = next(entry["message"] for entry in all_entries if entry.get("message", {}).get("toolCallId") == "fixture-model-codemode")
            assert replay_tool["content"] == tool["content"], replay_tool
            stats = pi.request("get_session_stats")
            assistant_cost = sum(entry.get("message", {}).get("usage", {}).get("cost", {}).get("total", 0)
                                 for entry in all_entries if entry.get("message", {}).get("role") == "assistant")
            assert abs(stats["cost"] - assistant_cost - 0.00033) < 1e-12, stats
            large = pi.prompt("route-large")
            assert next(event["message"]["model"] for event in reversed(large)
                        if event.get("type") == "message_end" and event["message"].get("role") == "assistant") == "large"
            assert pi.request("get_session_stats")["contextUsage"]["contextWindow"] == 65536
            session = pi.request("get_state")["sessionFile"]
            pi.request("new_session")
            pi.request("switch_session", sessionPath=session)
            assert pi.request("get_state")["model"]["provider"] == "pi-router-fixture"
            pi.request("prompt", message="/fixture-model-tree " + first_leaf)
            assert pi.request("get_session_stats")["contextUsage"]["contextWindow"] == 32768
            active_leaf = pi.request("get_entries")["leafId"]
            assert active_leaf == first_leaf, (active_leaf, first_leaf)
            # Pi persists the new branch once work is appended after in-memory navigation.
            pi.prompt("route-small after-tree")
            first_run = pi.events
            pi.close()
            pi = Pi(root, session)
            restored = pi.request("get_state")
            assert restored["model"]["provider"] == "pi-router-fixture" and restored["thinkingLevel"] == "high", restored
            assert pi.request("get_session_stats")["contextUsage"]["contextWindow"] == 32768
            failure = pi.prompt("codemode error")
            assert any(event.get("type") == "tool_execution_end" and event.get("isError") for event in failure), failure
            pi.send("prompt", message="codemode cancel")
            pi.until(lambda event: event.get("type") == "extension_ui_request" and event.get("statusText") == "MODEL_FIXTURE_WAITING")
            pi.send("abort")
            pi.until(lambda event: event.get("type") == "agent_settled")
            assert not pi.request("get_state")["isStreaming"]
            events = len(first_run) + len(pi.events)
            if capture := os.environ.get("PI_MODEL_RPC_CAPTURE"):
                Path(capture).write_text(json.dumps({"sdk": sdk, "runs": [first_run, pi.events]}, ensure_ascii=False, indent=2) + "\n")
            print(json.dumps({"passed": True, "cases": ["public-sdk-success-error-abort", "virtual-selection-dispatch-thinking", "physical-context-window",
                  "codemode-image-classifier-cost", "image-live-replay", "resume-tree-fresh-load", "codemode-error-abort"], "events": events}))
        finally:
            pi.close()


if __name__ == "__main__":
    run()
