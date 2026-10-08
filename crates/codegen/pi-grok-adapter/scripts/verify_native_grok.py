#!/usr/bin/env python3
"""Check native UI architecture and installed-Pi source contracts.

Source guards do not prove runtime behavior, endpoint absence or Git blob identity.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any

from pi_contract_sources import load_pi_contract, runtime_rpc_commands


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def source_files(root: Path) -> set[str]:
    # Git defines source candidates; ignored dependencies and build artifacts
    # are not source candidates for the architecture guard.
    candidates = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=root,
    ).decode("utf-8").split("\0")
    return {name for name in candidates if name and (root / name).is_file()}


def adapter_dependency_hits(cargo_text: str, workspace_cargo_text: str = "") -> list[str]:
    """Check declared dependencies, including aliases, targets and test dependencies."""
    hits: set[str] = set()
    workspace_dependencies = tomllib.loads(workspace_cargo_text).get("workspace", {}).get("dependencies", {}) if workspace_cargo_text else {}
    def visit(value: Any) -> None:
        if not isinstance(value, dict):
            return
        for key, entries in value.items():
            if key in {"dependencies", "build-dependencies", "dev-dependencies"}:
                for name, definition in entries.items():
                    if isinstance(definition, dict) and definition.get("workspace") is True:
                        definition = workspace_dependencies.get(name, definition)
                    package = definition.get("package", name) if isinstance(definition, dict) else name
                    if package in {"ratatui", "crossterm", "tui-textarea"} or package.startswith(("xai-ratatui", "xai-grok-pager")):
                        hits.add(package)
            else:
                visit(entries)
    visit(tomllib.loads(cargo_text))
    return sorted(hits)


def native_command_errors(source: str, expected: list[str], forbidden: list[str]) -> tuple[list[str], list[str]]:
    match = re.search(r"const PI_GROK_NATIVE_COMMANDS:\s*&\[&str\]\s*=\s*&\[(.*?)\];", source, re.S)
    commands = re.findall(r'"([^"]+)"', match.group(1)) if match else []
    errors = []
    if commands != expected:
        errors.append("native command catalog differs from the reviewed UI whitelist")
    if set(commands) & set(forbidden):
        errors.append("stock product commands entered the local native catalog")
    return commands, errors


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
    # It is a test-only rendering fixture, not production composition code.
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

    dependency_hits = adapter_dependency_hits(adapter_cargo, root_cargo)
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
        "grok-pi composition uses the native Pager library",
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

    tracked_sources = source_files(ws)
    all_rs = "\n".join(read(ws / path) for path in sorted(tracked_sources) if path.endswith(".rs"))
    forbidden_messages = [
        "acknowledged by fallback renderer",
        "fallback renderer",
        "Unsupported Extension UI method",
    ]
    found_messages = [message for message in forbidden_messages if message in all_rs]
    old_tui_paths = sorted(path for path in tracked_sources if "pi-grok-tui" in Path(path).parts)
    check(
        "no_fallback_or_old_custom_tui",
        not found_messages and not old_tui_paths,
        "no fallback acknowledgement, old pi-grok-tui crate, or custom renderer remains"
        if not found_messages and not old_tui_paths
        else f"messages={found_messages}; paths={old_tui_paths}",
    )

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
        "vim-mode",
        "theme",
        "timestamps",
        "timeline",
        "toggle-mouse-reporting",
        "doctor",
        "debug",
        "pi-config",
        "pi-runtime",
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
        "voice",
    ]
    builtins, command_errors = native_command_errors(bin_source, expected_builtins, forbidden_local_commands)
    local_command_hits = [name for name in forbidden_local_commands if name in builtins]
    check(
        "slash_surface_uses_grok_native_commands_and_pi_catalog",
        not command_errors,
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
    runtime_commands, runtime_command_errors = runtime_rpc_commands(read(adapter / "src/pi_adapter/runtime.rs"))
    adapter_command_tokens.update(runtime_commands)
    missing_rpc_commands = sorted(adapter_command_tokens - pi_command_tokens)
    check(
        "adapter_rpc_calls_exist_in_uploaded_pi",
        not missing_rpc_commands and not runtime_command_errors,
        f"validated {len(adapter_command_tokens)} declared adapter RPC contracts including the explicit runtime generator against Pi {pi_identity['version']} {pi_identity['kind']}"
        if not missing_rpc_commands and not runtime_command_errors
        else f"missing Pi RPC commands: {missing_rpc_commands}; runtime generation errors: {runtime_command_errors}",
        {"commands": sorted(adapter_command_tokens), "runtimeGeneratorCommands": sorted(runtime_commands)},
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
        "schemaVersion": 3,
        "passed": passed,
        "workspace": str(ws),
        "piSource": str(pi),
        "piContract": pi_identity,
        "proofLayer": "source-architecture-and-Pi-contract",
        "limits": ["No upstream blob identity requirement", "No Cargo/runtime or endpoint-absence proof"],
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
