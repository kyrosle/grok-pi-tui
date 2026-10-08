Current development also includes opt-in [Durable integration](issues/架构/20261007-pi-durable-integration-SPEC.md): official SDK 1.0.4 owns SQLite/tasks/inboxes, a Node headless host exposes an independent protocol, and a library-only Rust adapter projects it into the same Pager. F2 persists a default-off preference for next start. Classic Pi RPC remains the factory default; ordinary plugins and several advanced capabilities are not adapted in Durable mode.

Current development source: the [four-cut SPEC](issues/架构/20261007-pi-native-four-cuts-SPEC.md) retires custom Eval runtime/MCP, experimental native Pi selectors, the second custom UI host and startup profiler. Enhanced Bash, the existing Remote TUI host and legacy transcript rendering remain. Published v0.1.10 predates this change.

# grok-pi native TUI architecture

## Governing master plan

[Master SPEC](issues/架构/20261003-pi-native-tui-SPEC.md) and [PLAN](issues/架构/20261003-pi-native-tui-PLAN.md) define one Pi-native Rust TUI and removal of stock Grok business/profile. Grok Build is a reference repository, not a merge upstream. Architecture/contract/entry/dependency/endpoint guards replace historical blob identity. T0/T1 begin this transition; compiled dependencies and adapter ownership debt remain explicit until T2–T7.

## 2026-10-08 reference adaptation

[Implementation record](issues/架构/20261008-reference-adaptation-PLAN.md) preserves exact reference SHAs and proof layers. Pi owns provider/auth/runtime and Durable SDK 1.1.0; the adapter projects semantic status and measured durations without a terminal dependency. Pager owns fullscreen Codemode, trace hints, price-tier descriptions and OSC 7501 output. No Grok business, custom Eval or second TUI host is reintroduced.

## Historical product-surface checkpoint

The current [SPEC](issues/架构/20261003-pi-product-surface-SPEC.md) and [PLAN](issues/架构/20261003-pi-product-surface-PLAN.md) define a native Grok Build-style TUI for Pi. Pi supplies models, providers, authentication, tools, sessions, retry and compaction; Pager presents controls and results. The external profile removes Grok voice/STT/TTS, account/billing, training/retention and stock agent/plugin/MCP controls across F2, palette and the Web host-settings catalog. Existing user configuration and credentials are retained; stock Grok keeps its own profile.

Bundled Todo, Subagents, Plan, Goal and enhanced Bash are extension/integration capabilities with explicit defaults. Adapter queue interception and Plan/Goal state, plus the optional Rhai Workflow runtime, remain ownership debts in the PLAN. Their migration is outside this product-surface cut.

The reference is the installed `@earendil-works/pi-coding-agent` 1.0.0 package: `docs/slash-commands.md` groups model/settings, session/context, export and runtime/resource commands; `docs/settings.md` separates model/thinking, interaction, tools, sessions, display and resources. `dist/modes/interactive/interactive-mode.js` routes `/settings` to `SettingsSelectorComponent`, uses `AgentSession` for compaction/queue controls and `SettingsManager` for persistent preferences. grok-pi follows those responsibilities while retaining native Pager controls and names such as `/effort` and `/rename`; it does not transplant Pi's interactive renderer or claim complete settings/command parity.

Pi's `docs/tui.md` recommends standard extension dialogs before custom components. `docs/rpc-extension-ui.md` defines the narrower stock RPC surface. Native QuestionView, status and toast mappings use that boundary; experimental Remote TUI remains a scoped compatibility host with per-component limits.

Current validation is recorded in [VERIFICATION](VERIFICATION.md). The following completed checkpoints are historical evidence, not proof of the current product-surface revision.

## Historical 2026-10-03 deep adaptation automatic evidence

Latest user clarification assigns agent/provider business and authentication to Pi, with Grok owning native presentation. The standalone auth bridge delegates generic login/logout and maps prompts, notifications, responses and cancellation; its Radius MCP writer, special selection and post-login configuration confirmation were removed. Generic Radius provider login remains, while MCP configuration uses Pi's supported entry points. The earlier tested binary is a pre-cut checkpoint; this TypeScript-only cut is checked as an extension without Cargo/build. Real provider/OAuth/image/terminal experience stays supplementary, not a new TUI-development closing prerequisite.

That increment follows [deep adaptation SPEC](issues/架构/20261003-pi-deep-adaptation-SPEC.md), [PLAN](issues/架构/20261003-pi-deep-adaptation-PLAN.md) and [exact source review](issues/架构/20261003-pi-deep-adaptation-SOURCE.md). Its measured production dependency graph has **806** packages with all seven prohibited stock runtimes absent. Corrected-source build, four native PTYs and combined verification passed at that checkpoint; the preceding 805-package result and completed verify below certify the Pi-first checkpoint at `84174917` only.

Fresh native package PTY exposed fixed startup resource admission: installing a declaration did not load its new extension during the old reload path. The correction recomputes policy admission from official SDK-resolved resources, retains user CLI restrictions/trust, and uses official child shutdown/restart with public session/leaf/model/thinking restoration when startup inputs change. Unsafe in-memory restoration is deferred rather than rewriting Pi session JSONL. Actual restore/build/PTY proof now passes the named paths; public-API restoration boundaries remain saved/deferred and supplementary Pi/provider human experience stays pending. Hidden loader errors and simultaneous external Web writes remain explicit boundaries in [VERIFICATION](VERIFICATION.md).

## Historical 2026-10-03 Pi-first checkpoint at 84174917

The accepted [SPEC](issues/架构/20261002-pi-first-tui-SPEC.md) keeps Grok native TUI and Pi 1.0 as the only agent core. Question/Workflow contracts, host-feature metadata, complete configuration/session DTOs and pure permission helpers have canonical neutral owners with stock compatibility re-exports. The production Pi profile uses `--no-default-features --features jemalloc,sandbox-enforce`; its `normal,build` graph has 805 unique packages and none of the seven prohibited stock execution runtimes. Both the isolated Pi and default stock profiles compile.

Codemode image/nested output and Subagent child live/replay come from actual installed Pi and official SDK sessions, through the headless ACP adapter, into native cards/gallery/child views. Auth initial/nested dialogs use native QuestionView and scope teardown. These synthetic-provider transport/component checks do not establish real-model, OAuth or every-terminal acceptance. [VERIFICATION](VERIFICATION.md) separates current architecture/transport/PTY evidence from historical frozen-source and combined verification records.

The exact source policy keeps all 3797 ancestor Git blobs and the complete 830-file native-component inventory, freezes historical integration changes to `222d614d`, and declares current-phase files one by one. Historical provenance records are not a full semantic-sync claim. The sole self-referential inventory metadata hash is canonical JSON with only its own SHA field omitted; native source and every other file use full-byte SHA-256. No native directory is exempted. [Selective TUI backports](upstream/TUI_BACKPORTS.md) record complete functional groups, source commits/revisions and reasons while `SOURCE_REV=c4ea71cfdbcdb21e32e41bc25a0043d7d4836714` remains associated with Git ancestor `37949780c144e37df692e3d669051a21fec24f20`.

## Acceptance Conclusion

The current entry point is not a self-drawn Ratatui shell. `grok-pi` lives inside Grok Build's production binary package `xai-grok-pager-bin` and calls `xai_grok_pager::app::run_external`. The adapter only produces ACP requests/notifications; every terminal surface is created by the Grok Pager.

## Reused Grok Production Components

| Capability | Native implementation | Pi integration path |
|---|---|---|
| Terminal lifecycle | `xai-grok-pager/src/app/mod.rs` | `run_external` enters the same init/writer/event-loop/restore path |
| Fullscreen / minimal / inline | `xai-grok-pager` + `xai-grok-pager-minimal` | same screen-mode resolver and IoC hook; selective native semantic-row/reprint/resize group |
| Input editor | `views/prompt_widget` | Pi prompt and `set_editor_text` enter the PromptWidget |
| Slash completion | `slash` + `views/completion_dropdown` | Pi `get_commands` converted to ACP `AvailableCommand` then merged |
| Markdown / code | `xai-grok-markdown` | `AgentMessageChunk`/`AgentThoughtChunk` enter the native scrollback pipeline |
| Tools and diffs | `acp/tracker`, native `RenderBlock`/`EditToolCallBlock` | Pi tool lifecycle converted to ACP `ToolCall`/`ToolCallUpdate`; an external-only F2 opt-in (default off) delegates edit rows to a sibling side-by-side layout renderer when wide enough, while disabled/narrow layouts stay on the native unified renderer |
| Q&A overlay | `views/question_view` | `select`/`confirm`/`input`/`editor` converted to `x.ai/ask_user_question` |
| Status and notifications | native toast / sticky surface | `notify`/`setStatus`/`setWidget` converted to narrow ACP notifications |
| Scroll and transcript | native scrollback / transcript | both historical and live events are ACP `SessionUpdate` |
| Model selection | native model selector | Pi models / thinking levels converted to `SessionModelState` |

## Current Architecture Evidence

Current verification checks ownership and production behavior:

- `pi-grok-adapter` remains headless and has no native terminal rendering or keyboard ownership;
- `grok-pi.rs` composes the backend and native Pager without a drawing or input loop;
- dependency, endpoint, command-entry and protocol guards identify forbidden runtime dependencies and remaining transition work;
- Grok Build improvements are reviewed at pinned SHAs and deliberately ported with their own validation. Historical byte-identity inventories are [archived](grok-build/archive/README.md) and do not freeze current native source.

## Why A Few Grok Pager Files Still Change

The ACP standard does not cover all of Pi's UI/command semantics, so narrow seams are needed:

1. `UiProfile::External` plus compile features: disables Grok.com product capabilities and disconnects stock execution runtimes while keeping native UI components.
2. `AcpConnection::external`: lets the existing Pager accept an external ACP channel.
3. `run_external`: reuses the production terminal/event-loop, skipping Grok Agent startup and login.
4. Pi UI notification handlers: map fire-and-forget status to native toast/banner/title/editor.
5. QuestionView hints: reuse the native freeform editor and support Pi timeout revocation.
6. slash profile: only selects existing Grok commands that are meaningful for Pi and fully work under the external ACP composition; Pi dynamic commands remain managed by the native registry.
7. `/compact <instructions>`: passes the optional text from the native Grok command to Pi `customInstructions`.
8. screen-mode boundary: Grok's native minimal/fullscreen renderer is retained, but the original slash re-exec would rebuild Grok's own `--resume` argv and cannot carry `grok-pi`'s Pi startup arguments, so only the startup option is exposed, not the broken `/minimal`/`/fullscreen` re-exec.
9. product settings and voice boundary: the external settings filter retains terminal UI and integrated Pi/extension controls; it removes stock product settings and voice entry points. External ACP connections do not create a Grok AuthManager. Stock retains its own voice and settings behavior.
10. tutorial copy profile: stock Grok retains its default onboarding content; grok-pi installs 18 static product-specific capability topics while reusing the native `TutorialState`, modal, picker, Markdown renderer and input routing.

Selective native terminal/reader/fence, clipboard/repaint, Minimal/Inline semantic rows, Swift syntax and Mermaid SVG/PNG groups update the existing native implementations. They preserve Pi ownership and use the same PromptWidget, QuestionView, scrollback, cards and image consumers. These seams do not create a second TUI or scrollback pipeline. The optional sibling EditTool layout renderer reuses native diff data, highlighting helpers, output rows, unified fallback, selection, and fullscreen viewer behavior.

## What Is Not Done

- Do not re-implement the Grok TUI;
- do not copy Pi-TUI;
- do not add an adapter-specific command palette;
- do not simulate toast, widget, or modal with character art;
- do not wrongly expose Grok login, cloud session, usage, plugin, or other product backend features to Pi;
- do not forge an extension component factory that Pi RPC does not expose.
