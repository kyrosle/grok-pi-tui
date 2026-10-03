# Terminal, Prompt & Input

grok-pi enters Grok Pager's production terminal lifecycle instead of wrapping
Pi in a second shell.

- Choose fullscreen, inline or minimal mode at startup. The tutorial itself is
  fullscreen-only because minimal mode has no modal host.
- Fresh starts use the native Welcome screen and prewarm a Pi session behind it.
- The PromptWidget supports multiline editing, optional Vim mode and pasted
  text or images; responses use native Markdown, code blocks and scrollback.
- `/hotkeys` shows the active profile's keys. Appearance controls include
  `/theme`, `/timestamps`, `/timeline` and mouse reporting.
- F2 or `/settings` opens the native Pager settings surface for terminal UI,
  integrated Pi controls and bundled extension options. The command palette
  and Web host-settings catalog use the same product filter.

Use `Tab` to move between prompt and scrollback, then search, select, copy or
export without leaving the native Pager.
