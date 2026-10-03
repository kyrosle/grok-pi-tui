#!/usr/bin/env python3
"""Installed Pi→real production child SDK/live+load bridge; no external inference."""
import json
import os
from pathlib import Path
import queue
import socket
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[4]


def main():
    with tempfile.TemporaryDirectory(prefix="pi-child-sdk-") as directory:
        root = Path(directory)
        listener = socket.socket(socket.AF_UNIX)
        endpoint = root / "child.sock"
        listener.bind(str(endpoint)); listener.listen(1)
        env = {key: value for key, value in os.environ.items() if not (
            key.endswith("_API_KEY") or key.endswith("_TOKEN") or key.startswith("AWS_"))}
        env.update(PI_CODING_AGENT_DIR=str(root / "pi"), GROK_HOME=str(root / "grok"),
                   GROK_PROJECT_DIR=str(root / "project"), PI_GROK_SUBAGENTS="1",
                   PI_GROK_SUBAGENT_SOCKET=str(endpoint),
                   PI_OFFLINE="1", PI_TELEMETRY="0", PI_GROK_RPC_WATCHDOG="0")
        command = [env.get("PI_BIN", "pi"), "--mode", "rpc", "--offline", "-ne", "-ns", "-np", "-nc",
                   "--no-themes", "--no-approve", "--session-dir", str(root / "sessions"),
                   "--model", "pi-child-fixture/local", "--tools", "spawn_subagent",
                   "--extension", str(ROOT / "extensions/pi-grok-subagents/index.ts"),
                   "--extension", str(Path(__file__).parent / "fixtures/pi_subagent_provider.ts")]
        process = subprocess.Popen(command, cwd=root, env=env, text=True, bufsize=1,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        output, stderr = queue.Queue(), []
        def read_stdout():
            source = process.stdout
            for line in source:
                try: output.put(json.loads(line))
                except ValueError: pass
            output.put(None)
        def read_stderr():
            source = process.stderr
            stderr.extend(source)
        def read_bridge():
            while True:
                try: connection, _ = listener.accept()
                except OSError: return
                with connection, connection.makefile() as lines:
                    for line in lines:
                        output.put({"_bridge": json.loads(line)["data"]})
        stdout_thread = threading.Thread(target=read_stdout, daemon=True)
        stderr_thread = threading.Thread(target=read_stderr, daemon=True)
        stdout_thread.start(); stderr_thread.start()
        threading.Thread(target=read_bridge, daemon=True).start()
        def send(value):
            process.stdin.write(json.dumps(value) + "\n"); process.stdin.flush()
        events = []
        def until(predicate, seconds=30):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                event = output.get(timeout=max(.1, deadline - time.monotonic()))
                assert event is not None, "Pi EOF: " + "".join(stderr)
                events.append(event)
                if predicate(event): return event
            raise AssertionError("Pi fixture deadline")
        try:
            send({"id": "prompt", "type": "prompt", "message": "NATIVE_PARENT_REQUEST"})
            until(lambda event: event.get("type") == "agent_settled")
            messages = [event["message"] for event in events if event.get("type") == "message_end"]
            result = next(message for message in messages if message.get("role") == "toolResult" and message.get("toolName") == "spawn_subagent")
            assert not result.get("isError"), result
            assert "NATIVE_CHILD_BODY" in str(result), result
            send({"id": "state", "type": "get_state"})
            state = until(lambda event: event.get("id") == "state")["data"]
            parent = Path(state["sessionFile"])
            assert parent.is_file(), state
            send({"id": "entries", "type": "get_entries"})
            entries = until(lambda event: event.get("id") == "entries")["data"]["entries"]
            # Live bridge traffic is required; persisted snapshots alone cannot prove presentation.
            bridge = [event["_bridge"] for event in events if "_bridge" in event]
            assert any(item["kind"] == "child_update" and "NATIVE_CHILD_BODY" in str(item) for item in bridge), bridge
            boundary = len(events)
            send({"id": "replay", "type": "prompt", "message": '/__pi_grok_subagent_replay {"mode":"load","requestId":"fixture-replay"}'})
            until(lambda event: event.get("_bridge", {}).get("kind") == "replay_complete")
            replay = [event["_bridge"] for event in events[boundary:] if "_bridge" in event]
            assert any(item["kind"] == "child_update" and item["replay"] and "NATIVE_CHILD_BODY" in str(item) for item in replay), replay
            # Produce a real running durable child, then exercise the official recovery command
            # after an abrupt parent restart. No session JSONL or sidecar is edited by this test.
            send({"id": "background", "type": "prompt", "message": "NATIVE_PARENT_WAIT_REQUEST"})
            until(lambda event: event.get("type") == "agent_settled")
            if not any("NATIVE_CHILD_WAIT_REQUEST" in str(event.get("_bridge", {}).get("payload", {}).get("update", {})) for event in events):
                until(lambda event: "NATIVE_CHILD_WAIT_REQUEST" in str(event.get("_bridge", {}).get("payload", {}).get("update", {})))
            process.kill(); process.wait(timeout=5)
            stdout_thread.join(timeout=5); stderr_thread.join(timeout=5)
            while not output.empty(): output.get_nowait()
            process = subprocess.Popen(command + ["--session", str(parent)], cwd=root, env=env, text=True, bufsize=1,
                                       stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stdout_thread = threading.Thread(target=read_stdout, daemon=True)
            stderr_thread = threading.Thread(target=read_stderr, daemon=True)
            stdout_thread.start(); stderr_thread.start()
            boundary = len(events)
            send({"id": "recovery", "type": "prompt", "message": '/__pi_grok_subagent_replay {"mode":"recovery","requestId":"fixture-recovery"}'})
            until(lambda event: event.get("_bridge", {}).get("kind") == "replay_complete")
            recovery = [event["_bridge"] for event in events[boundary:] if "_bridge" in event]
            assert [item["kind"] for item in recovery] == ["spawned", "finished", "replay_complete"], recovery
            assert recovery[1]["payload"]["status"] == "cancelled", recovery
            assert recovery[0]["subagentId"] == recovery[1]["subagentId"], recovery
            assert not any(item["replay"] for item in recovery), recovery
            # A second recovery settles no completed/cancelled child and cannot duplicate history.
            boundary = len(events)
            send({"id": "recovery-again", "type": "prompt", "message": '/__pi_grok_subagent_replay {"mode":"recovery","requestId":"fixture-recovery-again"}'})
            until(lambda event: event.get("_bridge", {}).get("kind") == "replay_complete")
            repeated = [event["_bridge"] for event in events[boundary:] if "_bridge" in event]
            assert [item["kind"] for item in repeated] == ["replay_complete"], repeated
            report = {"piBinary": command[0], "provider": "root-only-extension-registration", "live": bridge, "replay": replay,
                      "recovery": recovery, "repeatedRecovery": repeated, "persistedEntries": len(entries),
                      "proof": "actual Pi SDK + production subagent bridge; ACP/native requires adapter fixture"}
            if path := os.environ.get("PI_SUBAGENT_SDK_CAPTURE"):
                Path(path).write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
            print(json.dumps({"passed": True, "live": len(bridge), "replay": len(replay), "recovery": len(recovery), "child": "NATIVE_CHILD_BODY"}))
        finally:
            listener.close()
            process.terminate()
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired: process.kill(); process.wait(timeout=5)


if __name__ == "__main__": main()
