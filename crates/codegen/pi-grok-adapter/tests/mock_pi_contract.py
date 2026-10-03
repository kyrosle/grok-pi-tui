#!/usr/bin/env python3
"""Exercise the deterministic Pi JSONL fixture and audit the Rust routing contract."""
from __future__ import annotations

import argparse
import json
import queue
import threading
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from pi_contract_sources import load_pi_contract


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pi-source", type=Path, required=True)
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    mock = root / "tests/mock_pi_rpc.py"
    proc = subprocess.Popen(
        [sys.executable, str(mock)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    assert proc.stdin and proc.stdout and proc.stderr
    output_lines: queue.Queue[str | None] = queue.Queue()

    def drain_stdout() -> None:
        for line in proc.stdout:
            output_lines.put(line)
        output_lines.put(None)

    reader = threading.Thread(target=drain_stdout, name="mock-pi-stdout", daemon=True)
    reader.start()

    received: list[dict[str, Any]] = []
    next_id = 1

    def send(kind: str, **payload: Any) -> str:
        nonlocal next_id
        request_id = f"contract-{next_id}"
        next_id += 1
        value = {"id": request_id, "type": kind, **payload}
        proc.stdin.write(json.dumps(value, separators=(",", ":")) + "\n")
        proc.stdin.flush()
        return request_id

    def send_ui_response(event: dict[str, Any]) -> None:
        method = event["method"]
        response: dict[str, Any] = {"type": "extension_ui_response", "id": event["id"]}
        if method == "confirm":
            response["confirmed"] = True
        elif method in {"select", "input", "editor"}:
            response["value"] = {"select": "first", "input": "value", "editor": "edited"}[method]
        else:
            return
        proc.stdin.write(json.dumps(response, separators=(",", ":")) + "\n")
        proc.stdin.flush()

    def read_until(predicate, timeout: float = 5.0) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("timed out waiting for mock Pi output")
            try:
                line = output_lines.get(timeout=remaining)
            except queue.Empty as exc:
                raise TimeoutError("timed out waiting for mock Pi output") from exc
            if line is None:
                raise RuntimeError("mock Pi closed stdout")
            value = json.loads(line)
            received.append(value)
            if value.get("type") == "extension_ui_request":
                send_ui_response(value)
            if predicate(value):
                return value

    for kind in ["get_state", "get_available_models", "get_commands", "get_messages"]:
        request_id = send(kind)
        response = read_until(lambda value, rid=request_id: value.get("type") == "response" and value.get("id") == rid)
        assert response["success"] is True
        assert response["command"] == kind
        assert "data" in response

    prompt_id = send("prompt", message="contract smoke", images=[])
    response = read_until(lambda value: value.get("type") == "response" and value.get("id") == prompt_id)
    assert response["success"] is True and response["command"] == "prompt"
    read_until(lambda value: value.get("type") == "agent_settled")

    ui_methods = {
        value["method"] for value in received if value.get("type") == "extension_ui_request"
    }
    expected_ui = {
        "notify", "setStatus", "setWidget", "setTitle", "set_editor_text",
        "select", "confirm", "input", "editor",
    }
    event_types = {value.get("type") for value in received}
    expected_events = {
        "agent_start", "turn_start", "message_start", "message_update", "tool_execution_start",
        "tool_execution_update", "tool_execution_end", "message_end", "turn_end", "agent_end",
        "agent_settled",
    }

    adapter_source = "\n".join(
        p.read_text(encoding="utf-8") for p in sorted((root / "src").rglob("*.rs"))
    )
    rust_ui_tokens = {
        "notify": '"notify"',
        "setStatus": '"setstatus"',
        "setWidget": '"setwidget"',
        "setTitle": '"settitle"',
        "set_editor_text": '"set_editor_text"',
        "select": '"select"',
        "confirm": '"confirm"',
        "input": '"input"',
        "editor": '"editor"',
    }

    pi_identity, pi_rpc_types, _, _ = load_pi_contract(args.pi_source)
    settled = adapter_source.partition('"agent_settled" => {')[2].split('\n            "', 1)[0]
    checks = {
        "mock_ui_methods": ui_methods == expected_ui,
        "mock_stream_events": expected_events <= event_types,
        "rust_routes_all_ui_methods": all(token in adapter_source for token in rust_ui_tokens.values()),
        "pi_rpc_declares_all_ui_methods": all(f'method: "{method}"' in pi_rpc_types for method in expected_ui),
        "agent_settled_is_completion_barrier": 'self.finish_prompts(acp::StopReason::EndTurn)' in settled
        and '"agent_end" => {}' in adapter_source,
        "history_is_requested": '"type": "get_messages"' in adapter_source,
        "pi_commands_are_discovered": '"type": "get_commands"' in adapter_source,
    }

    proc.stdin.close()
    proc.wait(timeout=3)
    stderr = proc.stderr.read()
    reader.join(timeout=1)

    report = {
        "passed": all(checks.values()) and proc.returncode == 0 and not stderr,
        "piContract": pi_identity,
        "checks": checks,
        "uiMethods": sorted(ui_methods),
        "eventTypes": sorted(str(value) for value in event_types),
        "receivedLines": len(received),
        "mockExitCode": proc.returncode,
        "mockStderr": stderr,
    }
    out = args.json_out or root / "docs/mock-pi-contract.json"
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for name, passed in checks.items():
        print(f"[{'PASS' if passed else 'FAIL'}] {name}")
    print(f"[{'PASS' if proc.returncode == 0 else 'FAIL'}] mock_exit={proc.returncode}")
    print(f"Result: {'PASS' if report['passed'] else 'FAIL'}")
    print(f"Report: {out}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
