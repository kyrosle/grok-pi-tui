# Grok Native TUI × Pi 验证报告

## 2026-10-03 深度适配：已交付自动证据

本轮从干净 `main@84174917511690447ba32915571e9748b5e10ad4` 开始；[SPEC](issues/架构/20261003-pi-deep-adaptation-SPEC.md)、[PLAN](issues/架构/20261003-pi-deep-adaptation-PLAN.md)、[source review](issues/架构/20261003-pi-deep-adaptation-SOURCE.md) 保留范围、检查点与失败 captures。修正后的 shipping source/fixtures 已冻结，最新 build、4 native PTY、组合 verify 均通过；一个明确委任的集成 runner 运行 Cargo/build/commit。按最新用户所有权澄清，功能/鉴权交给Pi，TUI开发以原生委托/响应/取消/生命周期合同验收。真人OAuth/image/provider/目标终端体验仍补充pending，不再作为开发关闭前置。下方已构建binary/PTY/combinedverify早于独立auth裁剪，保留为checkpoint，不冒称新auth源码已构建。

| 要求 / 证据层 | 结果 | 证据与边界 |
|---|---|---|
| Auth所有权裁剪 | PASS，独立扩展 | `bun test extensions/pi-grok-auth/index.test.ts`5tests/28assertions/exit0；通用Radius委托Pi，无MCP确认/reload/writer，已有bytes不变、缺mcp.json不创建；保留select/copy/input/cancel/logout scope。Rustloader/contract未变，不跑Cargo/build或真实账号。 |
| DA-01 / disposition | PASS，有界 | Adapter lib207、disposition1 target/4scenarios、actuallifecycle1；started/queued/handled、晚回目标slot/reservation、取消、operation-scoped有界legacyprobe保持独立工作及一次completion。 |
| DA-02 / 运行控制 | PASS | 官方retry/compaction/abort-retry、持久化与native取消通过SDK/RPC/PTY；livecompaction与configuredretry分开，公开RPC没有live retry-policy getter。 |
| DA-03/04 / package/reload | PASS，命名路径 | Native真实Pi local install/remove及factory/command registry通过。FreshSDK installedPath/publicCONFIG_DIR_NAME settingsbase校正声明相对source；继承update取选中resource实际source scope。保留pins/filters/CLI限制/trust；不变admission走ctx.reload+ACK，变更输入走officialEOF/restart与公开session/leaf/model/thinking恢复。 |
| DA-04 / safe defer | 已确认边界 | 无persistent sessionFile及user-message leaf保留旧process/context、返回saved/deferred，待完成响应或手动重启；不改用户JSONL、不静默变history。 |
| DA-04 / loader errors | 已确认边界 | Invalidrelativeimport可能officialreload成功但无command/stderr/extension_error；只陈述registry并loaded:null/loadStatus:unverified。Native前置Load unverified防左pane裁掉边界。false/missing/nonboolean业务ACK单独失败；有界stderr不覆盖完整hiddenloadererrors。 |
| DA-04 / Web revision | PASS，有界 | Bun7tests/195assertions：GETpreflight+serverrevision/If-Match拒绝缺条件428/旧版本409，保留未知字段。compare/rename不是外部同时写的完整CAS，公开SDK无FileSettingsStorage transaction。 |
| DA-05/06 / 模型 | PASS，合成管线 | ActualPi virtualdispatch/context/cost+image/classifier Codemode live/replay ACP及nativeimageaffordance/classifiercost通过；selected/physical/thinking与unattributed明确，chatpicker排除非chat；不代替真实图片provider推理。 |
| DA-07/08 / extension UI | PASS，有界 | standard/mapped/degraded/unsupported、workingstatus、实际host门控facade、focus/input/resize/dispose、SDK/nativeprojection1target/7scenarios；不保证所有第三方factory。 |
| DA-09 / tests/profiles | PASS，有界 | Combinedverify：adapter207lib+disposition1/ACK1/EOF2非ignored、隔离binary98、productionno-default jemalloc/sandbox-enforce与stockdefaultchecks、另2个native单testfilter。Actualignored resource3/lifecycle1/nativeprojection1/modelprojection1另实际执行；资源modal17及其他focused通过，未跑总10146-testPager全suite。 |
| DA-09 / graph | PASS | Freshproductionnormal/build806packages，七禁止stockruntimes全空；stockcheck另exit0/1m55s。数量不代表coldbuild节省或跨平台runtime验收。 |
| DA-09 / source/negative/syntax/mock | PASS | Guard21/21：原始3797Git blobs、历史4479、上一84174917、native830/protected3147分层；phase722逐文件、unfrozen空、无目录豁免。Recursive rustfmt1662/failures空、negative、mock8checks/33lines/stderrempty通过；18声明RPC含explicitgenerator，不称任意computed类型全覆盖。仅selfmanifest canonicalomitselfSHA。 |
| DA-10 / 桥层开发与补充体验 | 桥证据PASS；补充流程PENDING | 原生UI委托、响应、cancel/timeout/EOF与生命周期支持TUI开发验收；defaultSDKchat HTTP200/requests1/OK/真实files不变。OAuth/image/native真实provider/真人terminal独立pending；最新用户范围澄清取代旧真人关闭前置。 |

最终artifact：**184,693,160bytes**，SHA-256 `9c46d5c6d0b9a3b85657b2fbb5ecc2bcd8e2256a571347863f03db36115c74ad`；正式build exit0/Cargo22.34s，proof `/tmp/grok-pi-deep-artifact-settings-scope-final-20261003.json`，log `/tmp/grok-pi-deep-build-settings-scope-final-20261003.log`。Source stamp为precommit fe7515f528e7cb6ace46ed8dfa7edbe6b32fcc5e+dirtyfrozen settings-scope，不声称cleanfinalHEAD/coldbenchmark；后续doc-onlyreceipts不变更测试binary。

最终4nativePTY均exit0，前后binarySHA不变：runtimecontrols/retrycancel；package实际localinstall/remove、registry/session/file/leaf/branch/model/thinking保全、local源保留、取消PIDdead；virtualmodel/image/classifiercard/cost；RemoteTUIresize/input/dispose。报告 `/tmp/grok-pi-deep-pty-settings-scope-final-20261003/report.json`，保留真实process/UItrace与xterm/headlesscaptures。旧readiness/admission/origin/noticevisibility/relativebase失败保留PLAN，不抹掉。

Combined `./verify.sh` exit0并结束Allverificationpassed，log `/tmp/grok-pi-deep-verify-settings-scope-final-20261003.log`，完整archive `/tmp/grok-pi-deep-verify-settings-scope-final-20261003/`。Transport2/actualrestore3/packageunit3/nativemodal17最新logs见PLAN；早期no-defaultPagerlibtest167feature-dependent编译错误仍记录，defaultfocused不冒称全suite。

真实SDK命令 `node crates/codegen/pi-grok-adapter/tests/pi_real_chat_smoke.mjs --execute-once`，项目rootcwd/exit0，safeproof `/tmp/grok-pi-real-chat-sdk-20261003.json`。Thinkingminimal→sentlow；input21/output5/total26/reasoning0；SDKcatalogcost0.000092（非billingreceipt），无abort。Server不支持maxTokens32hardcap，使用SSE/noretries、20sdeadline与观察>64textbytes后abort，不保证<=32。已有freshOAuth仅内存，无tools/session/refresh/login/credential-command/persistent写；preflight0dispatch/hash不变通过。独立脚本不embedded、不改shippingbinary，不替代剩余DA-10。

## 历史记录：2026-07-17

验证日期：2026-07-17
交付版本：`pi-grok-native-v4.0.0`

## 结论

当前交付已通过生产构建、适配器单元测试，以及原生 Grok 架构与 Pi 协议契约验证。入口确实使用 Grok Build 的生产 Pager，而不是独立 Ratatui/fallback/字符画前端。

尚不能宣称**全部**验证全绿：`verify.sh` 的 Rust 语法阶段依赖未声明的 Python 包 `tree_sitter` / `tree_sitter_rust`，当前环境因缺少该依赖停止；两条 Pager focused lib test 还会在既有跨 crate `#[cfg(test)]` helper 配置上失败（与 Pi adapter 逻辑无关）。尚未新增 `grok-pi` 的 PTY 端到端 smoke test。

## 2026-07-18 子代理适配增量

已新增内置 `pi-grok-subagents` extension：它用官方 Pi extension API 创建、追踪、取消并持久化 child `AgentSession`，通过 `pi-grok-subagent/v1` custom-message bridge 交给 adapter。adapter 仅验证/去重并投影到 Pager 已消费的 `x.ai/session/update` 和带 child session ID 的 ACP `SessionNotification`；Pager 本体继续复用原有 SubagentBlock、Tasks Pane、child AgentView 与取消 UI。

| 验证层 | 结果 | 说明 |
|---|---:|---|
| Pi custom-message bridge probe | PASS | RPC JSONL `message_start`/`message_end` 均保留 `customType`、`display:false` 与结构化 `details`。 |
| Tempfile extension load | PASS | 将 extension 复制到独立 tempfile 后以 `pi --mode rpc --extension <temp>.ts` 加载，隐藏 cancel command 出现在 command catalog。 |
| Adapter unit tests | PASS | `cargo test -p pi-grok-adapter`：53 项通过。 |
| `grok-pi` binary unit tests | PASS | `cargo test -p xai-grok-pager-bin --bin grok-pi`：7 项通过。 |
| `grok-pi` check | PASS | `cargo check -p xai-grok-pager-bin --bin grok-pi` 成功；仅既有 `PiModel.reasoning` dead-code warning。 |
| Pager child-route lib test | BLOCKED | 聚焦测试编译被既有无关 Pager test 配置错误阻断：缺 `set_voice_mode_enabled_for_test`、layout 参数漂移、`ActiveModal: Debug`、`AppView` 初始化字段漂移。 |
| 带真实模型的原生 TUI E2E | PENDING | 尚未手工验证 spawn/progress/child view/finish/cancel/resume/replay；不得将静态通过视为运行时验收。 |

## 已执行结果

| 验证层 | 结果 | 说明 |
|---|---:|---|
| 原生 Grok 架构审计 | PASS | `grok-pi` 位于 `xai-grok-pager-bin`，进入 `xai_grok_pager::app::run_external` |
| 自绘/fallback 排除 | PASS | adapter 为 library-only，无 Ratatui/Crossterm/terminal loop；旧 `pi-grok-tui` 不存在 |
| Grok 原生源码完整性 | PASS | 原始树中 2696 个文件保持 SHA-256 一致；仅 19 个声明的组合/ACP/状态/命令接缝变化 |
| Renderer/Input/Markdown 完整性 | PASS | 283 个核心文件与上传的 Grok 源码逐字节一致 |
| Pi RPC 命令契约 | PASS | 适配器使用的 13 个 RPC 命令均存在于包内 Pi `rpc-types.ts` |
| Pi 事件契约 | PASS | 映射的 20 类 lifecycle/stream/tool/queue/compaction/retry/UI 事件均可在 Pi 源码中定位 |
| Extension UI | PASS | Pi RPC 暴露的 9 个方法均有原生 Grok UI 路由 |
| Mock JSONL RPC | PASS | 27 条交互覆盖 bootstrap、history、commands、stream、tool、UI response 与 `agent_settled` |
| Rust tree-sitter 解析 | BLOCKED | `verify.sh` 未声明并预检 `tree_sitter` / `tree_sitter_rust` 依赖，当前环境缺失该模块 |
| Shell 脚本语法 | PASS | `build.sh`、`run-local.sh`、`run-installed.sh`、`verify.sh` 通过 `bash -n` |
| 补丁可应用性 | PASS | 对上传的原始 Grok 树执行 `patch --dry-run -p1`，29 个源码/manifest 文件全部可应用 |
| `cargo check` | PASS | `cargo check -p xai-grok-pager-bin --bin grok-pi` 成功；仅 adapter 中 1 条既有 dead-code warning |
| Adapter Rust 单元测试 | PASS | `cargo test -p pi-grok-adapter`：17 项通过 |
| `grok-pi` binary 单元测试 | PASS | `cargo test -p xai-grok-pager-bin --bin grok-pi`：1 项通过 |
| Pager focused lib tests | BLOCKED | 依赖 `xai-grok-pager-render` 的 `#[cfg(test)]` helper；测试依赖未启用 test-support feature，编译报错 |
| 本地 Pi npm build | PASS | `npm run build` 已在 Node.js `v24.15.0` 环境成功执行 |

机器可读报告：

- `crates/codegen/pi-grok-adapter/docs/native-grok-verification.json`
- `crates/codegen/pi-grok-adapter/docs/mock-pi-contract.json`
- `crates/codegen/pi-grok-adapter/docs/rust-syntax-verification.json`
- `verification-logs/cargo-status.json`
- `verification-logs/environment-status.json`
- `verification-logs/patch-status.json`

## 关键架构证据

### 生产 Grok Pager 入口

`crates/codegen/xai-grok-pager-bin/src/bin/grok-pi.rs` 只执行组合工作：

1. 启动 `pi --mode rpc`；
2. 将 Pi JSONL RPC 转为 ACP；
3. 构造 `AcpConnection::external`；
4. 调用 `xai_grok_pager::app::run_external`。

该文件不创建 Ratatui `Terminal`、`Frame` 或 Widget，也不读取 Crossterm input。

### 原生组件复用

`run_external` 继续使用 Grok 的：

- terminal init/restore 与 writer thread；
- production event loop；
- PromptWidget 与键盘输入；
- slash `CommandRegistry`、suggestion/dropdown；
- Markdown/code/diff/tool rendering；
- scrollback、find、copy、transcript、export；
- QuestionView；
- toast、sticky banner、terminal title；
- fullscreen、inline、minimal renderer。

### 修改边界

Grok 侧修改限制为：

- 增加 external ACP connection/profile；
- external backend 的产品功能 gate；
- Pi Extension UI 通知进入现有 Grok surface；
- QuestionView 增加 `initialText`/`noFreeform` 语义提示；
- 动态 Pi command 与被允许的 Grok builtin 合并；
- `/compact <instructions>` 参数透传；
- `grok-pi` composition binary。

Renderer、input engine、Markdown engine、tool renderer 和 minimal renderer 本体没有被重写。

## 在具备工具链的机器上必须执行

要求：

- Rust toolchain `1.92.0`（见 `rust-toolchain.toml`）；
- Node.js `>=22.19.0`；
- npm；
- 可安装 workspace 依赖。

执行：

```bash
./verify.sh
```

或者逐项执行：

```bash
cd grok-build-main
./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin grok-pi
./scripts/cargo-shared.sh test -p pi-grok-adapter
./scripts/cargo-shared.sh test -p xai-grok-pager --lib \
  external_builtin_filter_accepts_aliases_and_omits_product_commands
./scripts/cargo-shared.sh test -p xai-grok-pager --lib \
  slash_compact_with_context_enqueues_command
```

再构建完整运行链路：

```bash
cd ..
./build.sh
./run-local.sh /path/to/project --no-session
```

## 运行验收清单

构建成功后，至少手工验证：

1. 画面、PromptWidget、命令下拉、Markdown 和 tool cards 与 Grok Build Pager 一致；
2. `/help` 只显示允许的 Grok 本地命令，并合并 Pi 动态命令；
3. Pi extension `notify`/`setStatus` 不再生成 fallback 文本消息；
4. `select`、`confirm`、`input`、`editor` 使用 Grok QuestionView；
5. `/model` 与 `/effort` 实际修改 Pi model/thinking level；
6. active turn 中普通提交进入 Pi follow-up，send-now 进入 steer；
7. `!command` 使用 Pi `bash` RPC并渲染为 Grok tool card；
8. `/new`、`/compact instructions`、`/rename` 生效；
9. 重启已有 Pi session 时历史、reasoning、图片和工具结果恢复；
10. minimal/fullscreen 通过启动参数选择，终端退出后正确恢复。

## Upstream sync record (98c3b24)

日期：2026-07-17  
分支：`sync/upstream-98c3b24`（尚未 merge 回 `main`）

| 项 | 结果 |
|---|---|
| 上游 tip | `98c3b24`（含 `8adf901`） |
| 策略 | 有共同祖先 `c68e39f` 的 Git merge + 接缝修复，**非**直接在 main 碰运气 merge |
| `pi-grok-adapter` tests | PASS（46） |
| `grok-pi` cargo check | PASS |
| `grok-pi` unit tests | 4/5 PASS；1 项 `--append-system-prompt` 命名漂移为 main 既有失败 |
| 架构不变量 | adapter headless；Pager 唯一 TUI；Pi 唯一 core |

已知仍独立的基础设施 blocker 见上文 `verify.sh` / Pager focused lib tests 段落。
