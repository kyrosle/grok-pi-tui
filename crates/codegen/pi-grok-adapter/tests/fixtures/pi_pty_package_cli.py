#!/usr/bin/env python3
"""Forward real Pi; only the named cancellation fixture waits for termination."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

arguments = sys.argv[1:]
actual = os.environ["PI_PTY_ACTUAL_PI"]
def trace(value):
    with open(os.environ["PI_PTY_PACKAGE_TRACE"], "a") as output:
        output.write(json.dumps(value) + "\n")

# Record before Pi changes its OS process title, so policy/respawn argv remains inspectable.
trace({"event": "launcher", "argv": arguments, "pid": os.getpid()})
if arguments and arguments[0] in ("install", "remove", "update"):
    trace({"event": "started", "argv": arguments, "pid": os.getpid()})
    if arguments[0] == "install" and len(arguments) > 1 and Path(arguments[1]).name == "cancel-package":
        def cancel(_number, _frame):
            trace({"event": "cancelled", "argv": arguments})
            raise SystemExit(143)
        signal.signal(signal.SIGTERM, cancel)
        print("Waiting in the explicit package cancellation fixture", flush=True)
        while True:
            signal.pause()
    result = subprocess.run([actual, *arguments], check=False)
    trace({"event": "finished", "argv": arguments, "exit": result.returncode})
    raise SystemExit(result.returncode)
os.execv(actual, [actual, *arguments])
