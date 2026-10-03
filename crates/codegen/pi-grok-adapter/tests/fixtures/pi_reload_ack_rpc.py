"""Public RPC fixture: a command can be handled while its reload business ACK fails."""
import json
import os
from pathlib import Path
import sys

for line in sys.stdin:
    command = json.loads(line)
    kind = command["type"]
    with open(os.environ["PI_RELOAD_ACK_TRACE"], "a") as trace:
        trace.write(json.dumps(command) + "\n")
    data = {}
    if kind == "get_state":
        data = {"sessionId": "reload-ack", "isStreaming": False, "isCompacting": False}
    elif kind == "get_available_models":
        data = {"models": []}
    elif kind == "get_commands":
        data = {"commands": [{"name": "__pi_reload", "source": "extension"}]}
    elif kind == "prompt":
        assert command["message"].startswith("/__pi_reload ")
        arguments = json.loads(command["message"].split(" ", 1)[1])
        mode = os.environ["PI_RELOAD_ACK_MODE"]
        if mode != "missing":
            acknowledgement = {"ok": False, "error": "fixture ctx.reload rejected"}
            if mode == "not_boolean":
                acknowledgement = {"ok": "true"}
            Path(arguments["responsePath"]).write_text(json.dumps(acknowledgement))
        # This is the actual Pi command-catch behavior: handler failure is not
        # a failed RPC response. Only the business ACK carries that outcome.
        data = {"disposition": "handled"}
    print(json.dumps({"type": "response", "id": command.get("id"),
                      "command": kind, "success": True, "data": data}), flush=True)
