#!/usr/bin/env python3
"""Check Pi's production normal/build graph against staged removal policy."""
import argparse
from fnmatch import fnmatchcase
import json
import os
import re
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
STOCK = {
    "xai-grok-agent", "xai-grok-tools", "xai-grok-workspace", "xai-grok-mcp",
    "xai-grok-sampler", "xai-grok-shell", "xai-grok-plugin-marketplace",
}
AUDIO = {"cpal", "alsa-sys", "coreaudio-rs", "coreaudio-sys"}
# Move each reviewed removal from pending to forbidden when its cut lands.
FORBIDDEN = STOCK | AUDIO
REMOVAL_PATTERNS = {
    "xai-grok-agent", "xai-grok-tools", "xai-grok-tools-api", "xai-grok-shell*",
    "xai-grok-workspace*", "xai-grok-mcp", "xai-grok-plugin-marketplace",
    "xai-grok-sampler", "xai-grok-sampling-types", "xai-grok-login", "xai-grok-auth",
    "xai-grok-secrets", "xai-grok-telemetry", "xai-grok-otel", "xai-grok-announcements",
    "xai-grok-feedback", "xai-grok-gboom", "xai-grok-dashboard-store", "xai-grok-voice",
    "xai-grok-memory", "xai-grok-session-search", "xai-grok-diag-server",
    "xai-computer-hub-*", "xai-grok-bundle", "xai-grok-models",
    "sentry*", "mixpanel*", "opentelemetry*",
} | {"xai-mixpanel"}

# Only already-linked removal debt may remain pending. New business dependencies fail.
PENDING = {
    "xai-mixpanel",
    "opentelemetry",
    "opentelemetry-http",
    "opentelemetry-otlp",
    "opentelemetry-proto",
    "opentelemetry_sdk",
    "sentry",
    "sentry-anyhow",
    "sentry-backtrace",
    "sentry-contexts",
    "sentry-core",
    "sentry-debug-images",
    "sentry-panic",
    "sentry-tracing",
    "sentry-types",
    "xai-grok-announcements",
    "xai-grok-auth",
    "xai-grok-dashboard-store",
    "xai-grok-feedback",
    "xai-grok-gboom",
    "xai-grok-login",
    "xai-grok-models",
    "xai-grok-otel",
    "xai-grok-sampling-types",
    "xai-grok-secrets",
    "xai-grok-shell-base",
    "xai-grok-telemetry",
    "xai-grok-voice",
    "xai-grok-workspace-types",
}


def parse_packages(output: str) -> set[str]:
    # Accept cargo tree --prefix none/depth and normal Unicode tree indentation.
    return {f"{match[1]} v{match[2]}" for line in output.splitlines()
            if (match := re.search(r"([A-Za-z_][A-Za-z0-9_-]*) v([0-9][^\s]*)", line))}


def assess_dependencies(packages: set[str], enforce: bool = False) -> dict:
    def matches(name: str, patterns: set[str]) -> bool:
        return any(fnmatchcase(name, pattern) for pattern in patterns)
    forbidden = sorted(p for p in packages if matches(p.split()[0], FORBIDDEN)
                       or (matches(p.split()[0], REMOVAL_PATTERNS) and p.split()[0] not in PENDING))
    pending = sorted(p for p in packages if p.split()[0] in PENDING and p not in forbidden)
    return {
        "packages": len(packages), "stock_runtime": sorted(p for p in packages if p.split()[0] in STOCK),
        "audio_backends": sorted(p for p in packages if p.split()[0] in AUDIO),
        "forbidden": forbidden, "pending": pending, "enforceAllRemovals": enforce,
        "passed": bool(packages) and not forbidden and (not enforce or not pending),
        "terminalReady": bool(packages) and not forbidden and not pending,
        "policySource": "docs/issues/架构/20261003-pi-native-tui-SPEC.md#53-crate-初始分类",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--enforce", action="store_true", help="T7: reject every pending removal too")
    parser.add_argument("--tree-input", type=Path, help="Read a captured production cargo tree; does not invoke Cargo")
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()
    if args.tree_input:
        output = args.tree_input.read_text()
        source = {"kind": "captured-production-tree", "path": str(args.tree_input.resolve())}
    else:
        result = subprocess.run(
            [str(ROOT / "scripts/cargo-shared.sh"), "tree", "--locked", "-p", "xai-grok-pager-bin",
             "--no-default-features", "--features", "jemalloc,sandbox-enforce", "--edges", "normal,build",
             "--prefix", "none", "--format", "{p}"],
            cwd=ROOT, env=os.environ | {"CARGO_MAINTENANCE": "0"}, capture_output=True, text=True,
        )
        if result.returncode:
            sys.stderr.write(result.stderr)
            return result.returncode
        output = result.stdout
        source = {"kind": "fresh-cargo-tree", "edges": "normal,build", "profile": "Pi no-default jemalloc,sandbox-enforce"}
    packages = parse_packages(output)
    report = assess_dependencies(packages, args.enforce) | {"graphSource": source}
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.json_out:
        args.json_out.write_text(encoded)
    print(encoded, end="")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
