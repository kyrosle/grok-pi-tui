# Grok Native TUI × Pi Verification Report

## 2026-10-03 Pi-first current evidence

Implementation started at clean `main@222d614d`. [SPEC](issues/架构/20261002-pi-first-tui-SPEC.md) defines FR-01–11; [PLAN](issues/架构/20261002-pi-first-tui-PLAN.md) retains the execution history. Grok native components own all terminal surfaces. Installed Pi **1.0.0** and its official SDK/extension APIs own models, providers, auth, tools, MCP, children, context and sessions. The optional `pi-main` submodule is uninitialized; no Pi core source was modified. This run is macOS arm64 with system Pi 1.0.0, Node v25.9.0, Bun 1.3.14 and the repository Rust 1.94.0 toolchain; it does not establish other-platform runtime acceptance.

| Proof layer | Current result | Evidence and limit |
|---|---|---|
| Production dependency graph | PASS | `normal,build`, production `--no-default-features --features jemalloc,sandbox-enforce`: 805 unique packages including the root, versus initial 1013. The seven excluded packages are agent, tools, workspace, MCP, sampler, Shell and plugin-marketplace. Package counts do not establish build-time or binary-size savings. |
| Pi and stock checks | PASS | Isolated Pi binary and default-feature stock Pager checks exited 0. Stock compilation preserves its normal runtime; Pi uses neutral contracts and native UI components. |
| Isolated Pi binary tests | PASS | 96 tests, exit 0, using the production feature set. |
| Adapter / neutral layers | PASS | Adapter 196; ConfigTypes 66; Shared 312; Workflow 150 tests. Complete configuration/session/tool DTOs, canonical permission/origin helpers and compatibility re-exports remain. Shared config preserves all 15 top-level fields, double-lock/atomic persistence, cache isolation and hard IO/parse errors. |
| Workflow actual SDK / session scope | PASS | 2 Bun tests / 41 assertions and 1 ignored actual-Pi integration test. Synthetic provider and temporary sessions cover model/effort/capabilities, resume/fork/worktree, abort, child drain ACK, cancelled switch, accepted new/switch and durable scope isolation. |
| Eval v2 / actual Pi + local MCP | PASS | Existing production regression and 12 RPC scenarios: normal/Eval-only official nested calls, parameter/policy errors, exactly-once effects, resources/structured output, hidden/deferred exposure, CLI exclusions, MCP disabled, abort, max-4 parallel/max-1 sequential, background result, and policy after new/switch/fresh load. |
| Preserved login / Voice / telemetry / update | PASS scoped | Login credential resolver 21, Voice shared bearer 8, external telemetry requirements pin 15 and Update no-default 22 tests. These checks preserve native Voice/update/config boundaries; no real STT or account request. |
| Existing optional extension regressions | PASS scoped | Subagents final source runtime 13 tests plus earlier 27-test checkpoint resolving installed SDK; unchanged Remote TUI 13 tests. Actual child SDK/ACP/native tests below cover the updated provider inheritance. V2 remains off by default and real-model teams remain separate acceptance. |
| Auth extension | PASS | 5 tests / 26 assertions: initial provider/method selections, Anthropic copy code, cancel/logout scope, Radius unknown fields/invalid bytes/symlink preservation. No real credentials, account authorization or browser flow. |
| Actual Subagent SDK | PASS | Production child registration with inherited provider; actual UDS live/replay; host-crash cancellation and repeat-recovery dedupe. Native child card/view open-close and native Codemode image cache/Open Image hit rectangle each pass their focused tests. V2 remains opt-in. |
| Actual Pi → ACP | PASS | Seven cases: normal Eval, Eval-only, Codemode, child, auth signal, timeout and EOF. Live and active-branch replay retain nested/image/resource payloads; scoped dialogs retract. This is transport/SDK evidence with a synthetic provider. |
| Native renderer / Markdown / Mermaid | PASS | Render 1216; Markdown 496; Mermaid SVG 85; actual SVG→PNG engine 8 tests, exit 0. Render's old theme failure was a stale test color replacement; the current fixture asserts exact JSON text colors and the complete suite passes. |
| Native lifecycle / clipboard / repaint | PASS | Reader 3, restore 3, teardown fence 1, clipboard 4, full image repaint 1 and modified Enter 1 tests. Inline 67 and isolated Minimal full lib 96 tests pass. The same fresh binary passes the nine-case native PTY below. |
| Production build | PASS | Actual `./build.sh`, exit 0; 83.008 s incremental dev build. Final binary 184,086,200 bytes, SHA-256 `f81b4e47124da5884c81df4ca32943e302bb9be080088d86c5db1f76a598e9cd`, built from the precommit frozen source at `HEAD=a2644524` with a dirty working tree stamp. Initial stock-feature debug snapshot was 213,136,168 bytes: 29,049,968 bytes smaller in this local snapshot comparison. No controlled cold-build timing was measured. |
| Native PTY | PASS | Nine cases on the same fresh binary: Eval, Codemode, auth signal/timeout/EOF retraction, Minimal 120→64→120 resize, F2 save/reopen/failed-save rollback. All nine native exits 0; binary SHA above matches before/after. Actual fixture execute traces prove one nested call in Eval/Codemode/Minimal. |
| Source / protocol / negative / syntax / mock | PASS | Sourceguard 21/21; all 664 current-phase files pinned (`unfrozenPhaseSeams=[]`), with baseline/declaration/identity/renderer errors empty. Original 3797 Git blobs, historical 4479-file integration and 830 native-component inventory remain exact; 3147 files retain protected ancestor bytes. Recursive rustfmt parses 1649 Rust files; mock 7 checks / 33 lines; negative source/coverage/provenance/phase/self-content/self-SHA guards pass. |
| Combined verification entry | PASS | Actual `CARGO_MAINTENANCE=0 ./verify.sh`, exit 0, ending `All verification passed.` It runs sourceguard/mock/syntax, Pi and stock checks, graph 805 / forbidden set empty, adapter 196, isolated bin 96 and both Pager command-filter/compact tests. JSON reports use ignored `verification-logs`, preserving tracked historical records. |

The neutral Shared configuration retains `cli`, `models`, `ui`, `harness`, `skills`, `compat`, `management_api_key`, `permission`, `diagnostics`, `session`, `ask_user_question`, `privacy`, `consent`, `telemetry` and `features`; existing stock paths re-export their canonical definitions. F2 persistence uses the same product-isolated load/merge and locked atomic write path.

Source identity has three distinct layers: immutable `37949780c144e37df692e3d669051a21fec24f20` / `SOURCE_REV=c4ea71cfdbcdb21e32e41bc25a0043d7d4836714`; historical integration frozen to `222d614d9f12cc8fcd408d59419c7d4d197d1be3` (339 modified, 687 added, 5 removed files); and individually declared current-phase files. Historical `provenance-only` records require semantic review when touched and are not a claim of full upstream alignment. Only the inventory JSON itself uses canonical JSON after removing its own embedded `sha256`; every other reviewed file uses its complete-byte SHA-256. Negative checks reject protected-byte, ancestor-hash, missing inventory, directory exemption, undeclared file, current-phase byte, self-content and self-SHA tampering.

[Selective backport provenance](upstream/TUI_BACKPORTS.md) records the complete native dependency groups and decisions. The seven upstream `Changes:` lists are transcribed in [UPSTREAM_CHANGELOG](upstream/UPSTREAM_CHANGELOG.md). Partial imports do not advance `SOURCE_REV` or claim a complete upstream sync.

## FR-01–11 proof audit

| Requirement | Implemented behavior / named proof | Remaining acceptance boundary |
|---|---|---|
| FR-01 | Pi 1.0 version gate, system package contract, `--pi-bin` / `PI_BIN`, actual RPC and production build entry | Actual fresh production build passes |
| FR-02 | Official tool-context `ctx.executeTool()`; original pipeline invokes hooks/permissions once; abort/concurrency/background fixtures | Explicit external Eval MCP has no tool context and keeps its isolated compatibility path |
| FR-03 | F2 `pi_mcp` default off; Pi connection/trust/OAuth/registry; MCP enabled/deferred/excluded/disabled fixture | Real MCP OAuth/account flow |
| FR-04 | Active/registered/callable distinction, Eval-only `hiddenDeclarations`, recursive orchestration blocked, CLI/hidden policy on new/switch/load | Real model selection of allowed tools |
| FR-05 | Text/image/resource/nested-call structure through actual ACP live/replay; native cache, card, Open Image hit rectangle | Terminal image-protocol variation and human image-viewer experience |
| FR-06 | Pi ModelRuntime login/logout, native QuestionView; initial and nested auth scopes; Radius only after product choice | Real Anthropic/Radius OAuth and browser |
| FR-07 | Cancel calls `clear_queue` and retains editable local lanes; settle/resume/dispatch and remove/edit/reorder tests | Human interaction with local/external queue lanes |
| FR-08 | Complete neutral config/manifest/DTO/helper owners; stock re-exports; neutral Workflow manager/store/host/notify | Real-model workflow experience |
| FR-09 | Production normal/build forbidden set empty; isolated Pi and stock profiles both compile | Local debug artifact comparison recorded; no controlled cold-build speed comparison |
| FR-10 | Terminal/reader/fence, clipboard, repaint, Minimal/Inline, Swift and Mermaid groups have source commit/revision/file/dependency records and passing component tests | Varied-terminal manual acceptance |
| FR-11 | Recursive adapter/composition scan, actual installed-Pi contract, immutable Git-source layers and exact current file declarations | Frozen hashes, negative/syntax/mock reports pass; semantic upstream alignment remains scoped to the adopted groups |

## Reproduce the current checks

Run from the repository root with system Pi >= 1.0.0, Node >= 22.19.0, Python 3, Bun for extension/PTY tests, and the repository Rust toolchain. `./build.sh` skips the unprovisioned optional Pi submodule. Cargo uses the existing shared-target disk guard.

```bash
./verify.sh
./build.sh
./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin grok-pi --no-default-features --features jemalloc,sandbox-enforce
./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin xai-grok-pager
python3 crates/codegen/pi-grok-adapter/tests/pi_dependency_profile.py
./scripts/cargo-shared.sh test -p xai-grok-pager-bin --bin grok-pi --no-default-features --features jemalloc,sandbox-enforce
./scripts/cargo-shared.sh test -p pi-grok-adapter
./scripts/cargo-shared.sh test -p xai-grok-config-types --lib
./scripts/cargo-shared.sh test -p xai-grok-shared --lib
./scripts/cargo-shared.sh test -p xai-workflow --lib
node extensions/pi-grok-bash/test-v2.1.mjs
python3 crates/codegen/pi-grok-adapter/tests/pi_native_rpc_smoke.py
python3 crates/codegen/pi-grok-adapter/tests/pi_subagent_sdk_smoke.py
bun test extensions/pi-grok-auth/index.test.ts
bun test extensions/pi-grok-workflows/index.test.ts
bun test extensions/pi-grok-remote-tui
./scripts/cargo-shared.sh test -p xai-grok-login --lib credential_provider
./scripts/cargo-shared.sh test -p xai-grok-login --lib shared_api_key_provider
./scripts/cargo-shared.sh test -p xai-grok-telemetry --lib requirements
./scripts/cargo-shared.sh test -p xai-grok-update --no-default-features --lib
./scripts/cargo-shared.sh test -p pi-grok-adapter --test pi_workflow_scope -- --ignored
./scripts/cargo-shared.sh test -p pi-grok-adapter --test pi_native_projection -- --ignored
bun crates/codegen/pi-grok-adapter/tests/pi_native_pty_smoke.ts eval codemode signal timeout eof minimal settings
python3 crates/codegen/pi-grok-adapter/scripts/verify_native_grok.py --workspace . --pi-source pi-main --json-out /tmp/grok-pi-source-identity-final-20261003.json
python3 crates/codegen/pi-grok-adapter/scripts/test_native_identity.py
python3 crates/codegen/pi-grok-adapter/scripts/check_rust_syntax.py --workspace . --json-out /tmp/grok-pi-rust-syntax-frozen-20261003.json
python3 crates/codegen/pi-grok-adapter/tests/mock_pi_contract.py --pi-source pi-main --json-out /tmp/grok-pi-mock-frozen-20261003.json
```

The Login/Voice/telemetry filter commands above reproduce the recorded test layer; their argv was not captured in the earlier logs. Their actual results are retained in `/tmp/grok-pi-login-contract-final-20261003.log`, `/tmp/grok-pi-voice-bearer-final-20261003.log` and `/tmp/grok-pi-otel-pin-final-20261003.log`. Subagents Node source tests require the actual installed SDK to resolve; earlier checks used an unchanged temporary source copy with SDK imports resolved to that package, rather than provisioning `pi-main`.

The actual-Pi fixtures resolve the installed SDK from the selected `pi` executable and isolate state under temporary directories. Native F2 cases enter an active session through `/new` before opening Settings; Minimal explicitly enables Eval-only and uses its native footer/prompt markers. Actual tool execute traces, rather than rendered source identifiers, establish exactly one nested call. The PTY runner records binary SHA-256 before and after all cases and refuses a changed binary; it disables contextual pasteboard hints and strips credential variables. Use `PI_NATIVE_PTY_ARTIFACTS`, `PI_NATIVE_CAPTURE`, `PI_EVAL_RUNTIME_CAPTURE` and `PI_SUBAGENT_SDK_CAPTURE` to retain their machine reports. Auth tests use a mocked runtime and never authorize a real account.

Native group commands, source SHAs and exact accepted scope are in [TUI_BACKPORTS](upstream/TUI_BACKPORTS.md). A consolidated machine proof record is retained at `verification-logs/pi-first-final-proof-20261003.json`. Final runner commands, exit codes, elapsed times and full logs are retained at `/tmp/grok-pi-final-runner-checks-20261003.json`; graph proof is `/tmp/grok-pi-dependency-profile-final-20261003.json`, build/artifact proof is `/tmp/grok-pi-production-artifact-final-20261003.json`, and the complete nine-case screen/trace/clean-exit report is `/tmp/grok-pi-native-pty-complete-20261003/report.json`. Current passing component checks do not imply that the entire 10,134-test Pager lib suite was executed. `./verify.sh` writes current JSON reports to the existing ignored `verification-logs` tree, preserving tracked historical reports. The combined script ran successfully; its full log is `/tmp/grok-pi-verify-entry-final-20261003.log`, and `verification-logs/cargo-status.json` records `PASS`. Separate negative tamper checks were also executed.

Real model inference, real OAuth/browser flows, varied terminal/image/clipboard behavior and human experience remain independent acceptance layers. All earlier dated sections below are historical snapshots; their old package counts, tree-sitter blockers, source baselines and test counts are not current assertions.

## 2026-09-21 Remote TUI input ownership

Follow-up modified-key regression: the encoder returned `None` for modified
Press events, dropping Shift+S before it reached a component. Standalone
`cargo test -p xai-grok-pager --test remote_tui_keys` reproduced two failures
before the fix and passes all three tests after it. This integration target
links the production library without compiling the stale lib-test fixtures.
Coverage includes Shift+letter actions, Ctrl/Alt/Super presses, BackTab and
repeat/release behavior. The product build passed. A PTY probe loaded the actual
installed Shop `settingsPanel` with synthetic profiles: lower-case `s`, raw
uppercase `S`, and Kitty Shift+S each returned `save`. The probe stops at that
action and does not write any Shop configuration or call a model.

- `bun test extensions/pi-grok-remote-tui`: 13 passed. Covers lifecycle before
  the first frame, explicit child focus, letter/Escape delivery, shortcut
  isolation, native Pi non-interference, stale input rejection and partial JSONL.
- `cargo check -p xai-grok-pager-bin --bin grok-pi` through `cargo-shared.sh`:
  passed (existing warnings only).
- Focused Pager lib tests could not run: the test target has 110 unrelated
  compile errors, including missing timeline helpers and stale TasksPane/AppView
  test fixtures. No error in the newly added Remote TUI tests was reported.
- Adapter transport regression and embedded-extension materialization test each
  passed; `CARGO_MAINTENANCE=0 ./build.sh` passed.
- PTY smoke with system Pi 0.86.1 and the installed Curator: opened settings,
  changed language, saved with lower-case `s`, opened the model picker, searched
  `deepseek`, returned with Escape, force-closed with Ctrl+Shift+Escape, then
  typed into the restored composer. No model inference was requested.
- The smoke exposed out-of-order keyfile records for rapid typing and a delayed
  unread tail. Synchronous ACP enqueue plus ordered keyboard-notification
  handling and periodic draining fixed both in the repeated PTY scenario.
- The original user's exact third-party scene, live multi-terminal interaction,
  and full native-dialog combinations still require acceptance; this is not a
  claim that the entire Pager test suite or all plugin interactions pass.

The renderer exception is limited to letting native dialogs appear above remote
components in `agent_view/render.rs`; existing baseline hashes are unchanged.

Verification date: 2026-07-26
Delivered local main lineage: integration base `1a52f81af9d7f871f7067de35cc57756faf4bd31` (upstream merge `be91fe7`, upstream `47348d1`); restored WIP safety tip `3d4278d`

## Conclusion

The current delivery has passed production build, adapter unit tests, and native Grok architecture plus Pi protocol-contract verification. The entry point genuinely uses Grok Build's production Pager, not a standalone Ratatui/fallback/character-art frontend.

We cannot yet claim **all** verification is green. The Rust syntax stage of `verify.sh` depends on undeclared Python packages `tree_sitter` / `tree_sitter_rust`, which are missing from this environment. The native-source and renderer hash manifests, slash `fork`/`voice` rule, and one mock `agent_settled` completion-barrier expectation are stale and require deliberate baseline review; they were not broadened or regenerated during the merge. One Hooks SSRF test is environment-dependent here because the local resolver maps `.invalid` to an internal IPv6 address, and the same failure reproduces before the merge. Focused Pager lib tests now compile and pass. No new real-model PTY end-to-end smoke test has been added.



## 2026-08-22 Subagents V2 + Eval V2 production review

This review covers the opt-in Subagents V2 collaboration layer, the refactored bundled extension modules, and the current Eval V2 production regression surface. Subagents V2 remains **off by default** (`PI_GROK_SUBAGENTS_V2=1` opts in); passing static/unit checks does not promote it to default-on or replace the required real-model handtest.

| Verification layer | Result | Notes |
|---|---:|---|
| Subagents V2 coordinator/runtime tests | PASS | `cd extensions/pi-grok-subagents && bun test v2.test.ts runtime.test.ts` — 18 passed, 0 failed. Coverage includes scope precedence/malformed preset isolation, root/child routing, idle reactivation with child-session reuse, queued follow-up deferral, nested `FINAL_ANSWER`, atomic team rollback/roster registration, wait/interrupt, canonical record mutation, stable finished output, and queued cancellation. |
| Eval V2.1 production regression | PASS | `node extensions/pi-grok-bash/test-v2.1.mjs` — focused suite completed with `Eval v2.1 focused production regression: PASS`, including JS/Python all-mode host RPC parity, timeout/abort, concurrency/FIFO, background task limits, eval-v2-only tool isolation, regex tool search, and `display(image)` vision forwarding. |
| `grok-pi` binary unit suite | PASS | `cargo test -p xai-grok-pager-bin --bin grok-pi` — 96 passed, 0 failed. The embedded Subagents dependency-materialization test and Bash extension source test are included. |
| Stale Bash injector assertion | FIXED | The source test now requires `PYTHON_EVAL_WORKER_V2`, matching the authored `eval.ts` V2 Python worker selected by `PersistentEvalKernel`. Focused test passes. |
| Diff hygiene | PASS | `git diff --check` completed with no whitespace errors. |
| Extension TypeScript full check | BLOCKED (upstream baseline) | Direct `tsc` currently stops first at `TS2688` because the sibling Node type definitions are not visible; after making the `pi-main/node_modules/@types` path explicit, checking still stops in existing `pi-main` sources (including interactive footer `string`→`never` diagnostics, plus baseline target/declaration compatibility without overrides). No diagnostic from these runs pointed at `extensions/pi-grok-subagents`. The Bun tests above are the executable V2 source check for this increment. |
| Native real-model team E2E | PENDING | Before default-on/release promotion, handtest root→child, child→root, sibling messaging, nested spawn, idle follow-up/session reuse, nested final-answer wakeup, interrupt, concurrency queueing, preset override/disable, and V2-off surface absence with a real target model/provider. |

Reproducible commands for the V2-specific checks are also documented in `docs/usage/subagents-v2.md` and `docs/usage/subagents-v2.zh-CN.md`.

## 2026-07-26 Lossless Main Delivery

| Layer | Result | Notes |
|---|---:|---|
| Main history | PASS | ff-only from `906470c` to `1a52f81`; no rebase, squash, force update, or remote push |
| Restored WIP | PASS | 64 paths (33 modified, 31 added), zero SHA-256 or mode mismatches against safety tip `3d4278d` |
| Herdr extension | PASS | Node socket tests 2/2; Rust `grok-pi herdr` tests 3/3 |
| Model management | PASS | focused Pager model tests 12/12 |
| Product compile | PASS | `cargo check -p xai-grok-pager-bin --bin grok-pi` |
| CLI smoke | PASS | `target/debug/grok-pi --help` |
| Recovery | PASS | two original stashes, combined/rebased safety branches, binary patch and manifest retained |

## 2026-07-18 Subagent Adaptation Increment

A built-in `pi-grok-subagents` extension was added: it creates, tracks, cancels, and persists a child `AgentSession` using the official Pi extension API, and hands it to the adapter through a `pi-grok-subagent/v1` custom-message bridge. The adapter only validates/dedupes and projects to the Pager-consumed `x.ai/session/update` and child-session-id-tagged ACP `SessionNotification`; the Pager body continues to reuse the existing SubagentBlock, Tasks Pane, child AgentView, and cancel UI.

| Verification layer | Result | Notes |
|---|---:|---|
| Pi custom-message bridge probe | PASS | RPC JSONL `message_start`/`message_end` both preserve `customType`, `display:false`, and structured `details` |
| Tempfile extension load | PASS | Copied the extension to a standalone tempfile and loaded it via `pi --mode rpc --extension <temp>.ts`; the hidden cancel command appears in the command catalog |
| Adapter unit tests | PASS | `cargo test -p pi-grok-adapter`: 53 passing |
| `grok-pi` binary unit tests | PASS | `cargo test -p xai-grok-pager-bin --bin grok-pi`: 7 passing |
| `grok-pi` check | PASS | `cargo check -p xai-grok-pager-bin --bin grok-pi` succeeds; only a pre-existing `PiModel.reasoning` dead-code warning |
| Pager child-route lib test | BLOCKED | Focused test compilation blocked by a pre-existing unrelated Pager test config error: missing `set_voice_mode_enabled_for_test`, layout parameter drift, `ActiveModal: Debug`, `AppView` init field drift |
| Native TUI E2E with a real model | PENDING | Manual verification of spawn/progress/child view/finish/cancel/resume/replay is not yet done; static passes must not be treated as runtime acceptance |

## 2026-07-28 Subagent Configuration Increment

`/subagents` now displays the built-in profiles and edits only product-isolated
project/global Markdown overrides. Tools/models retain the existing Pager
QuestionView flow; extension/skill selection opens the existing Pi resource
manager in a non-mutating selection mode. `/subagent-message` and
`send_message_to_subagent` use Pi's official child-session prompt API for a
follow-up or a steer. None of this modifies Grok's original subagent
implementation: Pi remains responsible for child sessions, tools, models,
extensions, skills, and turn steering.

| Verification layer | Result | Notes |
|---|---:|---|
| Pi RPC extension-load probe | PASS | System Pi `0.82.1` loaded `extensions/pi-grok-subagents/index.ts`; `get_commands` returned both `subagents` and `subagent-message`. |
| Native QuestionView multi-select adapter test | PASS | `cargo test -p pi-grok-adapter product_multi_select_envelope_uses_native_checkbox_answer_shape` — 1 passing. |
| Pi resource picker adapter test | PASS | `cargo test -p pi-grok-adapter product_resource_picker_envelope_round_trips_selected_paths` — 1 passing. |
| Pager/resource-picker compile | PASS | `cargo check -p xai-grok-pager -p pi-grok-adapter` completed successfully; only pre-existing warnings remain. |
| Embedded extension source test | PASS | `cargo test -p xai-grok-pager-bin --bin grok-pi subagent_extension_source_is_a_loadable_typescript_module` — 1 passing. |
| Product compile and diff hygiene | PASS | `cargo check -p xai-grok-pager-bin --bin grok-pi` and `git diff --check` completed successfully; existing warnings remain. |
| Extension TypeScript check | PARTIAL | No diagnostic originates in `extensions/pi-grok-subagents`; the full check is blocked by three pre-existing `pi-main` diagnostics: two stale provider model-catalog assertions and missing `highlight.js` declarations. |
| Pager unit test harness | BLOCKED | `cargo test -p xai-grok-pager …` currently fails before this picker test because an unrelated existing test lacks `handle_switch_model_complete` import in `app/dispatch/tests/session/lifecycle.rs`; the normal library check passes. |
| Native TUI real-model E2E | PENDING | Manually exercise built-in override/restore, `/subagents` selection/save, project shadowing, extension/skill picker apply/cancel, `/subagent-message`, spawn, and soft `max_turns` summary before release. |

## Executed Results

| Verification layer | Result | Notes |
|---|---:|---|
| Native Grok architecture audit | PASS | `grok-pi` lives in `xai-grok-pager-bin` and enters `xai_grok_pager::app::run_external` |
| Self-draw/fallback exclusion | PASS | adapter is library-only, no Ratatui/Crossterm/terminal loop; old `pi-grok-tui` does not exist |
| Grok native source integrity | PASS | 2696 files in the original tree remain SHA-256 identical; only 19 declared composition/ACP/state/command seams changed |
| Renderer/Input/Markdown integrity | PASS | 283 core files are byte-for-byte identical to the uploaded Grok source |
| Pi RPC command contract | PASS | all 13 RPC commands used by the adapter exist in the in-package Pi `rpc-types.ts` |
| Pi event contract | PASS | all 20 mapped lifecycle/stream/tool/queue/compaction/retry/UI event types are locatable in Pi source |
| Extension UI | PASS | all 9 methods exposed by Pi RPC have a native Grok UI route |
| Mock JSONL RPC | PASS | 27 interactions covering bootstrap, history, commands, stream, tool, UI response, and `agent_settled` |
| Rust tree-sitter parsing | BLOCKED | `verify.sh` does not declare or pre-check `tree_sitter` / `tree_sitter_rust`; the module is missing in the current environment |
| Shell script syntax | PASS | `build.sh`, `run-local.sh`, `run-installed.sh`, `verify.sh` pass `bash -n` |
| Patch applicability | PASS | `patch --dry-run -p1` against the uploaded original Grok tree applies cleanly for all 29 source/manifest files |
| `cargo check` | PASS | `cargo check -p xai-grok-pager-bin --bin grok-pi` succeeds; only 1 pre-existing dead-code warning in the adapter |
| Adapter Rust unit tests | PASS | `cargo test -p pi-grok-adapter`: 17 passing |
| `grok-pi` binary unit tests | PASS | `cargo test -p xai-grok-pager-bin --bin grok-pi`: 1 passing |
| Pager focused lib tests | PASS | settings-modal suite: 173 passing, 1 ignored; `external_builtin_filter_accepts_aliases_and_omits_product_commands` and `slash_compact_with_context_enqueues_command` both pass |
| Local Pi npm build | PASS | `npm run build` succeeded in a Node.js `v24.15.0` environment |

Machine-readable reports:

- `crates/codegen/pi-grok-adapter/docs/native-grok-verification.json`
- `crates/codegen/pi-grok-adapter/docs/mock-pi-contract.json`
- `crates/codegen/pi-grok-adapter/docs/rust-syntax-verification.json`
- `verification-logs/cargo-status.json`
- `verification-logs/environment-status.json`
- `verification-logs/patch-status.json`

## Key Architecture Evidence

### Production Grok Pager Entry Point

`crates/codegen/xai-grok-pager-bin/src/bin/grok-pi.rs` performs only composition work:

1. Start `pi --mode rpc`;
2. Convert Pi JSONL RPC to ACP;
3. Construct `AcpConnection::external`;
4. Call `xai_grok_pager::app::run_external`.

This file creates no Ratatui `Terminal`, `Frame`, or Widget, and does not read Crossterm input.

### Native Component Reuse

`run_external` continues to use Grok's:

- terminal init/restore and writer thread;
- production event loop;
- PromptWidget and keyboard input;
- slash `CommandRegistry`, suggestion/dropdown;
- Markdown/code/diff/tool rendering;
- scrollback, find, copy, transcript, export;
- QuestionView;
- toast, sticky banner, terminal title;

so every visible terminal surface is Grok Pager, not a second TUI.

### Modification Boundaries

Grok-side changes are limited to:

- adding the external ACP connection/profile;
- gating product features of the external backend;
- Pi Extension UI notifications entering existing Grok surfaces;
- QuestionView gaining `initialText`/`noFreeform` semantic hints;
- merging dynamic Pi commands with allowed Grok builtins;
- `/compact <instructions>` parameter pass-through;

The renderer, input engine, Markdown engine, tool renderer, and minimal renderer bodies are not rewritten.

## Must Run On A Machine With The Toolchain

Requirements:

- Rust toolchain `1.92.0` (see `rust-toolchain.toml`);
- Node.js `22.19.0` or higher;
- Python 3 (for verification scripts);
- workspace dependencies installable.

Run:

```bash
./build.sh
./scripts/cargo-shared.sh test -p pi-grok-adapter
./scripts/cargo-shared.sh test -p xai-grok-pager-bin --bin grok-pi
./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin grok-pi
```

Or run step by step:

```bash
./build.sh
./scripts/cargo-shared.sh test -p pi-grok-adapter
./scripts/cargo-shared.sh test -p xai-grok-pager-bin --bin grok-pi
./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin grok-pi
```

Then build the full run chain:

```bash
./build.sh
```

## Runtime Acceptance Checklist

After a successful build, manually verify at least:

1. The screen, PromptWidget, command dropdown, Markdown, and tool cards match Grok Build Pager;
2. `/help` shows only allowed Grok local commands, merged with Pi dynamic commands;
3. Pi extension `notify`/`setStatus` no longer produce fallback text messages;
4. `select`, `confirm`, `input`, `editor` use the Grok QuestionView;
5. `/model` and `/effort` actually change the Pi model/thinking level;
6. a normal submission during the active turn enters Pi follow-up, send-now enters steer;
7. `!command` uses the Pi `bash` RPC and renders as a Grok tool card;
8. `/new`, `/compact instructions`, `/rename` take effect;
9. restarting an existing Pi session restores history, reasoning, images, and tool results;
10. minimal/fullscreen is selected via startup arguments, and the terminal restores correctly on exit.

## Upstream Integration Record

Date: 2026-07-17
Branch: `sync/upstream-98c3b24` (not yet merged back to `main`)

| Item | Result |
|---|---|
| Upstream tip | `98c3b24` (includes `8adf901`) |
| Strategy | Git merge with a common ancestor `c68e39f` plus seam fixes, **not** a blind merge onto main |
| `grok-pi` unit tests | 4/5 PASS; 1 item `--append-system-prompt` naming drift is a pre-existing main failure |
| Architecture invariants | adapter headless; Pager is the only TUI; Pi is the only core |

Known remaining blockers are the Python tree-sitter dependency, deliberate verifier/mock baseline maintenance, the resolver-dependent Hooks test, and manual real-model runtime acceptance described above.
