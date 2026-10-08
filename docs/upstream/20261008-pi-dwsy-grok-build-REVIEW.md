# Pi / Dwsy / Grok Build 增量审阅 — 2026-10-08

审阅结论：Dwsy 有 **2 个值得改造后移植的提交**；Pi 需要补齐工具策略、取消语义、耗时和 Remote TUI 光标适配，另有可选终端状态与价格展示；Grok Build **没有新增提交**。最初调查只 fetch、审阅与写记录；随后用户授权引入，已完成相应开发验证，并在本次审查后分批本地提交。实际采用 SHA 见 [REVIEWED.json](REVIEWED.json)，实现和未验证边界见 [PLAN](../issues/架构/20261008-reference-adaptation-PLAN.md) 与 [审查提交记录](../issues/架构/20261008-working-tree-review-COMMITS.md)。未 push 或重发公开 Release。

## 固定范围与本地基准

- 项目：`/Users/kyros/WorkStation/grok-pi-tui`，branch `main`，HEAD `646abebbd80f8856a1baa05cbd39a0bcec81cff5`。
- 对照的是当前 working tree：已有四组裁撤与可选 Durable 实现，不能按 HEAD 的旧 Eval 分支判断。已发布 v0.1.10 与本地开发树不等价。
- 当前系统 `pi --version` 实测 **1.1.0**；`runtime/pi-durable-host/package.json` 的 chord、pi-ai、pi-coding-agent、pi-durable 四包仍锁 **1.0.4**。本轮没有改动它们。
- 上次 [Durable SOURCE](../issues/架构/20261007-pi-durable-integration-SOURCE.md) 固定的是 `b2363841a525bec5bdfcf4361fa7a1730076f5c5`，它当时的 package.json 写 1.0.4，但已含未发布的 1.1.0 改动；不能据此认为 npm SDK 1.0.4 具有全部能力。
- 所以首次综合审阅以 **正式 v1.0.4 tag** 为 Pi 起点：38 个提交到旧 SOURCE，另 17 个到本轮远端 main，共 55 个。最后的硬件光标提交在 1.1.0 发布之后，未声称系统已安装它。
- Dwsy 以与本仓库的共同祖先开始；这也不是此前已建立的专用 Dwsy 审阅水位。此次建立 [三仓库 checkpoint](REVIEWED.json)，以后从 reviewed_sha 增量检查。
- Grok Build 依照仓库 [review 技能](../../.pi/skills/grok-build-review/SKILL.md)，从自己的 REVIEWED 开始，不用产品 merge-base。

| 仓库 / 远端分支 | 起点（不含） | 本次终点（含） | 提交数 |
|---|---|---|---:|
| [earendil-works/pi main](https://github.com/earendil-works/pi) | `7c10bd4337495ee613f2224843ecdf349b80d1df`（v1.0.4） | `1cedd32724abfcb0915f76cc61b6827e2c16dbad` | 55 |
| [Dwsy/grok-pi-tui main](https://github.com/Dwsy/grok-pi-tui) | `222d614d9f12cc8fcd408d59419c7d4d197d1be3`（共同祖先） | `fbaba29a9510254a4097b69f8014dd6de4b10ba4` | 4 |
| [xai-org/grok-build main](https://github.com/xai-org/grok-build) | `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`（REVIEWED） | `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8` | 0 |

Pi v1.1.0 发布 commit：`abe508e1b89912adde45528136c3221eb69acdd7`。Grok Build 对应 Source-Revision：`559751fdcec02d413e4c57c8832ab275e4f44980`。所有 SHA 均为 fetch 后的本地 Git 对象；范围已检查祖先关系和数量。审阅完成不表示移植完成，也不表示 SDK 已升级。

## 建议执行顺序

### P1：先补当前行为与升级兼容

1. **Pi signed tool list。** [ddaa0a0341a8](https://github.com/earendil-works/pi/commit/ddaa0a0341a84b073a087a3d89b9b9e7fbdaf6ba) 允许 `--tools +codemode` / `-bash` 修改默认选择。当前 CLI 透传正确，但 [tools_extension.rs](../../crates/codegen/xai-grok-pager-bin/src/bin/grok_pi/tools_extension.rs) 的 `csv_contains` / `codemode_requested` / `tool_name_allowed_by_cli` 按纯名称 allowlist 判断：`+codemode` 不匹配 `codemode`，且单独 `+codemode` 会把增强 Bash 桥判断为不允许。若 MCP 开启可能另行加载 Codemode，不能据此认为策略已经正确。应按官方 modifiers 语义处理本地桥接 admission，继续让 Pi 决定最终工具集合，并回归 +/-、显式 allowlist、exclude、no-tools、reload/F2 边界。
2. **消费取消标记。** [503c605528f9](https://github.com/earendil-works/pi/commit/503c605528f9af993c0e37ede468cf884fb0ff5b) 给 `agent_settled` 增加 `aborted`。当前 [events.rs](../../crates/codegen/pi-grok-adapter/src/pi_adapter/events.rs) 忽略该字段，按 EndTurn 完成，再尝试 queue/Goal continuation。ACP Esc 已有 Cancelled 路径，因此这里不能写成“所有 Esc 都坏了”；缺口主要是从 Pi/扩展侧发起的 abort 与 settled 之后的续跑判断。兼容旧 Pi 缺字段，按官方取消语义处理完成原因及续跑边界。
3. **移植 Dwsy Code Mode/耗时三项修复，同时接入 Pi 正式耗时。** [9a00b24e451a](https://github.com/Dwsy/grok-pi-tui/commit/9a00b24e451ad3a22d688f7dd80711818e157261) 加 Codemode 全屏入口、原生 viewer、回放耗时冻结、小数 nested durationMs。当前 `has_normal_fullscreen_viewer` 没有 Codemode，dispatch/viewer 也无 `for_codemode`；nested duration 仍用 `as_u64`。取这些现有原生组件的小 patch，不恢复 Eval 执行能力。
   - Pi [36a686ee8dd7](https://github.com/earendil-works/pi/commit/36a686ee8dd73afe8242011db387040890e24093) 已记录顶层 toolResult/assistant `durationMs`，Durable task `startedAt/endedAt`。当前 classic history DTO 没保留顶层 duration，Durable update 只有 isReplay，没有 agentTimestampMs。
   - 优先显示官方 `durationMs`（本次 execute/response 耗时），旧数据再用有效时间戳回退。Dwsy 的起止 timestamp 差是墙钟跨度，不一定等于 execute 时间；Durable task span 还包含等待/恢复，不能混为同一种耗时。缺字段不能编造 0ms。
   - 目标：[model.rs](../../crates/codegen/pi-grok-adapter/src/model.rs)、[classic tools/replay](../../crates/codegen/pi-grok-adapter/src/pi_adapter/tools.rs)、[Durable adapter](../../crates/codegen/pi-grok-adapter/src/durable.rs)、[native tracker](../../crates/codegen/xai-grok-pager/src/acp/tracker.rs)、`acp/meta.rs`、`scrollback/blocks/tool/{codemode.rs,mod.rs}`、`views/block_viewer` 与 `app/dispatch/transcript.rs`。
4. **下一次 Pi 升级前补 Remote TUI 新 cursor marker。** [1cedd32724ab](https://github.com/earendil-works/pi/commit/1cedd32724abfcb0915f76cc61b6827e2c16dbad) 的 Input/Editor 不再直接生成 reverse-video，而是输出 `pi:fc` / `pi:/fc`，由 Pi TuiBase 解析。当前 [host.ts](../../extensions/pi-grok-remote-tui/host.ts) 直接 `component.render()`，只清旧 `CURSOR_MARKER`；它不实例化 TuiBase，因此新解析步骤不会执行。现有 [native mapper](../../crates/codegen/xai-grok-pager/src/views/agent_status.rs) 只把旧 SGR cursor 转成主题高亮。
   - 源码契约 probe 确认新 fake markers 仍留在 frame；尚未做最新 Pi 构建的真人/PTY 重现，所以只断言未解析，不把具体乱码样式写成已实测。
   - 最小适配是在 Remote TUI 投影处把新标记转换成现有原生高亮语义并清理控制标记；普通 Rust composer 无需换成 Pi Editor。回归第一/末列、空输入框、overlay、失焦与旧 SDK。不要引入第二个终端宿主，也不要依赖未公开导出的常量或 protected 方法。

### P2：随后补原生体验

- **Dwsy trace hint**：[c6a4ff4cbf91](https://github.com/Dwsy/grok-pi-tui/commit/c6a4ff4cbf91d681ccb668d55fadfe21301f3523)。Collapse router 已能打开工具 trace，底部 hints 没说明。加入“选中折叠条目且含 tool_traces”条件，让非 vim 显示 ← trace、vim 显示 h trace；对齐当前 build_hints 签名，不照抄旧测试参数。
- **模型价格分档**：Pi [943a10e744b8](https://github.com/earendil-works/pi/commit/943a10e744b87d9a593f3dca7406d4671910f490) 保留 `cost.tiers`。本地 PiModel/ACP metadata 仅 input/output/cacheRead/cacheWrite，模型描述仍显示基础费率。把 tiers 保留用于选择器说明即可；实际费用继续读 Pi usage.cost，不重做定价引擎。Haiku 的模型/effort 无需硬编码，本地已按 thinkingLevelMap 支持 xhigh/max。
- **OSC 7501 Program Status**：Pi 的实现位于 TS ProcessTerminal/interactive reporter，RPC 不运行它；Rust 没有 7501。需要时在现有原生 terminal probe 与 lifecycle 添加 working/blocked/done/error/idle（含取消/退出清理），复用终端探测、去重并避免上报 prompt/回答内容。属可选体验，不是 Pi 1.1 运行前置条件。
- **OpenAI 登录品牌名**：[9ad083102aa9](https://github.com/earendil-works/pi/commit/9ad083102aa9afd483dbb5e0da53e69f2902c6a6)。官方 ModelRuntime.login 的第四个 options 可传 agentName；当前 native auth 只传前三项。可用 grok-pi 名称，仍委托官方 OAuth。

### Durable：通过 SDK 升级获得核心改进

官方 [v1.1.0 release](https://github.com/earendil-works/pi/commit/abe508e1b89912adde45528136c3221eb69acdd7) 汇总 [Durable changelog](https://github.com/earendil-works/pi/blob/abe508e1b89912adde45528136c3221eb69acdd7/packages/durable/CHANGELOG.md)。四个 Pi SDK 包应一起升级，重新锁依赖和打包；本轮未执行升级或 store 迁移。

| 官方新能力/变更 | pig 应做什么 |
|---|---|
| context 增量扫描、跨 task 缓存、retention 异常释放、system prompt/cache 顺序 | 升级 SDK 直接获得；不在 Rust/host 重写缓存、transcript 排序或 scheduler。使用官方 contextRetentionMs（默认 10 分钟）；只有测到内存问题时再暴露配置。 |
| ToolExecutionApi.models / HookApi.models | 后续模型工具用官方 api.models，当前不另造 ModelRuntime。 |
| Conversation.context(context, {at}) | 后续 context/tree UI 使用官方 cutoff 读取，不直接查询内部 SQLite 表或修改 JSONL。 |
| TaskRuntime.context 的 at 改为 options 对象 | 当前 host 无该调用；升级兼容测试覆盖将来使用它的 extension/tool。 |
| ScanOrder / cursor 顺序 | 使用官方 Node SQLite，未实现自定义 Storage；现有默认顺序保留。若做“最新任务优先”分页，使用 order=descending 与同方向 cursor，不在本地事后反转页。 |
| task startedAt/endedAt、tool/assistant durationMs | adapter/native 卡片需要映射，不能只升级包；旧 store 中字段可缺失。 |
| Cloudflare Durable Object SQLite | 当前 macOS/本地模式不需要；不增加 Cloudflare 后端。 |

升级验收应覆盖现有 SDK fixtures、旧 SQLite store 重开/恢复、队列取消、前台 subagent、context/分页边界、原生耗时与打包产物。源码审阅没有证明 1.1.0 运行兼容。普通 Pi extensions、MCP、Codemode/images 等 Durable 接入仍按既有 SPEC 的未完成边界处理；此次发布不会自动补齐。

## 无需在 pig 复制的核心改动

系统 Pi 1.1.0 已包含正式发布范围内的 provider/retry、模型 catalog、MCP manager/OAuth lifecycle、图像 worker、Codemode 描述和输出分隔等改动。当前 composition 显式加载 `builtin:mcp` / `builtin:tool-search` / `builtin:codemode`，所以继续使用官方实现。配置是否启用、真实 provider/OAuth 是否通过以及所有插件兼容，未由版本号或源码检查证明。

- 新 classifier（llama.cpp / OpenAI Decisions / images）供官方 models.classify/Code Mode 使用；本地 parse_model 排除 classifier 是聊天模型菜单的预期行为，无需增加“分类器模式”。
- Codemode text/console 内容已由官方 formatter 分隔；现有 tool_projection 仅去掉脚本头并保留正文，不额外重排。仍应回归多 text、console、image/error 的 live/replay 输出。
- ANSI 修复针对官方用户 Bash executor；增强 Bash 工具/后台任务原始日志是另一条路径，不能只凭 SDK 升级声称后台输出也全部修复。
- Termux clipboard、Pi TS fullscreen selection/outputPad/Herdr terminal detection、Pi Nix/发布脚本不直接用于 Rust Pager。需要对应平台体验时在现有 native 入口评估，不把 TS TUI 搬进产品。
- npm audit 提交主要改变 dev 工具（45 个节点）；另有 prod sandbox-runtime 依赖 shell-quote 1.10→1.12。当前 Durable lock 未含 shell-quote，不能把上游 root lock 直接复制过来，也不据此声称所有依赖已无风险。

## Pi 55 个提交逐项决定

表中 **SDK-owned** 表示留给官方核心：系统 Pi 通过升级获得，Durable 需升级锁定包。**Adapt then port** 是待做建议，尚无 port commit；**Observe** 是记录后不移植或按后续需求再评估。变更说明以 commit body、路径与相关 diff 归纳，非从 release notes 反推新增次数。

| commit SHA（链接为完整 SHA） | 变更 | 决定与理由 | 当前目标 |
|---|---|---|---|
| [28dcce2ba45c](https://github.com/earendil-works/pi/commit/28dcce2ba45ce4a9efeb0f5b686f0be830fd89b9) | Unreleased 文档 | Observe — 版本记录，无产品代码适配 | `none` |
| [9f013cf59c1b](https://github.com/earendil-works/pi/commit/9f013cf59c1b32658a496e762a56c581faad21c2) | managed install 清理 | SDK-owned — Pi 自己清理受管理安装；pig 的 Rust 安装产物不属于该目录 | `system Pi` |
| [f6127a1bf907](https://github.com/earendil-works/pi/commit/f6127a1bf907dec9b77be60e6a46db4b9668f081) | llama.cpp classifier | SDK-owned — 分类模型由 Pi/Code Mode 使用；当前聊天模型选择器排除 classifier 是正确边界 | `model.rs / Pi SDK` |
| [9ad083102aa9](https://github.com/earendil-works/pi/commit/9ad083102aa9afd483dbb5e0da53e69f2902c6a6) | OpenAI 登录应用名称 | Adapt then port · P2 — 可给官方 LoginOptions.agentName 传 grok-pi；当前 native auth 未传第四个 options 参数 | `extensions/pi-grok-auth/login.ts, shared.ts` |
| [1ffb6bd621c2](https://github.com/earendil-works/pi/commit/1ffb6bd621c29d7c8e38bb3b3c1cdcbda6b6ea90) | .env 自动载入关闭 | SDK-owned — 修改 Pi Bun standalone 的构建；pig 使用 Rust 二进制与系统 Pi，不复制 Bun 构建脚本 | `system Pi` |
| [0cf65d2bf7ca](https://github.com/earendil-works/pi/commit/0cf65d2bf7cad7a58437a6c8a9172f29dbc43ea7) | Codex caller headers 优先级 | SDK-owned — 官方 provider 负责合并 headers；不在 Rust 重写请求 | `system Pi / Durable SDK` |
| [428a12bc7751](https://github.com/earendil-works/pi/commit/428a12bc775145afa342530a9eaa652efb3e4422) | 贡献者审批名单 | Observe — 上游协作元数据，无产品功能 | `none` |
| [23cf2b948dc6](https://github.com/earendil-works/pi/commit/23cf2b948dc626c234f81d6fee7d9978b4c43631) | 包产物验证统一 | Observe — 属于 Pi npm/Bun/pnpm 发布流程；可参考消费端 smoke 思路，不复制发布体系 | `scripts/package-macos.py / runtime/pi-durable-host/test` |
| [ddaa0a0341a8](https://github.com/earendil-works/pi/commit/ddaa0a0341a84b073a087a3d89b9b9e7fbdaf6ba) | --tools +name/-name | Adapt then port · P1 — CLI 已透传；host 的 csv_contains / codemode_requested / tool_name_allowed_by_cli 仍按旧 allowlist 判断 | `grok_pi/tools_extension.rs, cli.rs` |
| [56b25ff4ebbd](https://github.com/earendil-works/pi/commit/56b25ff4ebbd8a119ef9185447c7b2416059dbcb) | message types 文档 | Observe — 协议说明校正；按当前实际事件与未知字段兼容性核对 | `model.rs / pi_adapter/events.rs` |
| [8b5708dbb1b4](https://github.com/earendil-works/pi/commit/8b5708dbb1b4a819f924b43a2de02c9cc8c7a46d) | server-busy retry | SDK-owned — Pi 的 retry classifier 修复，直接使用核心 | `system Pi / Durable SDK` |
| [83c9e2645378](https://github.com/earendil-works/pi/commit/83c9e26453782cdea5ea838b46744a2e4cff70bc) | fullscreen selection 重建清理 | Observe — 修改 Pi TS fullscreen 的 selection；pig 使用独立原生 selection，不直接搬入 TS 状态 | `app/agent_view/session.rs, selection.rs` |
| [68ccef17608c](https://github.com/earendil-works/pi/commit/68ccef17608c5964bc91d1c567b9d1fe5b0a590a) | Durable context range 重用 | SDK-owned · upgrade — 通过 SDK 升级获得；与 da866ada 的后续缓存实现一起使用，不复制 context 派生 | `runtime/pi-durable-host/package.json, package-lock.json` |
| [b0114ef5fabe](https://github.com/earendil-works/pi/commit/b0114ef5fabe1534b2b2f2ff5eba3604b5aa8fb0) | Durable Tool/Hook models API | SDK-owned · observe — 后续适配模型类工具可用 api.models；当前工具无需重写 ModelRuntime | `runtime/pi-durable-host/core.mjs, subagent.mjs` |
| [76f6c06dafe3](https://github.com/earendil-works/pi/commit/76f6c06dafe3f8b1e3e8b76c903b2edc90715df1) | Durable earlier-entry context | Observe — 为将来的原生 context/tree 面板提供官方读取方式；不因此增加自有历史存储 | `runtime/pi-durable-host/host.mjs / durable.rs` |
| [18336987add9](https://github.com/earendil-works/pi/commit/18336987add9a3966f338d7c6617e782b58cd91f) | Pi transcript outputPad | Observe — 影响 TS renderer 与 HTML exporter；普通原生工具卡片不由 Pi renderer 绘制，Remote TUI 组件使用 SDK 行输出 | `extensions/pi-grok-remote-tui/host.ts / native cards` |
| [27075fe07597](https://github.com/earendil-works/pi/commit/27075fe07597fbb9ada7a0b2e3ea9b9b34505cc0) | 上下文 token 估算 | SDK-owned — provider 请求边界由 Pi 估算；Rust 继续消费 Pi usage | `system Pi / Durable SDK` |
| [269121616c52](https://github.com/earendil-works/pi/commit/269121616c520a6a9aa9b5da29f1b233ccbed10c) | Codemode lookup async 描述 | SDK-owned — builtin:codemode 的官方工具描述直接继承，不保留一份自有描述 | `system Pi` |
| [311f0e020dfc](https://github.com/earendil-works/pi/commit/311f0e020dfcb1179e17ba4afd353dbc23eadb7a) | 估算测试与 import 排序 | Observe — 测试期望跟随官方估算；Durable 仅 import 排序，非新 API | `none` |
| [2989eb581f95](https://github.com/earendil-works/pi/commit/2989eb581f95627618d40387ac527dc3ba061b1c) | Bedrock OpenAI thinking effort | SDK-owned — 原生努力等级已按 Pi metadata 映射；发送 Bedrock 字段由 SDK 负责 | `model.rs / Pi SDK` |
| [636703a0a4f2](https://github.com/earendil-works/pi/commit/636703a0a4f2f4d8558d08f2308cb41109585bf5) | TaskRuntime.context options | SDK-owned · compatibility — 升级注意 at 参数改为 {at}；当前 host 不调用 TaskRuntime.context，也不实现自定义 Storage | `runtime/pi-durable-host/*.mjs` |
| [36a686ee8dd7](https://github.com/earendil-works/pi/commit/36a686ee8dd73afe8242011db387040890e24093) | response/tool/task 正式耗时 | Adapt then port · P1 — Rust 丢掉顶层 durationMs；Durable ACP 缺 agentTimestampMs。应优先正式 execute 耗时，再按有证据的时间戳回退 | `model.rs, pi_adapter/tools.rs, replay.rs, durable.rs, acp/meta.rs, tracker.rs` |
| [43d37639912f](https://github.com/earendil-works/pi/commit/43d37639912fe15c0926ca5bc7965822ced5615c) | Radius gateway catalog 权威 | SDK-owned — 组织禁用模型不应被本地 catalog 重新加回；继续使用官方 ModelRuntime | `system Pi / Durable SDK` |
| [4dd2af42c8e3](https://github.com/earendil-works/pi/commit/4dd2af42c8e34a0cea015d17bbe1b97edda3b8bf) | Durable 双向分页 | SDK-owned · compatibility — 当前采用官方 SQLite，默认顺序不变；以后加分页用 query.order 且同一 cursor 不切方向 | `runtime/pi-durable-host/core.mjs, host.mjs` |
| [3ba22ce17aba](https://github.com/earendil-works/pi/commit/3ba22ce17abaabd1f49f35d9f18121ea4622af44) | response stream assignability | SDK-owned — 过渡修复由下个 f284a246 完成；不照抄过渡 WeakMap | `Pi SDK` |
| [f284a2460cce](https://github.com/earendil-works/pi/commit/f284a2460cce76f9e0ac466f01bdae97294c421f) | response stream 正式实现 | SDK-owned — 使用官方 event stream；本地 fixture 升级时检查 mocks，不重写 SDK stream | `runtime/pi-durable-host/test/sdk.test.mjs` |
| [92216fa15c18](https://github.com/earendil-works/pi/commit/92216fa15c18473d75987f2a6af4d108e6ebec73) | Durable system prompt/cache 顺序 | SDK-owned · upgrade — SDK 修正上下文派生与 provider cache；不得在 adapter 重排 SQLite transcript | `runtime/pi-durable-host/package.json, package-lock.json` |
| [4c28a6865c42](https://github.com/earendil-works/pi/commit/4c28a6865c4230f61c75e2cbc3d17f2d75a8a81c) | Durable entry graph budget | Observe — 上游源码依赖检查脚本阈值；不是 runtime 能力 | `none` |
| [fe11328b0d31](https://github.com/earendil-works/pi/commit/fe11328b0d31d8666c8e9b662c00097291132483) | faux prompt-cache 估算优化 | SDK-owned — 测试 provider 内部优化；真实 provider 无 Rust 适配 | `Pi SDK` |
| [da866ada17bc](https://github.com/earendil-works/pi/commit/da866ada17bc78c81855d091ab06ab8fb3b6e16e) | Durable context cache / CF SQLite | SDK-owned · upgrade / skip CF — 官方缓存可降低重复扫描；macOS 使用 Node SQLite，无需 Cloudflare adapter。保留官方 retention 设置 | `runtime/pi-durable-host/package.json, package-lock.json, core.mjs` |
| [ae92585d3b3e](https://github.com/earendil-works/pi/commit/ae92585d3b3e5f1e4b123d14a34314d826d8d9f5) | Durable retention 失败处理 | SDK-owned · upgrade — 缓存 settings 异常由官方 scheduler 上报并释放；不加第二个计时器 | `runtime/pi-durable-host/package.json, package-lock.json` |
| [eb326d265ae0](https://github.com/earendil-works/pi/commit/eb326d265ae0b88489a6d10319307780df827cdf) | Codemode text/console 分离 | SDK-owned · regression — 返回内容已有 text N/M 和 console_output 分隔；现有 projection 仅去脚本头并保留正文，暂不加第二套 formatter | `tool_projection.rs / native Codemode card` |
| [ce8972a0e99c](https://github.com/earendil-works/pi/commit/ce8972a0e99cab79f2620a594a1c17baef436c67) | OpenAI Decisions / classifier images | SDK-owned — 由 models.classify 及 Codemode 使用；classifier 不应加入生成模型菜单。Durable 普通 Codemode 仍未适配 | `model.rs / system Pi` |
| [6b585445408f](https://github.com/earendil-works/pi/commit/6b585445408fc862084390442f698cc4c74419ce) | 空 classifier catalog 类型 | SDK-owned — 上游类型修复，不扩大聊天模型菜单 | `Pi SDK` |
| [2db5e359bf84](https://github.com/earendil-works/pi/commit/2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b) | MCP manager responsive | SDK-owned · regression — pig 加载 builtin:mcp；连接状态和后台动作由官方扩展管理 | `grok-pi.rs / extensions/pi-grok-remote-tui/host.ts` |
| [8d8ae2fc247b](https://github.com/earendil-works/pi/commit/8d8ae2fc247bc2c2fd8157e6cca3929316519893) | Anthropic OAuth 空闲端口 | SDK-owned — native auth 委托 ModelRuntime；保留官方 redirect URI、fallback 和取消信号 | `extensions/pi-grok-auth/login.ts / Pi SDK` |
| [ea6fa125ab75](https://github.com/earendil-works/pi/commit/ea6fa125ab7590d583aa7c7f7293ed44114403e4) | Nix catalog pin | Observe — pig 的 macOS 发布不使用 Pi Nix catalog | `none` |
| [b2363841a525](https://github.com/earendil-works/pi/commit/b2363841a525bec5bdfcf4361fa7a1730076f5c5) | Herdr OSC 8 / image detection | Observe — 是 Pi TS terminal 检测；pig 需按原生 probe 适配，待需要支持 Herdr 时处理，不能假称 Rust 已继承 | `xai-grok-pager-render/src/terminal` |
| [7fb59f995b0a](https://github.com/earendil-works/pi/commit/7fb59f995b0a1db552001a8577b234e4105d7179) | Mistral finish_reason error retry | SDK-owned — 瞬时 provider 错误由 SDK retry，已安装系统 Pi 1.1.0 包含该发布范围 | `system Pi / Durable SDK` |
| [b30a6dd77934](https://github.com/earendil-works/pi/commit/b30a6dd779340f7bc2f3ffa60f4c0a5f914ba9ae) | image worker 消息过滤 | SDK-owned — Node watch 消息导致丢图由 Pi resize worker 修复；Rust clipboard/image UI 为独立路径 | `system Pi` |
| [adae8246453a](https://github.com/earendil-works/pi/commit/adae8246453a2928a268ccecb9fb55125d96d0af) | Azure/Foundry provider 文档 | Observe — 官方配置说明；不增加自有 provider 逻辑 | `none` |
| [592fb57b70e3](https://github.com/earendil-works/pi/commit/592fb57b70e34a7f1011f3d7f20b556dce17c321) | Termux Android clipboard | Observe — 本次 macOS 产品范围不需要 Android 分支；Pi TS clipboard 也不是 Rust clipboard | `none` |
| [27c7b6ff48cc](https://github.com/earendil-works/pi/commit/27c7b6ff48ccca57694a960e4af10529f77deae6) | 用户 Bash 跨 chunk ANSI | SDK-owned · regression — 修复 Pi executeBashWithOperations。增强 Bash 的后台原始日志不是该 executor，须分别回归，不声称全部自动继承 | `system Pi / extensions/pi-grok-bash/bash-tasks.ts` |
| [f10993bc7f28](https://github.com/earendil-works/pi/commit/f10993bc7f28145df1375f3ff39c7f5c4cfc05f0) | MCP OAuth cancel / close cleanup | SDK-owned · regression — builtin:mcp 用官方 AbortSignal；Remote TUI 分发组件键盘，仍需真实 OAuth/关闭回归，不新建 MCP client | `grok-pi.rs / extensions/pi-grok-remote-tui/host.ts` |
| [503c605528f9](https://github.com/earendil-works/pi/commit/503c605528f9af993c0e37ede468cf884fb0ff5b) | OSC 7501 / settled.aborted | Adapt then port · P1/P2 — P1 读取 aborted；P2 原生终端可选支持 OSC 7501。TS interactive reporter 不会在 RPC 中替 Rust 输出 | `pi_adapter/events.rs / pager terminal lifecycle` |
| [f76c1db66271](https://github.com/earendil-works/pi/commit/f76c1db66271570d763d1c17b6a467f20a58d7b8) | Haiku 5.5 / adaptive effort | SDK-owned · regression — 当前从 Pi 获取模型与 thinkingLevelMap，已支持 xhigh/max；价格分档显示单独补，不硬编码模型 | `model.rs / pi_adapter.rs` |
| [943a10e744b8](https://github.com/earendil-works/pi/commit/943a10e744b87d9a593f3dca7406d4671910f490) | catalog prompt-length tiers | Adapt then port · P2 — PiModel 与 ACP metadata 只保留基础四项费率；需保留 cost.tiers 供显示，usage.cost 继续由 Pi 计算 | `model.rs, pi_adapter.rs / model picker` |
| [dce4ae6f79d4](https://github.com/earendil-works/pi/commit/dce4ae6f79d4fb40489d639e239eefac9f31255f) | 1.1.0 changelog 核对 | Observe — 发布记录；避免把 release notes 重复算为功能提交 | `none` |
| [a2eef9eb60fb](https://github.com/earendil-works/pi/commit/a2eef9eb60fb44bf399532151b43dab83c0a9c88) | Kimi K3 cache-write pricing | SDK-owned — 使用 Pi 官方 catalog 和 usage cost，不复制 Rust 价格表 | `system Pi / Durable SDK` |
| [70759f48bed0](https://github.com/earendil-works/pi/commit/70759f48bed0c0b30b089b9094f4080cbc3148c5) | npm audit dependencies | SDK-owned · observe — 45 个 dev 节点变化，另有 sandbox-runtime 使用的 shell-quote 1.10→1.12；当前 Durable lock 没有 shell-quote，不复制上游 root lock | `runtime/pi-durable-host/package-lock.json / Pi SDK` |
| [e9163107058b](https://github.com/earendil-works/pi/commit/e9163107058b60b091617c98c5d6d7e58480b966) | 测试去固定 sleep | Observe — 上游测试稳定性；本轮审阅不增加无关测试 | `none` |
| [bf8d9c659f0a](https://github.com/earendil-works/pi/commit/bf8d9c659f0af76caf41205cee92d25aac64b389) | release skip-tests 开关 | Observe — 不复制跳过测试的发布开关，保持本项目验证要求 | `none` |
| [abe508e1b899](https://github.com/earendil-works/pi/commit/abe508e1b89912adde45528136c3221eb69acdd7) | Release v1.1.0 | SDK-owned · upgrade — Durable 的四个 Pi 包仍锁 1.0.4；成组升级并验证 SDK API、旧 store 恢复和打包，不只改版本字符串 | `runtime/pi-durable-host/package.json, package-lock.json / build packaging` |
| [75a99721d102](https://github.com/earendil-works/pi/commit/75a99721d102d1948dcdd08ba200fb7caef81c1d) | 下一轮 Unreleased | Observe — 发布周期文档，无功能变更 | `none` |
| [1cedd32724ab](https://github.com/earendil-works/pi/commit/1cedd32724abfcb0915f76cc61b6827e2c16dbad) | hardware cursor / fake markers | Adapt then port · P1 before next Pi upgrade — Remote TUI 直接 component.render()，只清旧 CURSOR_MARKER；未执行新 resolveFakeCursors，需把新标记转成现有原生 cursor 样式 | `extensions/pi-grok-remote-tui/host.ts, index.test.ts / views/agent_status.rs` |

## Dwsy 4 个提交逐项决定

| commit SHA（链接为完整 SHA） | 变更 | 决定与理由 | 当前目标 |
|---|---|---|---|
| [9a00b24e451a](https://github.com/Dwsy/grok-pi-tui/commit/9a00b24e451ad3a22d688f7dd80711818e157261) | Code Mode 全屏、回放耗时、小数耗时 | Adapt then port · P1 — 三个修复当前均缺失；按现有四组裁撤后的代码取小 patch，并结合 Pi 1.1 durationMs，而非整仓合并 | `scrollback/block.rs, blocks/tool/{codemode.rs,mod.rs}, app/dispatch/transcript.rs, views/block_viewer/mod.rs, acp/tracker.rs` |
| [c6a4ff4cbf91](https://github.com/Dwsy/grok-pi-tui/commit/c6a4ff4cbf91d681ccb668d55fadfe21301f3523) | trace 快捷键提示 | Adapt then port · P2 — 原生 Collapse 已能开 trace，但 hints 未提示；只加入实际具备 trace 的折叠条目条件并对齐当前参数 | `app/agent_view/render.rs / views/agent.rs` |
| [1d20a198378c](https://github.com/Dwsy/grok-pi-tui/commit/1d20a198378cd328c1278d990c7c94dc729c5642) | Dwsy v0.1.10 发布日志 | Observe · skip — 本 fork 已在 10-05 发布 macOS arm64 v0.1.10；Dwsy 的 10-06/六平台发布描述不能覆盖本 fork | `CHANGELOG.MD / docs/CHANGELOG.zh-CN.md` |
| [fbaba29a9510](https://github.com/Dwsy/grok-pi-tui/commit/fbaba29a9510254a4097b69f8014dd6de4b10ba4) | 旧 Eval replay import | Observe · skip — 当前 replay 已移除该 Eval-only 分支和 import；不得为修旧分支恢复已删除 Eval runtime | `pi_adapter/replay.rs` |

`9a00b24` 与 `c6a4ff4` 应用时需要保留本地四组裁撤和 Durable 改动；旧 Eval render 可以继续作为历史只读兼容，但不是恢复自有 Eval runtime 的理由。发布日志与旧 Eval import 不选入。

## Grok Build：无新增提交

本次 fetch 后 `grok-build/main` 与 [REVIEWED](../grok-build/REVIEWED) 同为 `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`，`REVIEWED..main` 的提交数为 0。终点 Source-Revision 为 `559751fdcec02d413e4c57c8832ab275e4f44980`，commit 日期 `2026-09-29T16:57:09Z`。没有新的 Changes bullet 可分类，没有新的 TUI 优化可 pick；不移动已有 REVIEWED，不重复宣称移植历史 UI backports。此次空范围另记入 [Grok Build REVIEW_LOG](../grok-build/REVIEW_LOG.md)。

## 下次检查与验证边界

1. 从 [REVIEWED.json](REVIEWED.json) 读取每仓库 reviewed_sha 和 fetch_ref；本次 55/4/0 均为 review complete，已采用部分的本地 commit 登记在 implementation_commits。
2. fetch 相同 repo/分支，验证 reviewed_sha 是新 tip 的祖先，再按 `old..new` 逐提交审阅。若 history 被改写，保留旧水位，先记录分叉/重写情况，不静默换 merge-base。
3. 新的审阅检查“新增提交”和“已记录待做项”两部分；对照当前 adoption 和 capability，避免重复采用已落实的内容。新的移植继续登记本地 commit 与验证。
4. 原调查阶段验证：fetch 成功；Git 对象/祖先/范围数量；源码契约 probe（signed tool list、aborted、fake cursor、flat cost）；checkpoint 与表格覆盖、路径/链接与文档 diff/whitespace。保留已有 dirty 文件。
5. 原调查阶段没有 Cargo/build/PTY 或 SDK 安装。后续开发及本次审查的实际 SDK、构建、PTY 和 commit 回执另见 PLAN/VERIFICATION；仍未执行真人模型/OAuth验收或用户 SQLite迁移。

## 2026-10-08 已授权实施更新

审阅结束后，用户授权按建议引入。[实施 PLAN](../issues/架构/20261008-reference-adaptation-PLAN.md) 记录后续实现与验证；上面的 pending 表格保留为当时的审阅决定，不再代表当前实现状态。Dwsy 的 `9a00b24`/`c6a4ff4` 已按本地四组裁撤后的代码改造移植，旧 Eval import 和 fork 的发布记录继续跳过。本地采用 commit 为 `f21287bff`，其他分批 SHA见 REVIEWED.json；未 push。

Pi signed tools、aborted settle、官方执行耗时、Remote TUI 新 cursor、价格 tiers、登录 agentName 与 native OSC 7501 已落实。Durable 四个 SDK 包及随包 host 升级 1.1.0；实际旧 SDK 临时会话重开与稳定提交身份已验证。原始 opaque replay envelope 已保留 durationMs，因此实现复用其 raw_output，不增加一套历史 DTO；typed projection 继续保留该字段。

原生 PTY 进一步发现带图片的 Codemode 文本 viewer 被原有 graphics guard 阻挡：普通文本 viewer 现优先打开，graphics 检查仅约束 media-only fallback。该调整也复用于有图片引用的其他原生文本块。优化构建、最终包与本机安装证据以 PLAN/VERIFICATION 中最后记录为准。后续审阅仍从上面三个 reviewed_sha 开始，不能把移植状态当作新水位。
