#!/usr/bin/env python3
"""Parse every Pi integration Rust seam with the repository Rust toolchain."""
from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    workspace = args.workspace.resolve()
    adapter = workspace / "crates/codegen/pi-grok-adapter"
    manifest = json.loads(
        (adapter / "docs/grok_uploaded_baseline_sha256.json").read_text(encoding="utf-8")
    )

    paths: set[Path] = {
        workspace / rel
        for rel in manifest["allowedModifiedFiles"] + manifest["allowedAddedFiles"]
        if rel.endswith(".rs")
    }
    for path in list(paths):
        module_dir = path.parent if path.name == "mod.rs" else path.with_suffix("")
        if module_dir.is_dir():
            paths.update(module_dir.rglob("*.rs"))
    paths.update((adapter / "src").rglob("*.rs"))
    paths.update((workspace / "crates/codegen/xai-grok-pager-bin/src/bin/grok_pi").rglob("*.rs"))
    paths.update(workspace / f"crates/codegen/xai-workflow/src/{name}.rs" for name in ("backend", "tracker", "store"))
    failures: dict[str, list[dict[str, object]]] = {}
    parsed: list[str] = []

    for path in sorted(paths):
        if not path.exists():
            failures[path.relative_to(workspace).as_posix()] = [
                {"kind": "missing_file", "start": [0, 0], "end": [0, 0]}
            ]
            continue
        relative = path.relative_to(workspace).as_posix()
        # stdout emission parses without writing/reformatting the source. It
        # checks syntax only; Cargo checks names/types and runtime checks behavior.
        result = subprocess.run(
            ["rustfmt", "--edition", "2024", "--emit", "stdout", "--config", "skip_children=true", str(path)],
            cwd=workspace, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
        )
        parsed.append(relative)
        if result.returncode != 0:
            failures[relative] = [{"kind": "rustfmt_parse_error", "diagnostics": result.stderr}]

    report = {
        "schemaVersion": 1,
        "passed": not failures,
        "parsedFileCount": len(parsed),
        "parsedFiles": parsed,
        "failures": failures,
    }
    output = args.json_out or adapter / "docs/rust-syntax-verification.json"
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    for relative in parsed:
        print(f"[{'FAIL' if relative in failures else 'PASS'}] {relative}")
    print(f"Result: {'PASS' if report['passed'] else 'FAIL'}")
    print(f"Report: {output}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
