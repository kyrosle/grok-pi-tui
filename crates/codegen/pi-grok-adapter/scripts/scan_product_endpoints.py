#!/usr/bin/env python3
"""Report Grok service/domain/key literals in repository production sources and an artifact."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

SOURCE_SUFFIXES = {".rs", ".ts", ".js", ".json", ".toml", ".sh", ".html"}
EXCLUDED_PARTS = {"tests", "test", "fixtures", "examples", "benches", "docs", "snapshots", "testdata", "test_data", "mocks", "test_support", "test_helpers", "__tests__", "node_modules", "target", ".git"}
PATTERNS = {
    "grok-service-domain": rb"(?<![a-z0-9_.-])(?:[a-z0-9-]+\.)*(?:x\.ai|grok\.com)(?![a-z0-9_.-])",
    "mixpanel-domain": rb"(?<![a-z0-9_.-])(?:[a-z0-9-]+\.)*mixpanel\.com(?![a-z0-9_.-])",
    "sentry-domain": rb"(?<![a-z0-9_.-])(?:[a-z0-9-]+\.)*sentry\.(?:io|com)(?![a-z0-9_.-])",
    "xai-key-prefix": rb"\bxai-[a-z0-9]{16,}",
}
COMPILED = {name: re.compile(pattern, re.I) for name, pattern in PATTERNS.items()}


def production_only(text: str, suffix: str) -> str:
    """Mask explicit Rust cfg(test) items, preserving source line locations."""
    if suffix != ".rs":
        return text
    # Strings/comments are masked only while locating balanced item braces.
    token = re.compile(r'//[^\n]*|/\*[\s\S]*?\*/|r(?P<hashes>#{0,16})"[\s\S]*?"(?P=hashes)|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'')
    masked = token.sub(lambda m: re.sub(r"[^\n]", " ", m.group()), text)
    ranges = []
    for match in re.finditer(r"#\[cfg\(test\)\]", masked):
        start = match.start()
        brace = masked.find("{", match.end())
        semicolon = masked.find(";", match.end())
        if semicolon >= 0 and (brace < 0 or semicolon < brace):
            ranges.append((start, semicolon + 1))
            continue
        if brace < 0:
            continue
        depth = 1
        end = brace + 1
        while end < len(masked) and depth:
            depth += (masked[end] == "{") - (masked[end] == "}")
            end += 1
        if depth == 0:
            ranges.append((start, end))
    for start, end in reversed(ranges):
        text = text[:start] + re.sub(r"[^\n]", " ", text[start:end]) + text[end:]
    return text


def production_paths(workspace: Path) -> list[Path]:
    names = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=workspace).decode().split("\0")
    selected = []
    for name in sorted(set(names)):
        path = Path(name)
        lower = path.name.lower()
        if not name.startswith(("crates/", "extensions/")) or path.suffix not in SOURCE_SUFFIXES:
            continue
        if set(path.parts) & EXCLUDED_PARTS or any(word in lower for word in ("license", "licence", "notices")):
            continue
        if any(part.endswith(("-tests", "-test-support", "-pty-harness")) for part in path.parts):
            continue
        if lower in {"tests.rs", "test.rs", "test_helpers.rs"} or lower.endswith(("_tests.rs", "_test.rs")) or ".test." in lower or ".spec." in lower:
            continue
        if name.startswith("crates/") and "src" not in path.parts:
            continue
        if (workspace / path).is_file():
            selected.append(workspace / path)
    return selected


def scan_bytes(data: bytes, path: str, scope: str, protocol_observations: list[dict] | None = None) -> list[dict]:
    hits = []
    for kind, pattern in COMPILED.items():
        for match in pattern.finditer(data):
            value = "xai-[REDACTED]" if kind == "xai-key-prefix" else match.group().decode("ascii").lower()
            hit = {"scope": scope, "file": path, "kind": kind, "value": value}
            if scope == "source":
                hit["line"] = data[:match.start()].count(b"\n") + 1
            else:
                hit["byteOffset"] = match.start()
            if kind == "grok-service-domain" and value == "x.ai":
                method = re.match(rb"/[A-Za-z_][A-Za-z0-9_./:-]*", data[match.end():])
                # A URL authority can contain userinfo; never emit that prefix.
                start = match.start()
                while start and data[start - 1] > 32 and data[start - 1] not in b'"<>':
                    start -= 1
                prefix = data[start:match.start()]
                url = re.search(rb"(?:https?|wss?)://[^/\s\"<>]*$", prefix, re.I)
                if method and not url:
                    hit["kind"] = "bare-acp-namespace-literal"
                    hit["value"] = "x.ai" + method.group().decode("ascii")
                    if protocol_observations is not None:
                        protocol_observations.append(hit)
                    continue
            hits.append(hit)
    return hits


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--binary", type=Path, help="Explicit product artifact; SHA is recorded, freshness is not inferred")
    parser.add_argument("--enforce", action="store_true", help="T7: require source+binary scan with zero hits")
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()
    if args.enforce and not args.binary:
        parser.error("--enforce requires --binary for product-artifact evidence")
    workspace = args.workspace.resolve()
    paths = production_paths(workspace)
    hits = []
    protocol_observations = []
    for path in paths:
        source = production_only(path.read_text(encoding="utf-8", errors="replace"), path.suffix)
        hits.extend(scan_bytes(source.encode(), str(path.relative_to(workspace)), "source", protocol_observations))
    artifact = {"status": "NOT_RUN"}
    if args.binary:
        data = args.binary.read_bytes()
        artifact = {"status": "SCANNED", "path": str(args.binary.resolve()), "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "freshness": "not inferred against current source"}
        hits.extend(scan_bytes(data, str(args.binary.resolve()), "binary", protocol_observations))
    passed = bool(paths) and not hits if args.enforce else None
    report = {
        "schemaVersion": 1, "mode": "enforce" if args.enforce else "baseline-report", "enforced": args.enforce, "passed": passed,
        "status": ("PASS" if passed else "FAIL") if args.enforce else "REPORTED",
        "sourceFileCount": len(paths), "sourceSelection": "Git tracked plus unignored untracked crates/src and extensions production files",
        "binary": artifact, "hitCount": len(hits), "sourceHitCount": sum(h["scope"] == "source" for h in hits),
        "binaryHitCount": sum(h["scope"] == "binary" for h in hits), "hits": hits,
        "protocolObservationCount": len(protocol_observations), "protocolObservations": protocol_observations,
        "limits": ["Literal scan, not network reachability or Rust feature resolution", "Bare x.ai/method literals are separate protocol observations; URL authorities and standalone domains remain violations", "Tests/fixtures/docs/license paths and explicit Rust cfg(test) items are excluded", "Baseline hits are observations, never a permanent allowlist", "No user state directories are scanned"],
    }
    output = args.json_out or workspace / "verification-logs/product-endpoints.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(f"Endpoint {report['status']}: sources={len(paths)}, sourceHits={report['sourceHitCount']}, binaryHits={report['binaryHitCount']}; binary={artifact['status']}; protocolObservations={len(protocol_observations)}")
    print(f"Report: {output}")
    return 0 if not args.enforce or passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
