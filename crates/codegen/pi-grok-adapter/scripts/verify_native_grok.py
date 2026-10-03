#!/usr/bin/env python3
"""Prove that Pi is hosted by the uploaded Grok Build TUI, not a replacement TUI.

The verifier checks architecture and exact layered source identity. Original
upstream Git blobs remain protected; committed integration deviations are
frozen separately, and current-phase seams require exact file provenance.
A Git merge or frozen integration snapshot is not semantic alignment proof.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from functools import lru_cache
from pathlib import Path
from typing import Any

from pi_contract_sources import load_pi_contract


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def source_files(root: Path) -> set[str]:
    # Git defines source candidates; ignored dependencies and build artifacts
    # are not native source. No native directory is exempted from identity checks.
    candidates = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=root,
    ).decode("utf-8").split("\0")
    return {name for name in candidates if name and (root / name).is_file()}


UPSTREAM_COMMIT = "37949780c144e37df692e3d669051a21fec24f20"
INTEGRATION_COMMIT = "222d614d9f12cc8fcd408d59419c7d4d197d1be3"
PHASE_MANIFEST_PATH = "crates/codegen/pi-grok-adapter/docs/grok_uploaded_baseline_sha256.json"
PHASE_MANIFEST_HASH_MODE = "canonical-json-without-self-sha256"
NATIVE_COMPONENT_PREFIXES = (
    "crates/codegen/xai-grok-pager/",
    "crates/codegen/xai-grok-pager-render/",
    "crates/codegen/xai-grok-pager-minimal/",
    "crates/codegen/xai-grok-pager-diff/",
    "crates/codegen/xai-grok-markdown/",
)


def phase_sha256(root: Path, relative: str) -> str:
    if relative != PHASE_MANIFEST_PATH:
        return sha256(root / relative)
    # The manifest cannot embed its own raw-byte SHA. Only this exact metadata
    # path hashes canonical content, omitting its single self SHA field.
    content = json.loads(read(root / relative))
    content["phaseSeams"][relative].pop("sha256", None)
    canonical = json.dumps(content, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


@lru_cache(maxsize=2)
def git_source(root: Path, revision: str) -> tuple[str, dict[str, str]]:
    """Hash source Git blobs, never accept working-tree bytes as a baseline."""
    tree = subprocess.check_output(
        ["git", "rev-parse", f"{revision}^{{tree}}"], cwd=root, text=True,
    ).strip()
    blobs: dict[str, str] = {}
    for row in subprocess.check_output(["git", "ls-tree", "-rz", revision], cwd=root).split(b"\0"):
        if row:
            meta, path = row.split(b"\t", 1)
            _mode, kind, oid = meta.decode().split()
            if kind == "blob":
                blobs[path.decode()] = oid
    oids = sorted(set(blobs.values()))
    result = subprocess.run(
        ["git", "cat-file", "--batch"], cwd=root, check=True,
        input=("\n".join(oids) + "\n").encode(), stdout=subprocess.PIPE,
    ).stdout
    hashes: dict[str, str] = {}
    offset = 0
    for oid in oids:
        end = result.index(b"\n", offset)
        actual, kind, size_text = result[offset:end].decode().split()
        if (actual, kind) != (oid, "blob"):
            raise ValueError(f"invalid Git blob response: {actual} {kind}")
        size = int(size_text)
        offset = end + 1
        hashes[oid] = hashlib.sha256(result[offset:offset + size]).hexdigest()
        offset += size + 1
    return tree, {path: hashes[oid] for path, oid in blobs.items()}


def verify_source_identity(
    root: Path, manifest: dict[str, Any], renderer: dict[str, Any],
) -> dict[str, Any]:
    upstream_tree, upstream = git_source(root, UPSTREAM_COMMIT)
    integration_tree, integration = git_source(root, INTEGRATION_COMMIT)
    historical = manifest.get("historicalFiles", {})
    phase = manifest.get("phaseSeams", {})
    current = source_files(root)
    baseline_errors: list[str] = []
    declaration_errors: list[str] = []
    identity_errors: list[str] = []
    renderer_errors: list[str] = []
    if manifest.get("schemaVersion") != 3:
        baseline_errors.append("layered manifest schemaVersion must be 3")
    if manifest.get("upstreamSource") != {
        "commit": UPSTREAM_COMMIT,
        "sourceRevision": "c4ea71cfdbcdb21e32e41bc25a0043d7d4836714",
        "tree": upstream_tree,
    }:
        baseline_errors.append("upstream source commit/revision/tree mismatch")
    if manifest.get("integrationSource") != {"commit": INTEGRATION_COMMIT, "tree": integration_tree}:
        baseline_errors.append("frozen integration source commit/tree mismatch")
    if manifest.get("baselineFiles") != upstream or manifest.get("baselineFileCount") != len(upstream):
        baseline_errors.append("upstream file inventory/hashes differ from source Git blobs")

    expected_historical = {
        path: "added" if path not in upstream else "removed" if path not in integration else "modified"
        for path in upstream.keys() | integration.keys()
        if upstream.get(path) != integration.get(path)
    }
    if set(historical) != set(expected_historical):
        declaration_errors.append(f"historical inventory mismatch: {sorted(set(historical) ^ set(expected_historical))}")
    commit_reasons: dict[str, str] = {}
    for path, item in historical.items():
        commit = item.get("lastChangeCommit", "")
        if commit not in commit_reasons and re.fullmatch(r"[0-9a-f]{40}", commit):
            result = subprocess.run(
                ["git", "show", "-s", "--format=%s", commit], cwd=root,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            )
            commit_reasons[commit] = result.stdout.strip() if result.returncode == 0 else ""
        if (item.get("kind") != expected_historical.get(path)
            or item.get("sourceCommit") != INTEGRATION_COMMIT
            or item.get("sha256") != integration.get(path)
            or not item.get("reason")
            or item.get("reason") != commit_reasons.get(commit)
            or item.get("category") not in {"historical-native-carryover", "historical-integration"}
            or not item.get("semanticReview")):
            declaration_errors.append(f"invalid frozen source provenance: {path}")
    for path, item in phase.items():
        safe_path = Path(path)
        if safe_path.is_absolute() or ".." in safe_path.parts or not item.get("reason") or not item.get("source"):
            declaration_errors.append(f"invalid exact phase declaration: {path}")
        elif not (root / item["source"]).is_file():
            declaration_errors.append(f"missing phase provenance reference: {path}")
        if "sha256" in item and not re.fullmatch(r"[0-9a-f]{64}", str(item["sha256"])):
            declaration_errors.append(f"invalid reviewed phase hash: {path}")
        hash_mode = item.get("hashMode")
        if (hash_mode is not None and (path != PHASE_MANIFEST_PATH or hash_mode != PHASE_MANIFEST_HASH_MODE)
            or path == PHASE_MANIFEST_PATH and "sha256" in item and hash_mode != PHASE_MANIFEST_HASH_MODE):
            declaration_errors.append(f"invalid exact phase hash mode: {path}")
    phase_commits = {
        commit for item in phase.values()
        for commit in item.get("sourceCommits", [item["sourceCommit"]] if item.get("sourceCommit") else [])
    }
    for commit in sorted(phase_commits):
        result = subprocess.run(
            ["git", "cat-file", "-e", f"{commit}^{{commit}}"], cwd=root,
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
        )
        if result.returncode != 0:
            declaration_errors.append(f"missing phase source commit: {commit}")
    expected_modified = {p for p, kind in expected_historical.items() if kind == "modified"} | (phase.keys() & upstream.keys())
    expected_added = {p for p, kind in expected_historical.items() if kind == "added"} | (phase.keys() - upstream.keys())
    expected_removed = {p for p, kind in expected_historical.items() if kind == "removed"}
    if (manifest.get("allowedAddedPrefixes") != []
        or manifest.get("allowedModifiedFiles") != sorted(expected_modified)
        or manifest.get("allowedAddedFiles") != sorted(expected_added)
        or manifest.get("allowedRemovedFiles") != sorted(expected_removed)):
        declaration_errors.append("exact additions/modifications/removals differ from source declarations, or directory exemption exists")
    missing = (set(integration) | set(phase)) - current
    extra = current - set(integration) - set(phase)
    identity_errors.extend(f"missing source: {path}" for path in sorted(missing))
    identity_errors.extend(f"undeclared addition: {path}" for path in sorted(extra))
    identity_errors.extend(f"declared removal restored: {path}" for path in sorted(expected_removed & current))
    for path in sorted(current & phase.keys()):
        if "sha256" in phase[path] and phase_sha256(root, path) != phase[path]["sha256"]:
            identity_errors.append(f"reviewed phase bytes changed: {path}")
    for path in sorted(current & integration.keys() - phase.keys()):
        if sha256(root / path) != integration[path]:
            origin = "frozen integration" if path in historical else "protected upstream"
            identity_errors.append(f"{origin} bytes changed without phase declaration: {path}")

    native_upstream = {p: value for p, value in upstream.items() if p.startswith(NATIVE_COMPONENT_PREFIXES)}
    if (renderer.get("schemaVersion") != 3 or renderer.get("sourceCommit") != UPSTREAM_COMMIT
        or renderer.get("componentPrefixes") != list(NATIVE_COMPONENT_PREFIXES)
        or renderer.get("files") != native_upstream or renderer.get("fileCount") != len(native_upstream)):
        renderer_errors.append("complete native component inventory/hashes differ from source Git blobs")
    renderer_errors.extend(error for error in identity_errors if any(prefix in error for prefix in NATIVE_COMPONENT_PREFIXES))
    return {
        "baselineErrors": baseline_errors,
        "declarationErrors": declaration_errors,
        "identityErrors": identity_errors,
        "rendererErrors": renderer_errors,
        "upstreamFileCount": len(upstream),
        "upstreamExactFileCount": len(upstream.keys() & integration.keys() - historical.keys() - phase.keys()),
        "frozenIntegrationFileCount": len(integration),
        "historicalModifiedFileCount": sum(kind == "modified" for kind in expected_historical.values()),
        "historicalAddedFileCount": sum(kind == "added" for kind in expected_historical.values()),
        "historicalRemovedFileCount": len(expected_removed),
        "historicalNativeCarryovers": sorted(p for p, item in historical.items() if item.get("category") == "historical-native-carryover"),
        "semanticReviewRequired": sorted(p for p, item in historical.items() if item.get("semanticReview") == "provenance-only"),
        "phaseSeamCount": len(phase),
        "unfrozenPhaseSeams": sorted(p for p, item in phase.items() if "sha256" not in item),
        "protectedNativeFileCount": len(native_upstream),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", type=Path, required=True)
    parser.add_argument("--pi-source", type=Path, required=True)
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    ws = args.workspace.resolve()
    pi = args.pi_source.resolve()
    adapter = ws / "crates/codegen/pi-grok-adapter"
    pager = ws / "crates/codegen/xai-grok-pager"
    pager_bin = ws / "crates/codegen/xai-grok-pager-bin"
    native_bin = pager_bin / "src/bin/grok-pi.rs"
    docs = adapter / "docs"

    checks: dict[str, dict[str, Any]] = {}

    def check(name: str, passed: bool, detail: str, evidence: Any | None = None) -> None:
        item: dict[str, Any] = {"passed": bool(passed), "detail": detail}
        if evidence is not None:
            item["evidence"] = evidence
        checks[name] = item

    root_cargo = read(ws / "Cargo.toml")
    adapter_cargo = read(adapter / "Cargo.toml")
    bin_cargo = read(pager_bin / "Cargo.toml")
    adapter_sources = "\n".join(
        read(path) for path in sorted((adapter / "src").rglob("*.rs"))
    )
    bin_paths = [native_bin, *sorted((native_bin.parent / "grok_pi").rglob("*.rs"))]
    # This exact, cfg(test)-gated module renders native components to a Buffer.
    # Source identity still covers it; it is not production composition code.
    model_tests = native_bin.parent / "grok_pi/model_manager_tests.rs"
    test_gate = '#[cfg(test)]\n#[path = "grok_pi/model_manager_tests.rs"]\nmod model_manager_tests;'
    if native_bin.exists() and test_gate in read(native_bin):
        bin_paths.remove(model_tests)
    bin_source = "\n".join(read(path) for path in bin_paths if path.exists())
    pager_app_source = read(pager / "src/app/mod.rs")

    check(
        "adapter_is_workspace_library",
        '"crates/codegen/pi-grok-adapter"' in root_cargo
        and '"crates/codegen/pi-grok-tui"' not in root_cargo
        and "[lib]" in adapter_cargo
        and "[[bin]]" not in adapter_cargo
        and not (adapter / "src/main.rs").exists(),
        "pi-grok-adapter is a library-only workspace member; the former standalone TUI target is absent",
    )

    banned_deps = ["ratatui", "crossterm", "xai-ratatui", "tui-textarea"]
    dependency_hits = [name for name in banned_deps if name in adapter_cargo.lower()]
    check(
        "adapter_has_no_terminal_dependencies",
        not dependency_hits,
        "no terminal/rendering dependencies in the Pi adapter"
        if not dependency_hits
        else f"found terminal dependencies: {dependency_hits}",
    )

    banned_adapter_patterns = {
        "ratatui": r"\bratatui\b",
        "crossterm": r"\bcrossterm\b",
        "terminal_type": r"\bTerminal\s*<",
        "frame_type": r"\bFrame\b",
        "widget_impl": r"\bimpl\s+Widget\b",
        "draw_call": r"\.draw\s*\(",
        "render_method": r"\bfn\s+render\s*\(",
        "event_read": r"event::read\s*\(",
        "raw_mode": r"enable_raw_mode\s*\(",
        "alternate_screen": r"EnterAlternateScreen",
    }
    adapter_hits = [
        name
        for name, pattern in banned_adapter_patterns.items()
        if re.search(pattern, adapter_sources)
    ]
    check(
        "adapter_has_no_renderer_input_or_terminal_loop",
        not adapter_hits,
        "adapter only translates Pi JSONL RPC to ACP"
        if not adapter_hits
        else f"renderer/input patterns found: {adapter_hits}",
    )

    check(
        "composition_binary_is_native_grok_package",
        native_bin.exists()
        and 'name = "grok-pi"' in bin_cargo
        and 'path = "src/bin/grok-pi.rs"' in bin_cargo
        and 'pi-grok-adapter = { path = "../pi-grok-adapter" }' in bin_cargo,
        "grok-pi is a second composition root in xai-grok-pager-bin",
        str(native_bin.relative_to(ws)) if native_bin.exists() else None,
    )

    banned_bin_patterns = {
        "direct_ratatui": r"\bratatui\b",
        "direct_crossterm": r"\bcrossterm\b",
        "custom_terminal": r"\bTerminal\s*(?:::|<)",
        "custom_draw": r"\.draw\s*\(",
        "custom_event_read": r"event::read\s*\(",
        "custom_raw_mode": r"enable_raw_mode\s*\(",
    }
    bin_hits = [
        name for name, pattern in banned_bin_patterns.items() if re.search(pattern, bin_source)
    ]
    check(
        "composition_binary_does_not_render",
        not bin_hits,
        "grok-pi performs process/ACP composition only"
        if not bin_hits
        else f"custom terminal code found in grok-pi: {bin_hits}",
    )

    required_native_calls = [
        "xai_grok_pager_minimal::install()",
        "AcpConnection::external(",
        "run_external_deferred(start, ready)",
        "ExternalRunStartConfig {",
        "PagerArgs::parse_from",
    ]
    missing_calls = [token for token in required_native_calls if token not in bin_source]
    run_external_calls = [
        "spawn_writer_thread()",
        "init_terminal(",
        "event_loop::run(",
        "restore_terminal(",
    ]
    missing_run_calls = [token for token in run_external_calls if token not in pager_app_source]
    check(
        "binary_enters_production_grok_pager",
        not missing_calls and not missing_run_calls,
        "grok-pi enters Grok's production terminal initialization, writer, event loop and restore path"
        if not missing_calls and not missing_run_calls
        else f"missing composition={missing_calls}; missing pager lifecycle={missing_run_calls}",
    )

    required_components = [
        pager / "src/views/prompt_widget/mod.rs",
        pager / "src/views/completion_dropdown.rs",
        pager / "src/views/question_view.rs",
        pager / "src/views/agent_status.rs",
        pager / "src/scrollback/render.rs",
        pager / "src/slash/acp_command.rs",
        ws / "crates/codegen/xai-grok-pager-minimal/src/lib.rs",
        ws / "crates/codegen/xai-grok-markdown/src/render.rs",
    ]
    missing_components = [
        str(path.relative_to(ws)) for path in required_components if not path.exists()
    ]
    check(
        "native_grok_tui_components_present",
        not missing_components,
        "native prompt, slash, question, status, scrollback, minimal and Markdown components are present"
        if not missing_components
        else f"missing native components: {missing_components}",
    )

    baseline_manifest = json.loads(read(docs / "grok_uploaded_baseline_sha256.json"))
    renderer_manifest = json.loads(read(docs / "native_renderer_sha256.json"))
    identity = verify_source_identity(ws, baseline_manifest, renderer_manifest)
    check(
        "upstream_source_manifest_matches_git_blobs",
        not identity["baselineErrors"],
        f"complete {identity['upstreamFileCount']}-file original upstream Git inventory and source commit/tree are exact",
        identity["baselineErrors"],
    )
    check(
        "frozen_integration_and_phase_source_identity_are_exact",
        not identity["identityErrors"],
        f"{identity['upstreamExactFileCount']} protected files retain original upstream bytes; "
        f"{identity['historicalModifiedFileCount']} committed modifications, {identity['historicalAddedFileCount']} additions "
        f"and {identity['historicalRemovedFileCount']} removals are frozen to {INTEGRATION_COMMIT[:8]}; "
        f"{identity['phaseSeamCount']} current-phase files are explicitly declared, not hashed as a new source baseline",
        identity["identityErrors"],
    )
    check(
        "native_renderer_inventory_and_layered_identity_are_exact",
        not identity["rendererErrors"],
        f"all {identity['protectedNativeFileCount']} original Pager/Render/Minimal/Diff/Markdown files are covered; "
        "declared historical carryovers and current-phase seams are separate from upstream identity",
        identity["rendererErrors"],
    )
    check(
        "declared_source_surface_is_exact_and_has_provenance",
        not identity["declarationErrors"],
        "every historical deviation and phase seam has exact file-level source/reason metadata; no directory exemptions; "
        "frozen native history proves provenance, with semantic review reported separately",
        identity["declarationErrors"],
    )

    all_rs = "\n".join(
        read(path)
        for path in ws.rglob("*.rs")
        if "target" not in path.parts and ".git" not in path.parts
    )
    forbidden_messages = [
        "acknowledged by fallback renderer",
        "fallback renderer",
        "Unsupported Extension UI method",
    ]
    found_messages = [message for message in forbidden_messages if message in all_rs]
    old_tui_paths = [
        str(path.relative_to(ws))
        for path in ws.rglob("pi-grok-tui")
        if path.is_dir()
    ]
    check(
        "no_fallback_or_old_custom_tui",
        not found_messages and not old_tui_paths,
        "no fallback acknowledgement, old pi-grok-tui crate, or custom renderer remains"
        if not found_messages and not old_tui_paths
        else f"messages={found_messages}; paths={old_tui_paths}",
    )

    builtins_match = re.search(
        r"const PI_GROK_NATIVE_COMMANDS:\s*&\[&str\]\s*=\s*&\[(.*?)\];",
        bin_source,
        re.S,
    )
    builtins = re.findall(r'"([^"]+)"', builtins_match.group(1)) if builtins_match else []
    expected_builtins = [
        "exit",
        "help",
        "hotkeys",
        "tutorial",
        "new",
        "compact",
        "model",
        "effort",
        "rename",
        "resume",
        "session-info",
        "tree",
        "tree-map",
        "fork",
        "clone",
        "reload",
        "notify",
        "dashboard",
        "recap",
        "btw",
        "copy",
        "find",
        "jump",
        "review-session",
        "review-message",
        "transcript",
        "export",
        "expand",
        "queue",
        "plan",
        "plan-mode",
        "view-plan",
        "multiline",
        "compact-mode",
        "eval-display",
        "vim-mode",
        "theme",
        "timestamps",
        "timeline",
        "toggle-mouse-reporting",
        "voice",
        "doctor",
        "debug",
        "pi-config",
        "pi-models",
        "pi-shortcut-manager",
    ]
    # Product/session-store commands must not leak into the Pi composition.
    # Pi extension/prompt/skill commands arrive dynamically over get_commands.
    forbidden_local_commands = [
        "rpc",
        "diagnostics",
        "capabilities",
        "stats",
        "history",
        "login",
        "logout",
        "usage",
        "plugins",
        "mcp",
        "memory",
        "workspace",
        "share",
    ]
    local_command_hits = [name for name in forbidden_local_commands if name in builtins]
    check(
        "slash_surface_uses_grok_native_commands_and_pi_catalog",
        builtins == expected_builtins and not local_command_hits,
        f"retained {len(builtins)} existing Grok UI or ACP-backed commands; no adapter-specific slash UI"
        if builtins == expected_builtins and not local_command_hits
        else f"builtins={builtins}; forbidden local commands={local_command_hits}",
        builtins,
    )
    check(
        "pi_commands_are_dynamic_acp_commands",
        '"type": "get_commands"' in adapter_sources
        and "AvailableCommandsUpdate" in adapter_sources
        and "command_catalog(&self.commands, workflows_enabled)" in adapter_sources,
        "Pi extension/prompt/skill commands are discovered from get_commands and merged by Grok's native ACP slash registry",
    )

    pi_identity, rpc_text, agent_session_text, pi_event_corpus = load_pi_contract(pi)
    command_section = rpc_text.split("// RPC Responses", 1)[0]
    pi_command_tokens = set(re.findall(r'type:\s*"([a-zA-Z0-9_]+)"', command_section))
    adapter_command_tokens = {
        "get_state",
        "get_available_models",
        "get_commands",
        "get_messages",
        "get_entries",
        "clear_queue",
        "prompt",
        "abort",
        "abort_bash",
        "bash",
        "new_session",
        "compact",
        "set_model",
        "set_thinking_level",
        "set_session_name",
    }
    missing_rpc_commands = sorted(adapter_command_tokens - pi_command_tokens)
    check(
        "adapter_rpc_calls_exist_in_uploaded_pi",
        not missing_rpc_commands,
        f"validated {len(adapter_command_tokens)} adapter RPC calls against Pi {pi_identity['version']} {pi_identity['kind']}"
        if not missing_rpc_commands
        else f"missing Pi RPC commands: {missing_rpc_commands}",
    )

    expected_ui = {
        "select",
        "confirm",
        "input",
        "editor",
        "notify",
        "setStatus",
        "setWidget",
        "setTitle",
        "set_editor_text",
    }
    pi_ui_methods = set(re.findall(r'method:\s*"([A-Za-z0-9_]+)"', rpc_text))
    adapter_ui_tokens = {
        "select": '"select"',
        "confirm": '"confirm"',
        "input": '"input"',
        "editor": '"editor"',
        "notify": '"notify"',
        "setStatus": '"setstatus"',
        "setWidget": '"setwidget"',
        "setTitle": '"settitle"',
        "set_editor_text": '"set_editor_text"',
    }
    missing_pi_ui = sorted(expected_ui - pi_ui_methods)
    missing_adapter_ui = sorted(
        method for method, token in adapter_ui_tokens.items() if token not in adapter_sources
    )
    native_ui_routes = [
        '"pi/ui/notify"',
        '"pi/ui/status"',
        '"pi/ui/widget"',
        '"pi/ui/title"',
        '"pi/ui/editor_text"',
        '"pi/ui/cancel_interaction"',
        '"x.ai/ask_user_question"',
    ]
    pager_ui_text = read(pager / "src/app/acp_handler/mod.rs") + read(
        pager / "src/app/acp_handler/interactions.rs"
    )
    missing_native_routes = [route for route in native_ui_routes if route not in (adapter_sources + pager_ui_text)]
    check(
        "all_pi_extension_ui_methods_use_native_grok_surfaces",
        not missing_pi_ui and not missing_adapter_ui and not missing_native_routes,
        "all 9 Pi Extension UI methods map to Grok toast/status/widget/title/prompt/question surfaces"
        if not missing_pi_ui and not missing_adapter_ui and not missing_native_routes
        else (
            f"missing Pi={missing_pi_ui}; adapter={missing_adapter_ui}; "
            f"native routes={missing_native_routes}"
        ),
    )

    event_tokens = {
        "agent_start",
        "agent_end",
        "agent_settled",
        "turn_start",
        "turn_end",
        "message_start",
        "message_update",
        "message_end",
        "tool_execution_start",
        "tool_execution_update",
        "tool_execution_end",
        "queue_update",
        "compaction_start",
        "compaction_end",
        "auto_retry_start",
        "auto_retry_end",
        "session_info_changed",
        "thinking_level_changed",
        "extension_ui_request",
        "extension_error",
    }
    missing_event_routes = sorted(
        token for token in event_tokens if f'"{token}"' not in adapter_sources
    )
    check(
        "pi_runtime_events_are_projected_to_acp",
        not missing_event_routes,
        f"validated {len(event_tokens)} Pi lifecycle, stream, tool, queue, compaction, retry and UI event routes"
        if not missing_event_routes
        else f"missing event routes: {missing_event_routes}",
    )

    # Most stream/tool lifecycle events come from AgentEvent, while Pi-specific
    # queue/compaction/retry state is declared in AgentSessionEvent. Validate the
    # Pi-specific set directly and validate the remaining event strings across
    # the uploaded Pi coding-agent source tree.
    missing_pi_event_contract = sorted(
        token for token in event_tokens if f'"{token}"' not in pi_event_corpus
    )
    check(
        "adapter_event_routes_exist_in_uploaded_pi",
        not missing_pi_event_contract and "AgentSessionEvent" in agent_session_text,
        f"all {len(event_tokens)} routed events exist in the uploaded Pi source contract"
        if not missing_pi_event_contract and "AgentSessionEvent" in agent_session_text
        else f"missing Pi event contract tokens: {missing_pi_event_contract}",
    )

    behavior_tokens = {
        "history_replay": '"type": "get_messages"',
        "model_catalog": "build_model_catalog(",
        "reasoning_stream": "AgentThoughtChunk",
        "markdown_stream": "AgentMessageChunk",
        "tool_cards": "ToolCallUpdate",
        "image_blocks": "ContentBlock::Image",
        "steer": '"streamingBehavior": "steer"',
        "follow_up": '"followUp"',
        "direct_bash": '"type": "bash"',
        "completion_barrier": '"agent_settled" => {',
    }
    missing_behavior = [name for name, token in behavior_tokens.items() if token not in adapter_sources]
    check(
        "pi_semantics_feed_grok_native_views",
        not missing_behavior,
        "history, Markdown/reasoning streams, images, tools, model/effort, steer/follow-up and Bash are mapped into ACP"
        if not missing_behavior
        else f"missing semantic mappings: {missing_behavior}",
    )

    dialog_semantics = {
        "timeout": "extension_dialog_timeout(&event)",
        "native_cancel": '"pi/ui/cancel_interaction"',
        "pi_cancel_response": '"cancelled": true',
        "confirm_response": '"confirmed": answer.eq_ignore_ascii_case("yes")',
        "freeform_annotations": '"input" | "editor" => annotated_answer(value)',
    }
    missing_dialog_semantics = [
        name for name, token in dialog_semantics.items() if token not in adapter_sources
    ]
    check(
        "extension_dialog_semantics_match_pi",
        not missing_dialog_semantics,
        "Pi timeouts cancel Grok QuestionView; confirm/value/cancel response shapes match rpc-types.ts"
        if not missing_dialog_semantics
        else f"missing dialog semantics: {missing_dialog_semantics}",
    )

    readme_text = read(adapter / "README.md") if (adapter / "README.md").exists() else ""
    capabilities_text = read(docs / "capabilities.json") if (docs / "capabilities.json").exists() else ""
    stale_doc_tokens = [
        token
        for token in ["pi-grok-tui", "Rust/Ratatui terminal frontend"]
        if token.lower() in (readme_text + "\n" + capabilities_text).lower()
    ]
    required_doc_tokens = [
        "xai-grok-pager",
        "pi-grok-adapter",
        "native",
        "get_commands",
    ]
    missing_doc_tokens = [
        token for token in required_doc_tokens if token.lower() not in (readme_text + capabilities_text).lower()
    ]
    check(
        "documentation_describes_native_grok_architecture",
        not stale_doc_tokens and not missing_doc_tokens,
        "README and capabilities identify the production Grok pager and the headless Pi adapter"
        if not stale_doc_tokens and not missing_doc_tokens
        else f"stale={stale_doc_tokens}; missing={missing_doc_tokens}",
    )

    passed = all(item["passed"] for item in checks.values())
    report = {
        "schemaVersion": 2,
        "passed": passed,
        "workspace": str(ws),
        "piSource": str(pi),
        "piContract": pi_identity,
        "sourceIdentity": identity,
        "checks": checks,
    }
    out = args.json_out or docs / "native-grok-verification.json"
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    for name, item in checks.items():
        marker = "PASS" if item["passed"] else "FAIL"
        print(f"[{marker}] {name}: {item['detail']}")
    print(f"\nResult: {'PASS' if passed else 'FAIL'}")
    print(f"Report: {out}")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
