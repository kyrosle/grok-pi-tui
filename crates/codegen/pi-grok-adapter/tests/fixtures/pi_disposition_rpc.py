"""Pi-shaped RPC fixture: responses are authoritative before delayed events."""
import json
import os
import sys
import threading
import time

lock = threading.Lock()
streaming = False
cancelled = False
trace_path = os.environ["PI_DISPOSITION_TRACE"]


def emit(value):
    with lock:
        print(json.dumps(value), flush=True)


def record(value):
    with lock, open(trace_path, "a") as trace:
        trace.write(json.dumps(value) + "\n")


def run(delay):
    global streaming
    time.sleep(delay)
    if cancelled:
        return
    streaming = True
    record({"event": "agent_start"})
    emit({"type": "agent_start"})
    time.sleep(0.04)
    streaming = False
    emit({"type": "agent_end", "messages": []})
    record({"event": "agent_settled"})
    emit({"type": "agent_settled"})


for line in sys.stdin:
    command = json.loads(line)
    kind = command["type"]
    record({"command": kind})
    data = {}
    if kind == "get_state":
        data = {"sessionId": "disposition", "thinkingLevel": "off",
                "isStreaming": streaming, "isCompacting": False,
                "autoCompactionEnabled": True}
    elif kind == "get_available_models":
        data = {"models": []}
    elif kind == "get_commands":
        data = {"commands": []}
    elif kind == "get_session_stats":
        data = {"contextUsage": {"tokens": 0, "contextWindow": 32768}}
    elif kind == "prompt":
        case = command["message"]
        if case == "handled":
            data = {"disposition": "handled"}
        elif case == "event_first":
            run(0)
            data = {"disposition": "started"}
        else:
            data = {"disposition": "queued" if case == "queued" else "started"}
            threading.Thread(target=run, args=(0.15,), daemon=True).start()
    elif kind in ("abort", "clear_queue"):
        cancelled = True
    emit({"type": "response", "id": command.get("id"),
          "command": kind, "success": True, "data": data})
