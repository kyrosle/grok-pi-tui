# Subagents & Dashboard

grok-pi includes a bundled Subagents extension using child Pi AgentSession
instances. F2 `Pi subagents` is on by default and requires restart when changed.
Subagents is an extension capability, not a built-in Pi agent-loop feature.

- A subagent appears as a native SubagentBlock, Tasks Pane row and child
  AgentView, with cancellation routed back to the owning Pi child session.
- The child has its own model, tools and context while the parent keeps the main
  conversation.
- `/dashboard` or `Ctrl+\\` opens the native multi-session overview for active
  agents plus resumable Pi session rows discovered on demand.
- Dashboard dispatch can choose model, effort and Plan mode before creating a
  new session.

The bridge is available when grok-pi bridge extensions load, but actual
delegation still depends on the model choosing the subagent tool and on valid
model configuration. Known duplicate subagent packages are blocked by default
to avoid two competing implementations.
