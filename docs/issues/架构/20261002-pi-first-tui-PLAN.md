---
id: "2026-10-02-pi-first-tui-plan"
title: "Grok 原生 TUI 与 Pi 1.0 适配实施 Plan"
status: "in-progress"
created: "2026-10-02"
updated: "2026-10-02"
category: "architecture"
---

# 实施 Plan

依据：[SPEC](20261002-pi-first-tui-SPEC.md)。用户已授权本地实施，并在后续明确要求边做边本地 commit；主代理统一作阶段提交，不 push、创建 worktree 或写真实账号。历史日志中的“未 commit”只描述当时状态。

## 执行顺序

| 阶段 | 工作 | 验收与状态 |
|---|---|---|
| P0 | 修复 E0425；统一 Pi 1.0 基线；记录现状与准确 repo 路径；恢复相关测试入口 | 已完成构建基线：adapter 193 / bin 96 tests、check、build PASS；全量 verifier 的剩余 identity 失败归 P6 |
| P1 | 迁移 Eval v2 到官方嵌套执行；保留结果、并发、background 与 native task/replay 语义 | 待实施：生产 Eval 回归及真实 Pi 上下文 fixture；删除不再需要的私有 bridge 调用 |
| P2 | 接入 Pi MCP opt-in、callable/exposure/CLI 约束、Eval-only；完善 `clear_queue` 取消 | 待实施：本地 MCP RPC smoke、队列回归、嵌入扩展完整性测试 |
| P3 | Codemode 图片原生呈现；Pi 鉴权原生对话框与新登录语义；收敛核心 Remote TUI 耦合 | 待实施：live/replay 组件检查、无凭据交互/取消与 PTY fixture |
| P4 | config/manifest/UI DTO 和 Workflow 契约迁移；逐项隔离 stock runtime；用依赖图证明实际裁剪 | 待实施：每条依赖边完成后测量；保持发行包现有可选能力；Pi 与 stock 检查 |
| P5 | 记录 7 个上游 commit 的 Changes；按终端、输入/剪贴板、图像、minimal/Markdown 等 TUI 功能组选择性吸收 | 待实施：记录来源/取舍/依赖；不引入 Grok backend 语义；不更新完整 base 为部分导入 |
| P6 | 修复 verifier 的递归模块/实际 Pi 契约/声明源码接缝；清理确认不可达成员；同步产品文档与验收证据 | 待实施：完整 diff 检查、目标测试、生产构建、架构验证；人工/真实账号边界独立记录 |

阶段可以因真实依赖顺序交错，不能跳过完成要求或把失败标记为通过。每次完成阶段后在下方记录实际结果；保留原始失败和其修复说明。

## 验证方法

- Rust 使用 `./scripts/cargo-shared.sh`，保留磁盘保护；重复构建时只在新改动/失败/未解决风险需要时扩大验证。
- 当前相关命令：adapter unit tests、grok-pi bin unit tests、grok-pi check、`./build.sh`。全量 verifier 报告写到临时目录，避免覆盖历史证据。
- Extension 检查复用现有 Eval、Subagents、Remote TUI 回归入口；测试上下文使用 Pi 1.0 正式 API，不以不存在的 `pi.invokeTool` 模拟生产入口。
- Runtime 使用临时 `GROK_HOME` / `PI_CODING_AGENT_DIR`、本地 MCP fixture 和合成 provider，不读取/修改真实 session、token 或账号配置。该证明属于真实 transport/工具管线 + fixture，不是实际模型/账户验收。
- Cargo 图记录 normal/build、当前平台和 feature 集，包含根 package；体积/耗时只报告实际测量，包数不换算为收益。
- Native 源码校验保留精确文件身份与接缝范围。不得将整个 backend/UI 目录列为忽略来通过检查。

## 状态与证据日志

### 2026-10-02：Spec / Plan 完成

- 起点：`main@222d614d`，无 dirty files。
- Pi：系统 `1.0.0`，已有隔离 RPC/桥扩展注册证明；当前源码 E0425 未修复。
- 生产依赖图：adapter 909 / composition 1013；后续比较必须使用相同计数方式。
- 源码文件未在 spec/plan 前修改。接下来创建 goal，按上述阶段推进。

### 2026-10-02：P0 / P1 推进

- Goal 已创建并保持 active。E0425 漏导入已修复；adapter 190 tests、grok-pi bin 96 tests 均通过，退出码 0。当前仍有既有 dead-code warnings；check/build 后续完成。
- 已将当前支持基线统一到 Pi 1.0，修正实际 repo 路径；历史 Issue 的旧版本事实不全局替换。
- Pi 1.0 `ExtensionToolContext.executeTool` 要求 assistant-issued tool context；外部 Eval MCP 从 `session_start` 直接调用 Eval，缺少该上下文。为保留既有功能，正常 Eval 走官方管线，旧 capture 仅在显式 Eval MCP 开启时安装。Spec FR-02 已据此补充真实边界。
- Eval-only 改用官方 `prepareLoadout.hiddenDeclarations` 隐藏其它顶层声明，同时保留允许的 direct tools active，供正式 nested pipeline 调用；hidden exposure 与 CLI exclusions 仍由 Pi 约束。待实际管线与回归验证。

### 2026-10-02：可运行实施检查点（Goal 继续 active）

- P0：修复 E0425、Pi 1.0 基线、实际 repo 信息。adapter 192 tests PASS；grok-pi check PASS；`CARGO_MAINTENANCE=0 ./build.sh` exit 0，产物 `target/debug/grok-pi`。入口 96 tests 对最新累计改动再次 PASS，退出码 0。
- P1：正常 Eval 使用 `ExtensionToolContext.executeTool`，每 cell/背景任务独立绑定上下文。Eval-only 用 `prepareLoadout.hiddenDeclarations` 保留官方 callable tools。Node 25 REPL async 拒绝改用原生 `handleError`，Bun host 的 JS worker 使用 Node。现有 Eval v2.1 生产回归 PASS。真实安装 Pi + 合成 provider 的 RPC fixture 在 normal / Eval-only 下均 PASS：有效工具只执行一次、阻断与缺参被 Pi 拒绝、hidden tool 不可见、顶层 Eval-only 仅声明 Eval。这个证明不包含真实模型推理。
- P1 replay：官方 Pi 只记录 bounded nested summaries；通过官方 `appendEntry` 保存 UI-only start/end 输出，标记 replayOnly，live 使用 Pi 原始事件以避免双重呈现。新 snapshot 路径仍需补充完整 replay/PTY 验证。
- P2：新增默认关闭的 F2 `pi_mcp`；显式加载 `builtin:mcp` 与必要 Codemode，保留 CLI exclusions。取消/取消续跑都先发 `clear_queue`。完整本地 MCP fixture、exposure/资源与 CLI 交叉验收尚待完成。
- P3：登录/退出改为 native QuestionView，去除私有 Pi component 导入；新增 Anthropic copy-code/取消、Radius 原子配置保存/未知字段保持和 native logout 检查，4 tests PASS。Auth signal 完成有 scoped UI teardown，避免 RPC promise 结束后留下 native overlay。真实账户授权与浏览器联动未验收。
- P3：Codemode image blocks 进入 native gallery/viewer，进程私有临时 cache；2 个 native integration tests PASS，涵盖 live/replay cache 复用和非法载荷。还需要真实 PTY/终端图像显示验证。
- P4：HostFeatureManifest 下沉至 `xai-grok-shared`，旧路径 re-export；Question/ACP DTO 与纯转换逻辑移到已有 `xai-tool-types`，channel/resource 仍留在 tools；McpOAuthConfig 移入 `xai-grok-config`，config-types 取消 Grok MCP client 直接依赖。正常 config 读取采用本地产品配置层。已完成共享 manifest 的 3 个测试；生产根图仍有 Shell/Workflow/Tools/Workspace 入边，**真实 runtime 裁剪未完成**。
- P5：配置只读 upstream remote，fetch 到 `2bdd1d6a`。已按 skill 记录 `37949780..2bdd1d6a` 的 7 commits / 340 Changes；整体 diff 2484 files，+335380/-101458。尚未将记录视为代码迁移；完整 SOURCE_REV/base 未改变。终端 pop fence/恢复组与 clipboard 组需要联动迁移，不能孤立复制 helper。
- P6：累计 diff check 与受影响 Rust 文件格式化 PASS。完整 verifier 的 Pi 源码路径、递归扫描、baseline 维护，以及产品 README/Matrix 状态收尾仍待完成。

下一段按真实依赖继续：先完成 MCP 与 auth/replay 的 runtime/PTY fixture，再将 Workflow 的中性契约从 Shell 分离、建立可证明的生产构建隔离，并吸收适用上游 native TUI 功能组。未 commit/push/创建 worktree，未修改 Pi core 或真实账户/业务数据。

### 2026-10-02：MCP 运行证据与中性 Workflow 契约

- 本轮有实质进展：MCP fixture 揭示初始工具目录早于后台连接完成；初步尝试调用 tool_search 被 Pi 拒绝。核查确认 tool_search/Codemode 均为 model-only，相关嵌套调用已删除，没有更改其 exposure 或绕过 Pi。
- Eval 新增 `await tools.waitFor(name_regex, timeout_ms)`，仅观察公开 registry/callable metadata（标准库 25ms 间隔、有截止时间和 abort signal）；成功后更新本 cell 目录。JS/Python 使用同一契约；原 list/search/describe 仍同步。Pi 1.0 暂无公开 registry-ready 事件，等待不拥有 MCP client、transport、OAuth 或工具注册。
- 真实系统 Pi + 本地 stdio MCP + 合成 provider 的 8 个 RPC 场景 PASS（退出码 0）：普通/Eval-only 原生管线、codemode 与 deferred exposure、显式排除 tool_search、显式排除目标 MCP tool、完全禁用 MCP、Eval-only MCP。资源读取与原始 structuredContent envelope 保留；hidden tool 不可见；禁用时没有启动 MCP server；排除目标时没有 tools/call。
- 同一 RPC 检查新增 get_entries 验证：每个正式 nested call 保存一对 replayOnly UI custom entries，id 与 Pi nested summaries 一致；live 不重放 UI snapshots。Native ACP replay 与 PTY 显示仍需继续验收。
- Workflow backend request/trait 与 drain outcome 移入已有 xai-workflow；纯结果 DTO 移入已有 xai-tool-types。Grok 旧入口 re-export 保持，SubagentResult 的 From 转换在其 producer crate 实现，Pi backend 直接依赖中性契约。没有让 tools 为结果 DTO 新依赖整个 Rhai engine。grok-pi check PASS，xai-workflow 61 tests PASS。
- 现有 Eval v2.1 回归在新 helper 后 PASS；最新 prompt 文案增加异步目录准备说明，其再次回归结果随后记录。实际根构建隔离、Workflow manager/store/notify 的 Shell 依赖、native TUI 上游组迁移与 verifier 收尾仍未完成，goal 保持 active。

### 2026-10-02：Workflow 存储与选择性 TUI 迁移检查点

- P4：WorkflowControl、Tracker 和 Store 已从 Shell 下沉到已有中性层，旧路径保持 re-export。Store 的 effort 保存为宿主解释的字符串；stock 恢复仍先校验 canonical ReasoningEffort，xai-workflow 不依赖 sampling-types。中性引擎 93 tests PASS。
- 修复真实存储缺陷：原 Pi host 丢弃 WorkflowRunState，却给 WorkflowRunStateAndAck 返回成功。新 standalone writer 按序原子写入，ACK 传播存储失败，迟到旧 revision 和 cleared tombstone 保持保护；生产 WorkflowHost 运行后 state.json 为 Complete 的检查通过。Adapter 193 tests PASS。
- P3：Pi 退出、事件通道结束与实际 session 切换时，撤销待处理 native auth scopes；Radius mcp.json 原子写入保持现有 symlink，并在提交前复查目标。Auth 5 tests PASS，不涉及真实账号。
- P5：选择性迁移三个原生组：SUPER+Enter 选区替换、完整不安全 display 字符过滤、Markdown/OSC8 链接边界检查。定向检查 2 + 2 + 62 + 16 + 1 tests 全部 PASS，provenance 见 docs/upstream/TUI_BACKPORTS.md。SOURCE_REV/完整 upstream base 不变；终端/clipboard 联动组仍未迁移。
- P6：adapter/composition 扫描改为递归模块；source inventory 使用 Git 候选源文件；Pi 契约支持未初始化源码子模块时读取实际系统包的 dist declarations。mock 7 checks PASS；rustfmt 无写入地解析 568 files PASS。Native verifier 19/21 PASS，两个原始 source identity manifests 仍明确 FAIL，没有用目录级例外或自动哈希更新消除失败。
- Full Pager lib 不能运行：53 个既有编译错误，涉及旧 Subagent fields、settings render、hook helpers 等。Public PromptWidget integration 通过只证明当前生产输入路径；不能代替全量 lib suite。
- Bin test 首次重跑被磁盘保护以 exit 74 中止，测试未执行到结果；使用本项目维护脚本清理可再生 incremental cache，target 从 102.1 GiB 降至 42.3 GiB，20 GiB 下限保持。入口测试/build/依赖图的重跑结果随后补录。
- Goal 工具查询实际返回 blocked，与较早 active 回复不一致；本次未将目标标记 complete。完整 root runtime 裁剪、精确 identity baseline、终端/clipboard、replay/PTY 与剩余验收仍未完成，不能把本轮迁移写成整体完成。

### 2026-10-02：最终可运行检查点与依赖图

- 磁盘维护后入口 96 tests PASS，`./build.sh` exit 0（dev profile，3m16s），产物 `target/debug/grok-pi`，Pi 1.0.0 门禁；没有降低 20 GiB free-space floor。
- 同一 normal/build、prefix-none、package-format 去重计数：xai-workflow 85，四个 stock runtime 包均不可达；adapter 909、composition 1013，仍可达 xai-grok-shell / xai-grok-tools / xai-grok-workspace / xai-grok-mcp。因此仅完成中性组件隔离，根生产图尚未裁剪，也没有 package-count 收益。
- 完整构建/入口测试/graph 输出保留在临时证据目录 `grok-pi-final-checkpoint-r00td4zd`；TUI 定向检查目录 `grok-pi-backport-checks-f_jlgsu0`。当前语法、mock、native verifier 报告位于 `/tmp/grok-pi-*-20261002.json`。静态 source identity 的两项失败与 full Pager lib 的 53 错误均保留原始事实。
- README、双语 Feature Matrix、Native alignment 与 Verification 已同步当前状态；未把 MCP 写成 planned-only，未把局部 TUI backport 写成完整上游同步。真实模型/OAuth/人工体验仍独立待验收。
- 剩余实施：manager/host/registry/notify 的 Shell 断边与根编译 profile；terminal/clipboard 组联动导入；精确 source identity migration；ACP replay/PTY 与相关 stock 检查。Goal 未完成，工具状态仍 blocked；未 commit/push/创建 worktree、未修改 Pi core 或真实用户凭据/业务数据。

### 2026-10-02：继续实施与分阶段提交授权

- 用户明确要求“继续完成，边做边 commit”，覆盖此前仅本地修改、不 commit 的约束；不扩大到 push/worktree/Pi core/真实用户数据。
- 当前从 main@222d614d 和全部既有本任务改动继续；中断时没有仍运行的 Cargo。先提交 Spec/Plan 的当前范围与检查点，再分别验证、提交 Pi 1.0 接缝、中性契约、上游原生组与 verifier 修复。
- 三个并行执行范围：Workflow 真正切断 adapter→Shell；terminal restore/reader/fence 与必要 clipboard/repaint；来源 hash baseline 精确迁移与旧 Pager fixture 恢复。当前会话模型继承，不按旧角色别名切换；主代理统一 commit 和 Cargo 验证。
- 新核查发现 Workflow effort 被 host 计算后未进入 backend request，补齐传递并按 Pi SDK thinking level 映射；notifications/activity 已下沉，wire-shape 定向证明已通过（adapter 194 tests，Workflow 95 tests），最新 effort 改动仍需验证。
