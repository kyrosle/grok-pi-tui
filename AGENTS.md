# AGENTS.md — Pi-Grok Native TUI

## Project root and Git

This repository root (`grok-pi-tui/`) is the primary project checkout. Do not treat a parent directory as the project. Linked Git worktrees, when explicitly requested, share this checkout's Git common directory.

```text
origin   https://github.com/kyrosle/grok-pi-tui.git
grok-build https://github.com/xai-org/grok-build.git (reference; push disabled)
```

- Work from this directory; do not use its parent wrapper as a repository.
- `origin/main` is the product branch. Grok Build is a reference repository, not an upstream to merge.
- Source inherited from Grok Build may be changed, renamed or removed. Preserve licenses and copyright notices; port UI improvements deliberately.
- Keep commits focused. Do not stage generated `pi-main` model catalog changes unless they are intentional.
- Pi host default is **system `pi` >= 1.0.0** (`npm i -g @earendil-works/pi-coding-agent`). Override with `--pi-bin` / `PI_BIN`.
- Optional Pi source checkout is the git submodule [`pi-main`](https://github.com/earendil-works/pi) (not a vendored copy). Follow [`pi-main/AGENTS.md`](pi-main/AGENTS.md) when working inside the submodule.

The governing documents are [Pi native TUI SPEC](docs/issues/架构/20261003-pi-native-tui-SPEC.md) and [PLAN](docs/issues/架构/20261003-pi-native-tui-PLAN.md). The final product has one grok-pi binary, Pi as the only core, a native Rust TUI, and no stock Grok profile or business runtime. Execute T0–T8 in order; a completed checkpoint is not completion of the master SPEC.

The [Pi-first](docs/issues/架构/20261002-pi-first-tui-PLAN.md), [deep adaptation](docs/issues/架构/20261003-pi-deep-adaptation-PLAN.md) and [product surface](docs/issues/架构/20261003-pi-product-surface-PLAN.md) records are historical checkpoints. Adapter queue/Plan/Goal ownership, private extension hooks and compiled Grok dependencies remain master-plan work.

## Grok Build reference review

- Fetch `grok-build` and review `docs/grok-build/REVIEWED..grok-build/main` using [grok-build-review](.pi/skills/grok-build-review/SKILL.md). Record each change as port, adapt then port, skip business, or observe, with reasons and target paths in [REVIEW_LOG](docs/grok-build/REVIEW_LOG.md).
- Do not merge the reference repository wholesale. Actual ports use an isolated worktree, a narrow patch or a rewrite, and their own validation/commit. Commit trailers record `Ported-From: grok-build <sha> (Source-Revision <rev>)`.
- Advance `REVIEWED` only after the range's decisions are recorded. It is a review watermark, not a source-sync or adoption claim.
- Previous sync records and byte-identity inventories are [archived](docs/grok-build/archive/README.md); they do not constrain current source edits. Crate names may remain `xai-*` for reference-path continuity.

## Architecture invariants

1. **Grok Pager is the only terminal UI.** All visible terminal surfaces must come from `xai-grok-pager` or native component crates.
2. **Pi is the only agent core.** Pi owns models, providers, agent loop, tools, extensions, compaction, retries, and sessions.
3. **`pi-grok-adapter` is headless and library-only.** It may translate Pi RPC or the official-SDK Durable host JSONL ↔ ACP, but must not render widgets, own a terminal, read keyboard events, or depend on Ratatui/Crossterm.
4. **Reuse native Grok surfaces.** Map Pi capabilities to existing Pager prompt, slash, QuestionView, toast, banner, tool card, diff, and scrollback surfaces. Do not create a second TUI or ASCII fallback UI.
5. **Do not modify Pi source to extend RPC.** When a Pi core capability is not exposed over RPC, prefer the official extension API. Preserve Pi semantics rather than emulating them with JSONL edits or unrelated RPCs.
6. **Product-isolated state trees.** grok-pi must not share stock Grok’s user or project config roots (see [Product state isolation](#product-state-isolation)).
7. **No Grok business in the final product.** Pending dependencies are transitional T2/T7 work, not permanent exceptions. Validation uses dependency, endpoint, protocol, entry-policy and PTY guards; it does not require reference blob identity.

Read [`NATIVE_GROK_TUI_ALIGNMENT.md`](docs/NATIVE_GROK_TUI_ALIGNMENT.md) and [`FEATURE_MATRIX.md`](docs/FEATURE_MATRIX.md) before changing protocol or UI behavior.

## Product state isolation

Stock Grok uses `~/.grok` (user) and `<repo>/.grok` (project). **grok-pi defaults are product-isolated** so UI settings, trust, skills, hooks, and workflows do not collide with stock Grok:

| Layer | stock Grok | grok-pi default | Override |
|---|---|---|---|
| User home | `~/.grok` | `~/.grok-pi` | `$GROK_HOME` |
| Project tree | `<repo>/.grok` | `<repo>/.grok-pi` | `$GROK_PROJECT_DIR` |

Rules:

- `ensure_default_grok_home()` (grok-pi startup) sets `$GROK_HOME` → `~/.grok-pi` and `$GROK_PROJECT_DIR` → `.grok-pi` when unset.
- Resolve project paths only via `xai_grok_config::project_config_dirname()` / `project_config_dir(root)` — never hardcode `.join(".grok")` for project assets in grok-pi production code.
- User paths go through `grok_home()` / `$GROK_HOME` (workflows → `$GROK_HOME/workflows`, config → `$GROK_HOME/config.toml`, etc.).
- **No dual-scan of stock trees by default.** grok-pi does not auto-read `~/.grok` or `<repo>/.grok` for project discovery; migrate with `grok-pi migrate-home` (allowlisted user files only — **not** `workflows/`) or copy project trees manually into `.grok-pi`.
- Until T7, stock-profile tests retain their legacy defaults. T7 makes Pi behavior the sole default and removes stock tests/profile explicitly.
- Design note: [`docs/issues/架构/20260722-项目级.grok-pi隔离.md`](docs/issues/架构/20260722-项目级.grok-pi隔离.md).

Examples under a git repo:

```text
~/.grok-pi/config.toml              # F2 / UI (e.g. [ui].pi_workflows)
~/.grok-pi/workflows/*.rhai         # user workflows
<repo>/.grok-pi/workflows/*.rhai    # project workflows (folder trust)
<repo>/.grok-pi/hooks/              # project hooks
<repo>/.grok-pi/config.toml         # project config overlay
```

## Important paths

| Concern | Path |
|---|---|
| Composition binary | `crates/codegen/xai-grok-pager-bin/src/bin/grok-pi.rs` |
| Default home / project dirname | `crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/home.rs` |
| Path helpers (`grok_home`, `project_config_dir`) | `crates/codegen/xai-grok-config/src/paths.rs` |
| Pi JSONL RPC transport | `crates/codegen/pi-grok-adapter/src/pi_rpc.rs` |
| Pi data parsers | `crates/codegen/pi-grok-adapter/src/model.rs` |
| ACP adapter and UI mapping | `crates/codegen/pi-grok-adapter/src/pi_adapter.rs` |
| Native Pager external-profile seams | `crates/codegen/xai-grok-pager/src/app/` |
| Pi RPC facts (submodule / installed package) | `pi-main/packages/coding-agent/src/modes/rpc/` or npm package dist |
| Pi session lifecycle facts | `pi-main/packages/coding-agent/src/core/agent-session.ts` |
| Architecture and task records | `docs/` |
| Reference review watermark / log | `docs/grok-build/REVIEWED`, `docs/grok-build/REVIEW_LOG.md` |
| Reference review skill | `.pi/skills/grok-build-review/SKILL.md` |
| Project `.grok-pi` isolation issue | `docs/issues/架构/20260722-项目级.grok-pi隔离.md` |

## Session and tree rules

- These JSONL/session-tree rules describe default Pi RPC. Opt-in Durable follows [its SPEC](docs/issues/架构/20261007-pi-durable-integration-SPEC.md): official Harness/SQLite, separate conversations and product-isolated stores, no implicit cross-backend conversion.

- Pi owns session files, trees, and the active leaf.
- `/resume` must use the native Grok `SessionPicker`; catalog scanning is on-demand, never startup work.
- Respect Pi's default session root, `--session-dir`, `PI_CODING_AGENT_SESSION_DIR`, and `sessionFile`-derived custom directories.
- `navigateTree()` changes Pi's in-memory leaf and context. Do not fake it with `fork`, `switch_session`, or direct JSONL mutation.
- If tree navigation is added without changing Pi source, bridge the official Pi extension API (`ctx.navigateTree`) and render only with native Grok components.

## Build and verification

Run from the project root:

```bash
./build.sh
./scripts/cargo-shared.sh test -p pi-grok-adapter
./scripts/cargo-shared.sh test -p xai-grok-pager-bin --bin grok-pi --no-default-features --features jemalloc,sandbox-enforce
./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin grok-pi --no-default-features --features jemalloc,sandbox-enforce
./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin xai-grok-pager
python3 crates/codegen/pi-grok-adapter/tests/pi_dependency_profile.py
```

`./build.sh` builds `grok-pi` with `--no-default-features --features jemalloc,sandbox-enforce`; stock Pager keeps its default features. Plain default-feature Cargo checks do not prove Pi production isolation. When the optional `pi-main` submodule has workspace dependencies installed (`pi-main/node_modules/.bin/tsgo`), it rebuilds the coding-agent checkout first; a freshly initialized, unprovisioned submodule is skipped. Requires system Pi >= **1.0.0**, Node.js >= 22.19.0, and the repository Rust toolchain.

All linked worktrees share one Cargo output tree at `<git-common-dir>/pi-grok-cargo-target`. `./build.sh` and `./verify.sh` initialize the root `target` symlink automatically. Use `./scripts/cargo-shared.sh <cargo-args>` for project Cargo commands: it sets up the shared target, enables incremental compilation by default, caps generated target output at 128 GiB, and stops Cargo at the default 20 GiB free-space floor. Override the target cap with `CARGO_TARGET_MAX_GIB`; when a target is already over the cap, periodic maintenance clears incremental roots first and falls back to `cargo clean` if needed. Raise the free-space floor with `CARGO_MIN_FREE_GIB`; use `CARGO_DISK_GUARD_PATH` when output is on a custom filesystem; set `CARGO_MAINTENANCE=0` to skip one pre-command maintenance pass. The running disk guard continuously enforces the free-space floor; target-size maintenance runs on its configured cadence. An explicit `CARGO_TARGET_DIR` remains authoritative for CI or one-off isolation. Never copy `target/` between worktrees. Direct raw `cargo` remains available for deliberate recovery/maintenance, but is not protected by the project guard.

`./verify.sh` checks native/headless architecture, entry policies, the production dependency graph, installed-Pi contracts, mock RPC and Rust syntax. Stock checks remain only until T7. Endpoint and pending-dependency reports expose unfinished cuts; T7 enables strict terminal enforcement. Rustfmt parses Git-listed Rust source without editing it; no Python tree-sitter package or initialized `pi-main` is required. Follow [`VERIFICATION.md`](docs/VERIFICATION.md) for separate source/guard, synthetic transport, native PTY and real provider/OAuth/manual evidence. Historical byte-identity reports are archived, not current gates.

For a standalone change under `extensions/`, validate the extension source and diff only; do **not** run Cargo unless Rust code, the embedded-extension loader, or its Rust contract changed, or the user asks.

Before reporting completion:

1. Run the narrowest relevant tests and build/check.
2. Read the exit status and complete output.
3. Review the diff for scope and whitespace errors.
4. State known blockers separately from passing checks.

## Diagnosing Pi RPC bootstrap / extension failures

When Pi RPC bootstrap fails and grok-pi self-heals by dropping an extension or
relaunching with `-ne`, do not stop at the reported culprit name — the bisect
only names a file (e.g. `index.ts`), never the real error. Follow this order:

1. **Read the real Pi error first.** Pi's complete stderr is appended to
   `{GROK_HOME}/logs/pi-rpc-stderr.log` (default `~/.grok-pi/logs/pi-rpc-stderr.log`)
   by `pi_rpc.rs`'s stderr reader. This is the authoritative failure message
   (e.g. `Cannot find module './eval-tasks.ts'`). Trace it to the exact line
   before touching any Rust injector.
2. **Isolate the injected bundle.** Injected extensions live in
   `$TMPDIR/pi-grok-bash-*` (`NSTemporaryDirectory` on macOS) and are deleted by
   self-heal after the crash, so reproduce a clean copy before the directory
   disappears. Verify the bundle loads in isolation:
   `pi -ne --mode rpc --extension <bundle>/index.ts` (must exit 0).
3. **Check injection completeness.** After the 2026-10-07 four-cut, the current Bash-only bundle consists of `index.ts`, `bash-tasks.ts`, `prompts.ts`, and `shared.ts`; the Eval module list below records the historical bootstrap regression.
   The Rust injector
   (`bash_extension.rs` / `*_extension.rs`) must materialize **every** relative
   import of the TypeScript entry, and the transitive closure of those modules.
   Known regression: splitting the Bash extension into multiple modules
   (`eval.ts`, `bash-tasks.ts`, `eval-tasks.ts`, `prompts.ts`, `shared.ts`,
   `tool-bridge.ts`) added imports the injector did not initially copy, causing
   `Cannot find module './eval-tasks.ts'` at boot. When adding a module, mirror
   it into the injector and extend the injector's single unit test to assert the
   new import and module content.
4. **Rule out tool-name conflicts separately.** `Tool "bash" conflicts with …`
   comes from Pi's `resource-loader.ts` extension-conflict check (two extension
   registrations), independent of bundle loading. It only appears when Pi
   auto-discovers another extension, so reproduce with `-ne` to separate bundle
   errors from cross-extension conflicts.

## Documentation and change control

- Complex work must have a record in `docs/issues/` before implementation.
- Update the relevant Issue after each completed phase.
- Keep `README.md`, `docs/README.zh-CN.md`, `docs/FEATURE_MATRIX.md`, and `docs/VERIFICATION.md` aligned with actual behavior.
- Keep dependency, endpoint, headless-adapter, entry-policy and protocol guards meaningful as the architecture changes. Do not turn pending findings into permanent waivers or describe report mode as terminal acceptance.

## Safety

- Do not run destructive Git commands (`reset --hard`, broad restore, force checkout).
- Do not remove source files with `rm`; use `trash` for intentional deletion.
- Do not push, rename, delete, or recreate remote repositories unless the user explicitly authorizes it.
- Treat credentials, tokens, and user session data as private.
