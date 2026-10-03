"""Read Pi contracts from a checkout or the installed package behind PI_BIN/pi."""
from __future__ import annotations

import json
import os
import re
import shutil
from pathlib import Path


RUNTIME_RPC_COMMANDS = frozenset({"set_auto_retry", "set_auto_compaction", "abort_retry"})


def runtime_rpc_commands(source: str) -> tuple[set[str], list[str]]:
    """Audit the explicit production generator that request(command) hides.

    This is a narrow contract for runtime.rs, not a general Rust parser or a
    claim that arbitrary computed request types have been discovered.
    """
    production = source.split("#[cfg(test)]", 1)[0]
    function = re.search(r"fn runtime_command\b[\s\S]+?\n}\s*(?=impl PiAgent)", production)
    if not function:
        return set(), ["production runtime_command generator is missing"]
    generator = function.group()
    direct = re.findall(r'"type"\s*:\s*"([a-zA-Z0-9_]+)"', generator)
    conditional = re.findall(
        r'"type"\s*:\s*if\s+operation\s*==\s*"retry"\s*\{\s*"([a-zA-Z0-9_]+)"\s*}\s*else\s*\{\s*"([a-zA-Z0-9_]+)"\s*}',
        generator,
    )
    commands = set(direct) | {command for pair in conditional for command in pair}
    errors = []
    if commands != RUNTIME_RPC_COMMANDS:
        errors.append(f"runtime generator command declarations differ: {sorted(commands)}")
    if len(direct) != 1 or conditional != [("set_auto_retry", "set_auto_compaction")]:
        errors.append("explicit retry/compaction/cancel generation mapping changed")
    if len(re.findall(r'"type"\s*:', generator)) != len(direct) + len(conditional):
        errors.append("unrecognized computed runtime RPC command type")
    if not re.search(r"let\s+command\s*=\s*runtime_command\(params\)\?\s*;", production) or not re.search(r"\.request\(command\)", production):
        errors.append("runtime_control no longer dispatches the audited generator")
    return commands, errors


def load_pi_contract(preferred: Path | None) -> tuple[dict[str, str], str, str, str]:
    if preferred is not None:
        package = preferred / "packages/coding-agent"
        source = package / "src"
        rpc = source / "modes/rpc/rpc-types.ts"
        if rpc.is_file():
            version = json.loads((package / "package.json").read_text())["version"]
            identity = {"kind": "source_checkout", "root": str(package.resolve()), "version": version, "rpcTypes": str(rpc.resolve())}
            corpus = "\n".join(path.read_text() for path in source.rglob("*.ts"))
            return identity, rpc.read_text(), (source / "core/agent-session.ts").read_text(), corpus

    executable = shutil.which(os.environ.get("PI_BIN", "pi"))
    candidates = [preferred] if preferred is not None else []
    if executable:
        candidates.extend(Path(executable).resolve().parents)
    for package in candidates:
        manifest = package / "package.json"
        if not manifest.is_file():
            continue
        metadata = json.loads(manifest.read_text())
        if metadata.get("name") != "@earendil-works/pi-coding-agent":
            continue
        dist = package / "dist"
        rpc = dist / "modes/rpc/rpc-types.d.ts"
        if not rpc.is_file():
            continue
        identity = {"kind": "installed_package", "root": str(package.resolve()), "version": metadata["version"], "rpcTypes": str(rpc.resolve())}
        # Includes the public declaration files and the packaged event emitters;
        # no imports, processes, extensions or credentials are loaded.
        corpus = "\n".join(path.read_text() for path in dist.rglob("*") if path.suffix in {".ts", ".js"})
        return identity, rpc.read_text(), (dist / "core/agent-session.d.ts").read_text(), corpus
    raise FileNotFoundError("Pi RPC contracts unavailable; provide a Pi checkout/package with --pi-source or set PI_BIN to its executable")
