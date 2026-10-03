#!/usr/bin/env python3
"""Read-only identity check plus negative source/coverage/provenance guards."""
import copy
import json
from pathlib import Path
from unittest.mock import patch

import verify_native_grok as verifier

root = Path(__file__).resolve().parents[4]
docs = root / "crates/codegen/pi-grok-adapter/docs"
manifest = json.loads((docs / "grok_uploaded_baseline_sha256.json").read_text())
renderer = json.loads((docs / "native_renderer_sha256.json").read_text())
result = verifier.verify_source_identity(root, manifest, renderer)
for key in ("baselineErrors", "declarationErrors", "identityErrors", "rendererErrors"):
    assert not result[key], (key, result[key])

# One tampered run checks that a protected native byte change, a forged source
# hash, incomplete renderer inventory, and a directory exemption all fail.
bad_manifest, bad_renderer = copy.deepcopy(manifest), copy.deepcopy(renderer)
protected = next(p for p in renderer["files"] if p not in manifest["historicalFiles"] and p not in manifest["phaseSeams"])
bad_manifest["baselineFiles"][protected] = "0" * 64
bad_manifest["allowedAddedPrefixes"] = ["crates/"]
reviewed = next(iter(manifest["phaseSeams"]))
bad_manifest["phaseSeams"][reviewed]["sha256"] = "0" * 64
bad_renderer["files"].pop(protected)
original_sha256 = verifier.sha256
original_files = verifier.source_files
with patch.object(verifier, "source_files", lambda path: original_files(path) | {"undeclared-native.rs"}), patch.object(verifier, "sha256", lambda path: "0" * 64 if path == root / protected else original_sha256(path)):
    rejected = verifier.verify_source_identity(root, bad_manifest, bad_renderer)
for key in ("baselineErrors", "declarationErrors", "identityErrors", "rendererErrors"):
    assert rejected[key], key
assert any(protected in error for error in rejected["identityErrors"])
assert any("undeclared-native.rs" in error for error in rejected["identityErrors"])
assert any(f"reviewed phase bytes changed: {reviewed}" == error for error in rejected["identityErrors"])

# The sole self-referential metadata entry hashes canonical content. Both a
# changed content field and a forged embedded SHA must still be rejected.
self_path = verifier.PHASE_MANIFEST_PATH
self_manifest = copy.deepcopy(manifest)
self_manifest["phaseSeams"][self_path]["sha256"] = verifier.phase_sha256(root, self_path)
for tamper_content in (False, True):
    bad_self = copy.deepcopy(self_manifest)
    if tamper_content:
        changed_content = copy.deepcopy(manifest)
        changed_content["phaseSeams"][self_path]["reason"] += " tampered"
        original_read = verifier.read
        with patch.object(verifier, "read", lambda path: json.dumps(changed_content) if path == root / self_path else original_read(path)):
            rejected_self = verifier.verify_source_identity(root, bad_self, renderer)
    else:
        bad_self["phaseSeams"][self_path]["sha256"] = "0" * 64
        rejected_self = verifier.verify_source_identity(root, bad_self, renderer)
    assert any(f"reviewed phase bytes changed: {self_path}" == error for error in rejected_self["identityErrors"])
print("PASS: exact Git-source identity and negative protected-byte/coverage/provenance/self-content/self-SHA guards")
