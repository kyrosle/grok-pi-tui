#!/usr/bin/env python3
"""Exercise install.sh with a real package, without network or user state.

Usage: python3 scripts/test-macos-install.py dist/asset.tar.gz
"""

import hashlib
import os
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

archive = Path(sys.argv[1]).resolve()
installer = Path(__file__).resolve().parent.parent / "install.sh"
with tempfile.TemporaryDirectory(prefix="grok-pi install test ") as tmp:
    root = Path(tmp)
    shim = root / "shim"
    shim.mkdir()
    (shim / "curl").write_text('''#!/bin/sh
printf '%s\n' "$*" >> "$TEST_URL_LOG"
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then shift; cp "$TEST_ARCHIVE" "$1"; exit; fi
  shift
done
exit 1
''')
    (shim / "uname").write_text('#!/bin/sh\ncase "$1" in -s) echo "$TEST_OS";; -m) echo "$TEST_ARCH";; esac\n')
    (shim / "sw_vers").write_text('#!/bin/sh\necho "${TEST_MACOS_VERSION:-14.0}"\n')
    for p in shim.iterdir():
        p.chmod(0o755)
    destination = root / "installed bin"
    env = {**os.environ, "PATH": str(shim) + os.pathsep + os.environ["PATH"],
           "GROK_PI_INSTALL_DIR": str(destination), "GROK_PI_VERSION": "v0.1.10",
           "GROK_PI_REPO": "kyrosle/grok-pi-tui", "GROK_PI_SKIP_PI_HINT": "1",
           "GROK_HOME": str(root / "home"), "GROK_LEGACY_HOME": str(root / "legacy"),
           "TEST_ARCHIVE": str(archive), "TEST_URL_LOG": str(root / "urls"),
           "TEST_OS": "Darwin", "TEST_ARCH": "arm64"}
    for _ in range(2):
        subprocess.run(["sh", str(installer)], env=env, check=True, capture_output=True, text=True)
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            if member.isfile():
                installed = destination / member.name
                assert installed.is_file(), member.name
                assert hashlib.sha256(installed.read_bytes()).digest() == hashlib.sha256(tar.extractfile(member).read()).digest(), member.name
    for name in ["grok-pi", "pig", "pi-grok"]:
        output = subprocess.check_output([str(destination / name), "--version"], env=env, text=True)
        assert output.startswith("grok-pi 0.1.10 ["), output
        subprocess.run([str(destination / name), "--help"], env=env, check=True, capture_output=True)
    assert "https://github.com/kyrosle/grok-pi-tui/releases/download/v0.1.10/grok-pi-macos-aarch64.tar.gz" in (root / "urls").read_text()
    for os_name, arch in [("Darwin", "x86_64"), ("Linux", "aarch64")]:
        env.update(TEST_OS=os_name, TEST_ARCH=arch)
        result = subprocess.run(["sh", str(installer)], env=env, capture_output=True, text=True)
        assert result.returncode != 0 and "Apple Silicon" in result.stderr, result
    env.update(TEST_OS="Darwin", TEST_ARCH="arm64", TEST_MACOS_VERSION="13.6")
    result = subprocess.run(["sh", str(installer)], env=env, capture_output=True, text=True)
    assert result.returncode != 0 and "macOS 14" in result.stderr, result
print("PASS: first install, reinstall, aliases, relocated libraries/licenses, paths with spaces, unsupported platforms")
