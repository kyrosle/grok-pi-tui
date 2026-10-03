#!/usr/bin/env python3
"""Standard-library PTY ownership; JSONL bytes connect the installed screen emulator."""
import base64
import errno
import fcntl
import json
import os
import pty
import select
import signal
import struct
import sys
import termios

config = json.loads(sys.argv[1])
child, master = pty.fork()
if child == 0:
    os.chdir(config["cwd"])
    os.execvpe(config["argv"][0], config["argv"], dict(os.environ))
fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", config["rows"], config["cols"], 0, 0))
pending = b""
status = None
try:
    while status is None:
        for descriptor in select.select([master, sys.stdin.fileno()], [], [], 0.1)[0]:
            if descriptor == master:
                try:
                    data = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    data = b""
                if data:
                    print(json.dumps({"type": "output", "data": base64.b64encode(data).decode()}), flush=True)
            else:
                data = os.read(descriptor, 65536)
                if not data:
                    raise EOFError("PTY controller closed")
                pending += data
                while b"\n" in pending:
                    line, pending = pending.split(b"\n", 1)
                    command = json.loads(line)
                    if command["type"] == "close":
                        raise EOFError("PTY controller cleanup")
                    if command["type"] == "resize":
                        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", command["rows"], command["cols"], 0, 0))
                    else:
                        os.write(master, base64.b64decode(command["data"]))
        ended, result = os.waitpid(child, os.WNOHANG)
        if ended:
            status = os.waitstatus_to_exitcode(result)
    print(json.dumps({"type": "exit", "status": status}), flush=True)
except EOFError:
    pass
finally:
    try:
        os.killpg(child, signal.SIGTERM)
    except ProcessLookupError:
        pass
    os.close(master)
