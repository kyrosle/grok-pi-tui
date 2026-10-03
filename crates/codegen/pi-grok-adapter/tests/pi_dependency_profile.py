#!/usr/bin/env python3
"""Check the production Pi composition graph, excluding test/stock features."""
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
STOCK = {
    "xai-grok-agent", "xai-grok-tools", "xai-grok-workspace", "xai-grok-mcp",
    "xai-grok-sampler", "xai-grok-shell", "xai-grok-plugin-marketplace",
}
AUDIO = {"cpal", "alsa-sys", "coreaudio-rs", "coreaudio-sys"}


def main():
    result = subprocess.run(
        [str(ROOT / "scripts/cargo-shared.sh"), "tree", "--locked",
         "-p", "xai-grok-pager-bin", "--no-default-features",
         "--features", "jemalloc,sandbox-enforce", "--edges", "normal,build",
         "--prefix", "none", "--format", "{p}"],
        cwd=ROOT, env=os.environ | {"CARGO_MAINTENANCE": "0"},
        capture_output=True, text=True,
    )
    if result.returncode:
        sys.stderr.write(result.stderr)
        raise SystemExit(result.returncode)
    packages = {line.removesuffix(" (*)").strip()
                for line in result.stdout.splitlines() if " v" in line}
    retained = sorted(package for package in packages
                      if package.split()[0] in STOCK)
    audio = sorted(package for package in packages
                   if package.split()[0] in AUDIO)
    print(json.dumps({"packages": len(packages), "stock_runtime": retained,
                      "audio_backends": audio}, indent=2))
    assert packages and not retained, "Pi production profile still links stock runtime"
    assert not audio, "Pi production profile still links a microphone backend"


if __name__ == "__main__":
    main()
