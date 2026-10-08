#!/usr/bin/env python3
"""Parse Git-tracked repository Rust files without editing them or invoking Cargo."""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


def tracked_rust_paths(workspace: Path) -> list[Path]:
    names = subprocess.check_output(["git", "ls-files", "--cached", "-z"], cwd=workspace).decode().split("\0")
    return [workspace / name for name in sorted(set(names)) if name.endswith(".rs")]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()
    workspace = args.workspace.resolve()
    paths = tracked_rust_paths(workspace)
    deleted = set(subprocess.check_output(["git", "diff", "--name-only", "--diff-filter=D", "HEAD"], cwd=workspace, text=True).splitlines())
    removed = []
    failures = {}
    parsed = []
    for path in paths:
        relative = path.relative_to(workspace).as_posix()
        if not path.exists() and relative in deleted:
            removed.append(relative)
            continue
        if not path.exists():
            failures[relative] = [{"kind": "missing_tracked_file"}]
            continue
        result = subprocess.run(
            ["rustfmt", "--edition", "2024", "--emit", "stdout", "--config", "skip_children=true", str(path)],
            cwd=workspace, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
        )
        parsed.append(relative)
        if result.returncode:
            failures[relative] = [{"kind": "rustfmt_parse_error", "diagnostics": result.stderr}]
    report = {
        "schemaVersion": 2, "proofLayer": "syntax-only", "sourceSelection": "git-ls-files-cached-recursive", "parserEdition": "2024",
        "passed": bool(paths) and not failures, "selectedFileCount": len(paths),
        "removedTrackedFiles": removed, "parsedFileCount": len(parsed), "parsedFiles": parsed, "failures": failures,
        "limits": ["Includes tracked tests and third_party Rust", "No type, feature-profile or runtime proof", "Untracked Rust and submodule contents are not included"],
    }
    output = args.json_out or workspace / "verification-logs/rust-syntax-verification.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(f"Parsed {len(parsed)} of {len(paths)} Git-tracked Rust files; failures={len(failures)}")
    for name, errors in failures.items():
        print(f"[FAIL] {name}: {errors}")
    print(f"Result: {'PASS' if report['passed'] else 'FAIL'} (syntax only)")
    print(f"Report: {output}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
