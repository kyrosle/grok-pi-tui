#!/usr/bin/env python3
"""Small negative checks for adapter purity, staged dependencies and UI/endpoint guards."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile

import verify_native_grok as architecture
import scan_product_endpoints as endpoints

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location("dependency_profile", ROOT / "crates/codegen/pi-grok-adapter/tests/pi_dependency_profile.py")
dependencies = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dependencies)

# A Cargo alias must not hide a Pager/terminal dependency from the adapter guard.
assert architecture.adapter_dependency_hits('[dependencies]\nwire = "1"\n') == []
for package in ["ratatui", "xai-grok-pager", "xai-grok-pager-render", "xai-grok-pager-diff", "xai-grok-pager-minimal"]:
    assert architecture.adapter_dependency_hits(f'[dependencies]\nalias = {{ package = "{package}", version = "1" }}\n') == [package]

assert dependencies.parse_packages("0root v1.0.0 (/repo)\n12cpal v1.2.3 (*)\n├── root v1.0.0") == {"root v1.0.0", "cpal v1.2.3"}

assert architecture.adapter_dependency_hits('[dependencies]\nalias = { workspace = true }\n', '[workspace.dependencies]\nalias = { package = "xai-grok-pager", version = "1" }\n') == ["xai-grok-pager"]

# Transition debt is visible; already-cut dependencies fail even in report stages.
assert not dependencies.assess_dependencies({"cpal v1", "xai-grok-agent v1"})["passed"]
assert not dependencies.assess_dependencies({"root v1", "xai-grok-memory v1"})["passed"]
assert not dependencies.assess_dependencies({"root v1", "xai-grok-shell-new v1"})["passed"]
report = dependencies.assess_dependencies({"root v1", "sentry v1", "xai-grok-login v1"})
assert report["passed"] and len(report["pending"]) == 2 and not report["terminalReady"]
assert not dependencies.assess_dependencies({"root v1", "sentry v1"}, enforce=True)["passed"]

# A new or stock product command fails the actual local catalog comparator.
source = 'const PI_GROK_NATIVE_COMMANDS: &[&str] = &["help", "voice"];'
assert architecture.native_command_errors(source, ["help"], ["voice"])[1]
assert not architecture.native_command_errors(source.replace(', "voice"', ''), ["help"], ["voice"])[1]

# Test-only endpoints are removed, while production after a test item remains scanned.
source = '#[cfg(test)] mod tests { const URL: &str = "https://grok.com/test"; }\nconst URL: &str = "https://new.api.x.ai/live";'
hits = endpoints.scan_bytes(endpoints.production_only(source, ".rs").encode(), "src/lib.rs", "source")
assert len(hits) == 1 and hits[0]["value"] == "new.api.x.ai"

# ACP namespaces are observations, while actual URL authorities and hosts fail.
observations = []
assert not endpoints.scan_bytes(b'x.ai/session/info', "src/lib.rs", "source", observations)
assert observations[0]["kind"] == "bare-acp-namespace-literal"
for literal in [b'https://x.ai/session', b'ws://x.ai/session', b'https://user:password@x.ai/session', b'api.x.ai/session', b'"api.x.ai"', b'"x.ai"']:
    assert endpoints.scan_bytes(literal, "src/lib.rs", "source"), literal

private = "00112233445566778899aabbccddeeff"
hits = endpoints.scan_bytes(f"https://{private}@o123.ingest.sentry.io/1 xai-{private}".encode(), "binary", "binary")
assert {hit["kind"] for hit in hits} == {"sentry-domain", "xai-key-prefix"}
assert private not in json.dumps(hits)

# Report mode must expose hits without claiming a zero-endpoint pass; enforce fails.
with tempfile.TemporaryDirectory(prefix="pi-architecture-negative-") as directory:
    workspace = Path(directory)
    subprocess.run(["git", "init", "-q", str(workspace)], check=True)
    path = workspace / "crates/demo/src/lib.rs"
    path.parent.mkdir(parents=True)
    path.write_text('const URL: &str = "https://unreviewed.grok.com/api";')
    subprocess.run(["git", "-C", str(workspace), "add", "crates/demo/src/lib.rs"], check=True)
    binary = workspace / "fixture-binary"
    binary.write_bytes(b"https://api.x.ai/request")
    output = workspace / "report.json"
    argv = [sys.executable, str(Path(endpoints.__file__).resolve()), "--workspace", str(workspace), "--binary", str(binary), "--json-out", str(output)]
    result = subprocess.run(argv, capture_output=True, text=True)
    report = json.loads(output.read_text())
    assert result.returncode == 0 and not report["enforced"] and report["passed"] is None and report["hitCount"] == 2
    result = subprocess.run(argv + ["--enforce"], capture_output=True, text=True)
    assert result.returncode == 1 and json.loads(output.read_text())["passed"] is False
    # A sole untracked production endpoint must fail; ignored/test/dependency files stay out.
    path.write_text("// no service literals")
    binary.write_bytes(b"clean artifact")
    untracked = workspace / "crates/demo/src/new.rs"
    untracked.write_text('const URL: &str = "https://new.grok.com/api";')
    for relative in ["crates/demo/src/fixtures/test.rs", "extensions/demo/node_modules/pkg/index.ts", "crates/demo/src/ignored.rs"]:
        excluded = workspace / relative
        excluded.parent.mkdir(parents=True, exist_ok=True)
        excluded.write_text('"https://excluded.grok.com/api"')
    (workspace / ".gitignore").write_text("ignored.rs\n")
    result = subprocess.run(argv + ["--enforce"], capture_output=True, text=True)
    report = json.loads(output.read_text())
    assert result.returncode == 1 and report["sourceHitCount"] == 1 and report["binaryHitCount"] == 0
    assert report["hits"][0]["file"] == "crates/demo/src/new.rs"
    untracked.write_text('const METHOD: &str = "x.ai/session";')
    result = subprocess.run(argv + ["--enforce"], capture_output=True, text=True)
    report = json.loads(output.read_text())
    assert result.returncode == 0 and report["hitCount"] == 0 and report["protocolObservationCount"] == 1
    untracked.write_text('const URL: &str = "https://x.ai/session";')
    result = subprocess.run(argv + ["--enforce"], capture_output=True, text=True)
    assert result.returncode == 1 and json.loads(output.read_text())["sourceHitCount"] == 1
print("PASS: adapter dependency alias, staged forbidden policy, command whitelist, production-only endpoint and enforce negatives")
