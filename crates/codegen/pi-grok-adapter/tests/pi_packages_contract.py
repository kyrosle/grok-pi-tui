#!/usr/bin/env python3
"""Actual installed Pi, isolated local package lifecycle; no model or network calls."""
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[4]
PI = os.environ.get("PI_BIN") or shutil.which("pi")
if not PI:
    raise SystemExit("pi executable required")


class Rpc:
    def __init__(self, cwd, env, bridge):
        self.process = subprocess.Popen(
            [PI, "--mode", "rpc", "--no-session", "--approve", "-e", str(bridge)],
            cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, bufsize=1,
        )
        self.lines = queue.Queue()
        self.readers = []
        for name, stream in [("stdout", self.process.stdout), ("stderr", self.process.stderr)]:
            reader = threading.Thread(target=self.read_lines, args=(name, stream), daemon=True)
            reader.start()
            self.readers.append(reader)
        self.next_id = 0
        self.stderr = []
        self.events = []

    def read_lines(self, name, stream):
        for line in stream:
            self.lines.put((name, line))

    def request(self, command):
        self.next_id += 1
        request_id = str(self.next_id)
        self.process.stdin.write(json.dumps({**command, "id": request_id}) + "\n")
        self.process.stdin.flush()
        deadline = time.monotonic() + 30
        stderr = []
        while time.monotonic() < deadline:
            try:
                name, line = self.lines.get(timeout=0.2)
            except queue.Empty:
                if self.process.poll() is not None:
                    raise AssertionError("Pi exited: " + "".join(stderr))
                continue
            if name == "stderr":
                stderr.append(line)
                self.stderr.append(line)
                continue
            event = json.loads(line)
            self.events.append(event)
            if event.get("type") == "response" and event.get("id") == request_id:
                assert event.get("success"), event
                return event.get("data")
            if self.process.poll() is not None:
                raise AssertionError("Pi exited: " + "".join(stderr))
        raise AssertionError("Pi request timed out: " + "".join(stderr))

    def snapshot(self, path):
        self.request({"type": "prompt", "message": "/__pi_package_snapshot " + json.dumps({"responsePath": str(path)})})
        return json.loads(path.read_text())

    def reload(self):
        self.request({"type": "prompt", "message": "/__pi_reload"})

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)
        for reader in self.readers:
            reader.join(timeout=1)


def main():
    source = (ROOT / "crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/tree_bridge.rs").read_text().split('r#"', 1)[1].split('"#;', 1)[0]
    with tempfile.TemporaryDirectory(prefix="pi-packages-contract-") as temporary:
        root = Path(temporary)
        agent = root / "agent"
        project = root / "project"
        package = root / "local-package"
        for path in [agent, project, package / "extensions"]:
            path.mkdir(parents=True)
        (package / "package.json").write_text(json.dumps({"name": "pi-fixture-package", "pi": {"extensions": ["extensions/index.ts"]}}))
        (package / "extensions/index.ts").write_text('export default function(pi) { pi.registerCommand("fixture-package", { description: "Local package fixture", handler: async () => {} }); }')
        bridge = root / "bridge.ts"
        bridge.write_text(source)
        env = {**os.environ, "PI_CODING_AGENT_DIR": str(agent), "PI_OFFLINE": "1", "PI_GROK": "1"}
        for key in ["NODE_OPTIONS", "PI_GROK_REMOTE_TUI", "PI_CODING_AGENT_SESSION_DIR"]:
            env.pop(key, None)
        rpc = Rpc(project, env, bridge)
        cli = lambda *args: subprocess.run([PI, *args], cwd=project, env=env, capture_output=True, text=True, timeout=30, check=True)
        snapshot_path = root / "snapshot.json"
        try:
            assert not rpc.snapshot(snapshot_path)["packages"]
            cli("install", str(package), "--approve")
            declared = rpc.snapshot(snapshot_path)
            assert declared["projectTrusted"] is True
            assert declared["packages"][0]["scope"] == "user"
            assert not any(command["name"] == "fixture-package" for command in declared["commands"])
            rpc.reload()
            loaded = rpc.snapshot(snapshot_path)
            assert any(command["name"] == "fixture-package" for command in loaded["commands"])
            # Fresh official CLI reads retain concurrent/unrelated settings and
            # an existing object source's filters instead of replacing it.
            settings_path = agent / "settings.json"
            settings = json.loads(settings_path.read_text())
            settings["packages"] = [{"source": str(package), "extensions": [], "skills": []}]
            settings["fixtureUnrelated"] = {"kept": True}
            settings_path.write_text(json.dumps(settings))
            cli("install", str(package), "--approve")
            preserved = json.loads(settings_path.read_text())
            assert preserved["fixtureUnrelated"] == {"kept": True}
            assert preserved["packages"][0]["extensions"] == []
            rpc.reload()
            assert not any(command["name"] == "fixture-package" for command in rpc.snapshot(snapshot_path)["commands"])
            cli("install", str(package), "--local", "--approve")
            rpc.reload()
            assert any(command["name"] == "fixture-package" for command in rpc.snapshot(snapshot_path)["commands"])
            # Removing Global leaves Project active; removing local declarations
            # never deletes the source directory.
            cli("remove", str(package), "--approve")
            rpc.reload()
            assert any(command["name"] == "fixture-package" for command in rpc.snapshot(snapshot_path)["commands"])
            cli("update", str(package), "--approve")
            cli("update", "--extensions", "--approve")
            cli("remove", str(package), "--local", "--approve")
            rpc.reload()
            assert not any(command["name"] == "fixture-package" for command in rpc.snapshot(snapshot_path)["commands"])
            assert (package / "extensions/index.ts").is_file()
            assert not rpc.snapshot(snapshot_path)["packages"]
            # Loader diagnostics are a separate proof layer: official reload
            # can finish while an extension fails to join the live registry.
            (package / "extensions/index.ts").write_text('import "./pi-fixture-missing.ts"; export default () => {};')
            cli("install", str(package), "--approve")
            errors_before = len(rpc.events)
            stderr_before = len(rpc.stderr)
            rpc.reload()
            invalid = rpc.snapshot(snapshot_path)
            assert invalid["loaded"] is None
            assert invalid["loadStatus"] == "unverified"
            assert not any(command["name"] == "fixture-package" for command in invalid["commands"])
            errors = [event for event in rpc.events[errors_before:] if event.get("type") == "extension_error"]
            print("INVALID_LOADER_DIAGNOSTIC " + json.dumps({"stderr": rpc.stderr[stderr_before:], "extensionErrors": errors}))
            print("PASS actual Pi local package lifecycle: installed vs loaded, filters, project precedence, fresh settings, update targets, local remove")
        finally:
            rpc.close()


if __name__ == "__main__":
    main()
