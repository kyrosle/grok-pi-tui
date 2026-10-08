# pi-grok-bash

Enhanced Bash execution through the official Pi extension API. Pi owns the agent and tool pipeline; Pager owns native cards and task UI.

The injected bundle contains `index.ts`, `bash-tasks.ts`, `prompts.ts` and `shared.ts`. No persistent Node/Python Eval kernel or Eval MCP endpoint is created.

- `bash`: task_name, foreground/background execution, output limits, timeout and automatic background promotion.
- `get_task_output`: inspect task IDs, optionally wait; stale IDs do not discard valid results.
- `wait_tasks`: wait_any/wait_all with a configured wait cap; timeout_ms=0 is a snapshot.
- `kill_task`: terminate a task's process group. Session shutdown also clears live tasks.

`[ui].pi_bash` defaults on; disabling it restores Pi's standard Bash. Explicit CLI tool restrictions remain authoritative. PI_GROK_BASH_MAX_WAIT_MINS (default 4.5) caps blocking waits and controls automatic promotion; zero/negative disables the cap/promotion.

Code orchestration uses Pi's official Codemode; Python/scripts can run through Bash. Historical Eval messages still render, but old Eval configuration and environment flags cannot enable an execution environment.

Run the focused regression with the installed Pi package on NODE_PATH:

```bash
NODE_PATH="$(npm root -g)" bun extensions/pi-grok-bash/test-bash.mjs
python3 crates/codegen/pi-grok-adapter/tests/pi_native_rpc_smoke.py
```

The latter uses an isolated local synthetic provider and MCP fixture, with no real inference or credentials.
