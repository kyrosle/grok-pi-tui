# Selective native TUI backports

The Git ancestor remains `37949780c144e37df692e3d669051a21fec24f20`, with `SOURCE_REV=c4ea71cfdbcdb21e32e41bc25a0043d7d4836714`. Historical integration changes and source carryovers are declared separately in the exact source inventory; this is not a claim that all current files match that ancestor. The [upstream update record](UPSTREAM_CHANGELOG.md) describes the complete pending range through `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`. The groups below are selective imports, not a full sync.

| Group | Source commit / source revision | Adopted scope | Validation |
|---|---|---|---|
| Delivered SUPER+Enter | `a28ee2b2` / `e8563f8f182296ebb53cadb3e1eab7615d76408e` | `pager-render/src/input/{terminal_support,mod}.rs`, `pager/src/views/prompt_widget/{mod,tests}.rs`: explicitly insert a newline for a delivered SUPER+Enter and replace the current selection; keep Shift/Alt newline and bare-Enter send semantics | PASS: native public PromptWidget integration (1 test, 3 modifiers) and terminal_support matcher (2 tests). Final native public integration retest: 1 test, exit 0; render suite 1216 tests. Initial 53 stale Pager lib fixtures are now repaired enough to compile and run the targeted lib tests |
| Untrusted display characters | `97f190f644ae1ba07fd6ee185ef54c650e142666` / `f589e31cc17fbf9a41072a379dba4673f6aa0861` | Exact upstream `xai-tty-utils/src/display_char.rs` and tests; module export; `pager-render/src/render/line_utils.rs` re-exports the helper. Extend the existing shared filter to invisible format ranges and line separators, retaining ordinary RTL letters | PASS: TTY display_char (2 tests); existing native filtering now calls this exact helper |
| Link projection boundaries | `482711333c7195dc16a272777f86086d615e2afb` / `be7ce6e8cffe46d20bef9834b211616082ee866b` | Production bounds checks in `markdown/src/hyperlinks.rs` and `pager-render/src/render/osc8.rs`; skip invalid UTF-8 ranges / stale link indices rather than panic. Preserve the current valid-link layout. Broad indexing-lint and test rewrites are not imported | PASS: Markdown hyperlink tests (16) and OSC8 tests (62), including the added stale-index case |

| Terminal restore / reader join / Kitty pop fence | `a28ee2b2063426e8816e380ccea528b9de95e5da` / `e8563f8f182296ebb53cadb3e1eab7615d76408e` | `pager-render/src/terminal/{probe,pop_fence}` and tests; bounded stderr lock in `shared/src/stderr.rs`; `pager/src/app/{reader_thread,teardown_fence,terminal_restore}` and tests, with narrow `mod`, `event_loop`, and signal references. Both external Pi and stock entry paths own/join the reader before the DA1 drain, drain the writer before popping, and disable raw mode after the fence. Existing panic/write bounds and Windows console restore remain | PASS: shared stderr lock test (1), render probe tests (8), and pop-fence parser tests (3). The macOS probe fixture primes XNU FWASWRITTEN before its full flags comparison and also verifies a no-write guard preserves the complete status word. PASS: Pager reader 3, restore 3, teardown fence 1 tests, exit 0; varied-terminal runtime acceptance remains separate |

| Inline image full repaint | `f0e3be1100ef5252488e3be8bb0e91cf68d8c305` / `036a5d8348cd744767cd0b08518ab17bf608fa7f` | `pager/src/app/agent_view/media.rs` and the Presenter force-clear branch in `app/event_loop.rs`: invalidate parent and child transmission IDs after a full clear while keeping encoded image cache, so the next frame resends pixels | PASS: targeted parent/child transmission-cache regression 1 test, exit 0; native terminal image-protocol variation remains separate |

| Clipboard transport and attachment consistency | `482711333c7195dc16a272777f86086d615e2afb` / `be7ce6e8cffe46d20bef9834b211616082ee866b`; `f0e3be1100ef5252488e3be8bb0e91cf68d8c305` / `036a5d8348cd744767cd0b08518ab17bf608fa7f`; `97f190f644ae1ba07fd6ee185ef54c650e142666` / `f589e31cc17fbf9a41072a379dba4673f6aa0861` | Native `shared/src/clipboard.rs` Windows serialization, macOS per-probe 0700 temp protocol and 8s child deadline; `pager-render/src/clipboard/mod.rs` typed drop reasons and before/after changeCount guard; Pager effects 10s stage deadline, bracketed-paste origin check on every terminal, and shared text fallback for agent/dashboard/feedback. Preserve the existing public image/attachments API and telemetry schema; skip fingerprinting/read-path telemetry and backend features | PASS: native effects guard/origin/deadline/fallback regressions 4 tests and full render suite 1216 tests, exit 0; actual OS pasteboard transports are not exercised by the PTY fixture |

| Minimal history reprint / resize / semantic newlines | `f0e3be1100ef5252488e3be8bb0e91cf68d8c305` / `036a5d8348cd744767cd0b08518ab17bf608fa7f` (includes `a28ee2b2` semantic emission) | `xai-ratatui-inline` semantic-row writer, width-shrink/cursor anchoring and tests; render terminal/tmux policy and adopted-size frame API; Pager minimal reprint state/API/ticks and EntryRenderer wrap flags; Minimal commit/full-view/welcome/live/reprint and native modal ownership. Preserve public `draw_frame` signature and canonical Pi-profile imports. Resize adopts size before synchronized output; both marker sites use one terminal policy. History reprint waits 120 ms, clears screen and terminal scrollback, and reprints the newest 4000 rows; older committed entries remain reachable through the native `/transcript` surface | PASS: Inline 67, Render 1216 and isolated Minimal complete lib 96 tests. PASS: same fresh binary native Minimal 120→64→120 shrink/grow/reprint, exactly one committed final body and clean exit |

| Swift syntax and highlighting memory | `a28ee2b2063426e8816e380ccea528b9de95e5da` / `e8563f8f182296ebb53cadb3e1eab7615d76408e` | Markdown patched Swift grammar, build-time syntax dump and runtime cache/grammar selection plus original interpolation/multiline tests; Markdown build dependencies and workspace Syntect/Two-Face Oniguruma feature pair. Keep native public syntax APIs and existing link-boundary fixes | PASS: complete native Markdown lib 496 tests, including Swift grammar/interpolation/multiline and memory/cache selection; native Render 1216 tests, exit 0 |

| Mermaid flowchart Open Image | `4247f661689354b831191f11eeeac8424993fe3d` / `9bb727ccdff0a793ee73bcde4e2e09cbef6b5387` | `third_party/mermaid-to-svg` complete class-annotation/quoted-label/ampersand parser, layout text-color and SVG rendering/construction group with original tests and vendor change note; `xai-grok-mermaid/tests/pure_engine.rs` actual SVG→PNG regression. Preserve the existing native Mermaid engine and Open Image consumer | PASS: complete vendored parser/layout/SVG lib 85 tests; native pure_engine 8 tests through SVG→PNG, exit 0 |

Paths above are relative to `crates/codegen/`, except the explicitly named `third_party/mermaid-to-svg` paths and root workspace feature declarations. These groups only affect native input/rendering and its renderer build dependencies. They add no Grok model, account, session, tool or MCP execution semantics.

The terminal restore group is imported as a complete dependency unit; the extraction does not import the unrelated SignalStreams refactor, terminal width policy, or stock product features. Clipboard safety and image repaint fixes are imported through the existing native consumption chain. Clipboard transport preserves the public `get_image`/attachments API and the existing telemetry event; typed drop reasons live in the render component, so no new telemetry/backend dependency is required.

Other reviewed groups remain separate from these imports:

| Group | Decision and dependency boundary |
|---|---|
| Model-picker context-window step (`97f190f6`) | Keep Pi model/effort selection. Installed Pi 1.0 RPC `set_model` accepts provider/modelId, without Grok's session context-window override; the Grok control is not exposed as a Pi operation. Provider model configuration remains owned by Pi. |
| PlanKept/PlanCleared, post-turn review and `execute_plan` (`48271133`) | These changes depend on stock plan execution/review semantics. Keep the existing Pi Plan extension and native surfaces; no Grok execution gate is introduced into Pi. |
| Broader Markdown layout and indexing lint refactors | The independent syntax and Mermaid PNG groups above are adopted. The remaining broad safe-indexing lint/test rewrites, LaTeX/parser/layout rewrites, and theme-only churn are not imported; they are separate from those functional fixes and do not supply a missing Pi operation. Existing native layout/streaming behavior remains. This record does not claim a full Markdown sync. |

## Reproduce the adopted-group checks

Run from the repository root. The final runner retains exact commands, exit codes, elapsed times and logs at `/tmp/grok-pi-final-runner-checks-20261003.json`; the source freeze/identity report is recorded in [VERIFICATION](../VERIFICATION.md).

```bash
./scripts/cargo-shared.sh test -p xai-tty-utils display_char
./scripts/cargo-shared.sh test -p xai-ratatui-inline --lib
./scripts/cargo-shared.sh test -p xai-grok-pager-render --lib
./scripts/cargo-shared.sh test -p xai-grok-markdown --lib
./scripts/cargo-shared.sh test -p mermaid-to-svg --lib
./scripts/cargo-shared.sh test -p xai-grok-mermaid --test pure_engine
./scripts/cargo-shared.sh test -p xai-grok-pager --test native_tui_backports
./scripts/cargo-shared.sh test -p xai-grok-pager --lib app::reader_thread
./scripts/cargo-shared.sh test -p xai-grok-pager --lib app::terminal_restore
./scripts/cargo-shared.sh test -p xai-grok-pager --lib app::teardown_fence
./scripts/cargo-shared.sh test -p xai-grok-pager --lib clipboard_backport_tests
./scripts/cargo-shared.sh test -p xai-grok-pager --lib full_repaint_forgets_transmission
./scripts/cargo-shared.sh test -p xai-grok-pager-minimal --lib --no-default-features
bun crates/codegen/pi-grok-adapter/tests/pi_native_pty_smoke.ts minimal
```

The inherited `xai-grok-pager-minimal/src/panel.rs` test fixtures add only the current six SessionPicker entry fields and four surface-state fields so the native lib test target compiles. The Render theme rediscovery fixture sets explicit JSON text colors and asserts exact palettes. These are precise fixture repairs based on the existing `222d614d` integration, separate from upstream functional imports; production panel and theme behavior is unchanged.

The final palette repair keeps a `Color::Reset` foreground unknown while retaining the pre-existing synthetic dark Reset background for RGB/Indexed wave accents. Its focused test covers Reset/RGB/Indexed bases and all three opacity points. Minimal exact-width semantic-copy and diff-band fixtures use the existing `pin_theme` guard to avoid concurrent global-theme mutation; no production copy assertion is weakened. Both repairs have exact current-phase declarations based on the historical integration. Final Render 1216 / isolated Minimal 96 suites pass; the fresh binary nine-case PTY and combined `verify.sh` also pass.
