#!/usr/bin/env python3
"""Source guard for the four retired implementations and the surviving Bash bundle."""
from pathlib import Path
import json
import re

ROOT = Path(__file__).resolve().parents[4]
retired = [
    "extensions/pi-grok-native-commands",
    "extensions/pi-grok-rust-tui-bridge",
    "extensions/pi-grok-00-profiler",
    "crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/native_commands_extension.rs",
    "crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/rust_tui_bridge_extension.rs",
    "crates/codegen/xai-grok-pager/src/slash/commands/eval_display.rs",
]
retired += ["extensions/pi-grok-bash/" + name for name in
            ["eval.ts", "eval-tasks.ts", "tool-bridge.ts", "eval-pi-mcp.ts", "eval-token-count.ts"]]
for name in retired:
    assert not (ROOT / name).exists(), name

main = (ROOT / "crates/codegen/xai-grok-pager-bin/src/bin/grok-pi.rs").read_text()
for symbol in ["native_commands_extension", "rust_tui_bridge_extension", "PI_GROK_EVAL", '"eval-display"']:
    assert symbol not in main, symbol
for builtin in ["builtin:codemode", "builtin:mcp", "builtin:tool-search"]:
    assert builtin in main, builtin
assert '"grok-pi: remote TUI"' in main

bundle = ROOT / "extensions/pi-grok-bash"
pending, seen = [bundle / "index.ts"], set()
while pending:
    path = pending.pop()
    if path in seen:
        continue
    seen.add(path)
    source = path.read_text()
    assert "PersistentEvalKernel" not in source
    assert not re.search(r'name:\s*["\']eval["\']', source)
    pending.extend(path.parent / name for name in re.findall(r'from\s+["\'](\./[^"\']+)["\']', source))
assert {p.name for p in seen} == {"index.ts", "bash-tasks.ts", "prompts.ts", "shared.ts"}
injector = (ROOT / "crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/bash_extension.rs").read_text()
for path in seen:
    assert "extensions/pi-grok-bash/" + path.name in injector

settings = (ROOT / "crates/codegen/xai-grok-pager/src/settings/defs.rs").read_text()
assert not re.search(r'key:\s*"(?:pi_eval[^"\n]*|pi_builtin_tools\.eval)"', settings)
catalog = (ROOT / "extensions/pi-grok-web-config/web/ui-config.json").read_text()
assert "pi_eval" not in catalog and "pi_builtin_tools.eval" not in catalog
json.loads(catalog)
assert (ROOT / "crates/codegen/xai-grok-pager/src/scrollback/blocks/tool/eval.rs").is_file()
print("PASS: four retired implementations absent; Bash import/injector closure, official Pi tools, Remote TUI and historical Eval renderer preserved")
