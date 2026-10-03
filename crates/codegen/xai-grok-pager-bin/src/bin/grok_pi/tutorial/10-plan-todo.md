# Plan Mode & Todo

Use `/plan-mode` to toggle grok-pi Plan mode for the current Pi session. The keyboard shortcut is `Ctrl+Shift+T` on macOS/Linux and `Ctrl+Alt+T` on Windows.

Plan is a grok-pi integration, not a built-in Pi mode. Pi 1.0 has no public
Plan/read-only mode API; the adapter still owns its state, reminders and
approval continuation.

- Pi can read and search the real repository before proposing an approach.
- The bundled plan extension blocks `bash` and permits `edit`/`write` only for
  the session-private plan file. This gate does not cover arbitrary mutation
  tools supplied by other extensions or MCP servers.
- The mode state is stored beside the Pi session and survives resume.
- The extension's `exit_plan_mode` tool opens the native approval view so you can accept
  the plan or request changes before implementation.

Todo is a separate bundled Pi extension. grok-pi injects its `todo`
tool by default; F2 `[ui].pi_todo` controls it and requires a restart. Its
`details.tasks` snapshots map to the native TodoPane, badge and ACP Plan instead
of rendering a duplicate tool card.

When built-in Todo is enabled, grok-pi's resource policy blocks the compatible
`npm:@juicesharp/rpiv-todo` provider so only one `todo` tool is registered. Turn
`pi_todo` off and restart before using that community provider instead. Plan
mode and Todo remain complementary: Plan gates mutations and approval, while
Todo tracks the live work list.
