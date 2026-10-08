# What Is grok-pi?

grok-pi is a **Grok Build-style native TUI for Pi**.
It combines the Pi agent core with Grok Pager through three layers:

- **Pi is the agent core.** Pi owns providers, models, the agent loop, tools,
  extensions, compaction and local session files.
- **Grok Pager is the terminal UI.** It owns the prompt, scrollback, Markdown,
  tool cards, diffs, pickers, modals and terminal lifecycle.
- **The adapter stays headless.** It translates Pi RPC events into Pager-native
  surfaces and never draws a second terminal interface.

Todo, Plan mode, background Bash and Subagents are bundled grok-pi
extensions or integrations. Each has its own defaults and capability limits;
they are not Pi built-ins. Optional Rhai workflows use Pi workers but retain
their own orchestration runtime.

Queue interception and Plan/Goal state still have adapter ownership. The
current product-surface cut does not migrate those mechanisms.

Your models and sessions belong to Pi, not a Grok cloud account.
