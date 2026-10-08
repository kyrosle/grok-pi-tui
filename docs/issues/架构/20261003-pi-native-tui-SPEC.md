---
id: "2026-10-03-pi-native-tui-spec"
title: "grok-pi 总体改造：裁剪 Grok Build 业务，成为 Pi 原生 TUI SPEC"
status: "accepted"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# grok-pi 总体改造 SPEC：Pi 原生 TUI

本文件是 grok-pi 后续所有增量的**总纲**。执行顺序、阶段、命令与回执见 [PLAN](20261003-pi-native-tui-PLAN.md)。单阶段可以另开子 SPEC/PLAN，但不得与本文件的终态和约束冲突；冲突时先修订本文件。

起点：`main@7bbd748a`（[产品入口裁剪](20261003-pi-product-surface-PLAN.md)已完成）。此前的 [Pi-first](20261002-pi-first-tui-SPEC.md)、[深度适配](20261003-pi-deep-adaptation-SPEC.md)、[产品入口裁剪](20261003-pi-product-surface-SPEC.md) 是本总纲的已完成检查点，其验收证据保持独立。

## 1. 用户目标

用户明确（2026-10-03）：

1. grok-pi 要成为**完整的 TUI 工具，深度接入 Pi**，结合 Pi 1.0 的 RPC、extension API 与 interactive 模式语义。
2. **移除 Grok Build 的 Grok 业务功能**（账号、计费、voice、遥测、Grok agent/工具/工作区、插件市场、公告、反馈等），只保留并改造终端 UI 能力。
3. **Grok Build 不再作为上游**。今后只审阅它的提交，逐项评估是否手动移植到 grok-pi；不再整体合并，也不再以"上游 blob 一致"作为约束。

## 2. 终态定义

终态下 grok-pi 是一个 **"以 Pi 为唯一内核的原生 Rust TUI"**：

```text
┌──────────────────────────── grok-pi (单一产品二进制) ────────────────────────────┐
│  TUI 层（源自 Grok Pager，已去业务化）                                              │
│    prompt / slash / palette / F2 / QuestionView / toast / banner / status line   │
│    scrollback / tool card / diff / markdown / mermaid / session picker / tree    │
│                │ ACP（进程内通道）                                                │
│  pi-grok-adapter（headless 库：Pi JSONL RPC ⇄ ACP 投影，不持有业务状态）             │
│                │ stdin/stdout JSONL                                              │
├────────────────┼─────────────────────────────────────────────────────────────────┤
│  pi --mode rpc（系统 Pi ≥ 1.0）                                                   │
│    models / providers / auth / agent loop / tools / sessions / tree / compaction │
│    + pi-grok-host-bridge 扩展（补齐 RPC 未暴露的官方 extension API）              │
│    + 可选产品扩展（Bash、Subagents、Todo、Plan、Goal、Web config …）         │
└──────────────────────────────────────────────────────────────────────────────────┘
Grok Build（xai-org/grok-build）：只作参考仓库，逐提交审阅、按需移植 UI 改进
```

用户 2026-10-07 明确增加可选Durable mode，细节见[深度接入SPEC草案](20261007-pi-durable-integration-SPEC.md)/[PLAN](20261007-pi-durable-integration-PLAN.md)。上图为持续保留的出厂默认Pi RPC模式；F2保存开启（下次启动生效）或CLI单次覆盖开启时，使用同一原生Pager/ACP接入官方Pi Durable Harness与隔离SQLite。未开启时不初始化Durable依赖、数据库或owner。两种会话各属原后端，不热切、不隐式迁移；下表及§3的RPC/SessionManager/`appendEntry`要求描述默认模式，新模式的官方任务/storage所有权及capability按子SPEC验收。可选后端已在开发源码实施，当前自动验收和兼容缺口见子PLAN，本增量不更改出厂默认模式或宣称总纲完成。

终态可度量条件（全部满足才算完成）：

| ID | 终态条件 |
|---|---|
| END-01 | 仓库只构建 grok-pi 一个产品。stock Grok profile、`stock-runtime` feature、`xai-grok-pager` stock 二进制均移除 |
| END-02 | Grok 业务 crate（§5.3 "移除"类）不在 workspace 中，也不在依赖图中；依赖禁止名单检查通过 |
| END-03 | Pager 内 `external_agent` / `is_external` / `UiProfile` 分支为 0；`UiProfile`、`ExternalUiProfile`、`BuiltinCommandProfile::Grok` 删除，Pi 行为成为唯一行为 |
| END-04 | 产品二进制不含 Grok 服务端点：`x.ai`、`grok.com`、`api.x.ai`、Mixpanel、Sentry DSN 等字符串扫描为空（参考文档和许可声明除外） |
| END-05 | Pi 1.0 interactive 内置命令（§6.1）都有对应的原生 TUI 入口，或在矩阵中有明确的不提供理由 |
| END-06 | adapter 不持有 Pi 有官方等价物的业务状态：队列交给 Pi，Plan/Goal 移入 Pi 扩展；adapter 只做投影和短暂的 UI 关联 |
| END-07 | 扩展层没有私有 prototype patch，没有文件轮询 IPC，`ctx.ui.custom` 只有一个所有者 |
| END-08 | 用户可见文案中 "Grok" 只出现在"基于 Grok Build"的致谢和许可里；产品名、帮助、教程、错误信息统一为 grok-pi / Pi |
| END-09 | 验证体系由架构守卫组成（依赖禁止名单、端点扫描、adapter 无 UI 依赖、命令/设置白名单、PTY），不再依赖上游 blob 身份 |

## 3. 所有权原则

| 领域 | 权威 | grok-pi 的职责 |
|---|---|---|
| 模型、provider、thinking、scoped models | Pi | 呈现选择器，调用 RPC/扩展，展示状态 |
| 鉴权（login/logout、OAuth、API key） | Pi `ModelRuntime` | 原生对话框，薄桥委托；不读写凭据 |
| agent loop、工具、MCP、codemode、tool_search | Pi | 工具卡片、diff、权限/确认对话框 |
| 输入队列（steer / follow-up） | Pi | 呈现队列和编辑草稿；不维护第二份队列 |
| 会话、树、leaf、fork/clone、标签 | Pi | session picker、tree 视图；只读扫描会话目录用于 `/resume` |
| compaction、retry | Pi | banner、状态、开关 |
| 设置（Pi `settings.json`） | Pi | 原生编辑器 + 重载闭环 |
| packages、skills、prompt templates、extensions、trust | Pi | 资源面板；调用官方 CLI/SDK |
| 终端外观（主题、布局、鼠标、快捷键呈现） | grok-pi | 存在 `$GROK_HOME/config.toml`（UI 专属） |
| 更新 grok-pi 自身 | grok-pi | `grok-pi update`（GitHub Releases） |
| Plan / Goal / Todo / Subagents 等产品扩展 | 对应的 Pi 扩展 | 原生卡片/面板呈现；状态以 `appendEntry` 存在 Pi 会话中 |

原则：

- **P-1 不改 Pi 源码。** Pi 缺少的能力优先用官方 extension API 补齐；仍做不到的，在矩阵里如实写明，不用 JSONL 改写或无关 RPC 模拟。
- **P-2 不做第二套 renderer。** 所有可见界面来自 Pager 原生组件；不引入 ASCII fallback UI。
- **P-3 adapter 只是投影层。** headless、library-only，不依赖 Ratatui/Crossterm。
- **P-4 状态隔离保留。** `~/.grok-pi` / `<repo>/.grok-pi` 规则不变（见 AGENTS.md）。
- **P-5 不冒称迁移。** 隐藏 UI 不等于迁移完成；债务进矩阵，直到真正搬到 Pi 一侧。

## 4. 治理变更：Grok Build 从"上游"改为"参考仓库"

| ID | 要求 |
|---|---|
| GV-01 | git remote `upstream` 改名为 `grok-build`，push URL 设为不可用。文档统一称"参考仓库" |
| GV-02 | 新增 `docs/grok-build/REVIEWED`，记录最后一次审阅到的 grok-build 提交 SHA 及其 `Source-Revision`。审阅范围 = `REVIEWED..grok-build/main` |
| GV-03 | `upstream-changelog` skill 改为 `grok-build-review`：转录 `Changes:` 列表，逐条定性为 **移植 / 改造后移植 / 跳过（业务）/ 观察**，写明理由和目标 crate；已删除的业务路径自动标为跳过。输出写到 `docs/grok-build/REVIEW_LOG.md`（原 `docs/upstream/UPSTREAM_CHANGELOG.md` 归档） |
| GV-04 | 移植只在独立 worktree 中做，按路径导出补丁或手动重写；每个移植单独提交，提交信息带 `Ported-From: grok-build <sha> (Source-Revision <rev>)` |
| GV-05 | 退役 `SOURCE_REV`、AGENTS.md 中的 `base`、"Grok Build 源码只读"、"Upstream sync workflow" 整节，以及上游 blob 身份基线（`grok_uploaded_baseline_sha256.json` 及其三层 identity）。以 §8 的架构守卫取代 |
| GV-06 | 保留许可与署名：`LICENSE`、`THIRD-PARTY-NOTICES`、`third_party/` 不变；移植代码保留原版权头 |
| GV-07 | 允许修改、重命名、删除源自 Grok Build 的源码。crate 名 `xai-*` 默认**保留**，以降低今后移植的路径映射成本；是否改名在 T7 单独评估 |

## 5. 裁剪范围

### 5.1 产品入口（运行时可见）

| ID | 要求 |
|---|---|
| PR-01 | 斜杠命令、命令面板、F2、Web 配置目录、欢迎页、教程、快捷键帮助，全部只用**白名单**（命令面板目前还是黑名单，必须改成白名单） |
| PR-02 | 补齐已知残留：plugin CTA 环境变量覆盖（`event_loop.rs:1411`）、workspace dashboard 环境变量（`event_loop.rs:1417`）、命令面板 "Send Feedback"（`views/modal.rs:795`）、`OpenMemoryModal`（`dispatch/router.rs:1626`）、`mode_support` 对未知命令默认返回 `Both`（`slash/registry.rs:202`） |
| PR-03 | 任何 Grok 账号/计费/训练数据/权限模式/分享/公告/反馈/记忆/工作区/插件的 Action，在 grok-pi 中不会产生网络请求，也不会发出 Pi 没有处理器的请求 |
| PR-04 | 产品入口裁剪产生的过渡性 `external_agent` 守卫，在 T7 删除 stock profile 时一并清除，不长期保留 |

### 5.2 编译期切除

| ID | 要求 |
|---|---|
| CB-01 | 业务 crate 先变成可选依赖，挂在 `stock-runtime` 下（过渡）；到 T7 随 stock profile 一起从 workspace 删除 |
| CB-02 | 每切除一个 crate，同时把它加进 `pi_dependency_profile.py` 的禁止名单，并删除对应的运行时分支 |
| CB-03 | 遥测：Sentry、Mixpanel、OTEL 导出从 grok-pi 中去掉。本地诊断改为 `tracing` + `$GROK_HOME/logs`；`startup`/`session_ctx` 等打点 API 换成本地空实现或本地日志 |
| CB-04 | 共享类型如果寄居在业务 crate 中（例如 `xai-grok-tools` 里的 UI 类型、`xai-grok-voice` 里的 shared types），先抽到 UI 侧 crate（如 `xai-grok-shared`、`xai-tool-types`），再删除原 crate |
| CB-05 | 扩展崩溃上报（`grok_pi/extension_self_heal.rs:210` → `ext-crash-telemetry.dwsycode.workers.dev`）保持"默认询问、拒绝即不发送"，在矩阵中明确列为 grok-pi 自有的可选上报，不属于 Grok 遥测 |

### 5.3 crate 初始分类

> 初始分类，T0 用 `cargo tree` 和引用扫描核实，核实结果回写本表。"待定"在所属阶段决策。

| 类别 | crate |
|---|---|
| **保留：TUI 核心** | `xai-grok-pager`、`xai-grok-pager-render`、`xai-grok-pager-diff`、`xai-grok-pager-minimal`、`xai-grok-markdown`、`xai-grok-markdown-core`、`xai-grok-mermaid`（+ `third_party/` 的 dagre/graphlib/mermaid-to-svg）、`xai-ratatui-textarea`、`xai-ratatui-inline`、`xai-tty-utils`、`xai-grok-status-line`、`xai-fuzzy-file-search`、`xai-grok-image` |
| **保留：基础设施** | `xai-grok-pager-bin`（只保留 `grok-pi`）、`pi-grok-adapter`、`xai-acp-lib`、`xai-grok-config`、`xai-grok-config-types`、`xai-grok-paths`、`xai-dirs`、`xai-file-utils`、`xai-grok-shared`、`xai-grok-version`、`xai-crash-handler`、`xai-grok-extra-ca`、`xai-token-estimation`、`xai-grok-update`（只保留 Pi 发布通道）、`xai-tracing*`、`xai-grok-pager-pty-harness`/`xai-grok-test-support`（测试） |
| **移除：Grok 业务** | `xai-grok-agent`、`xai-grok-tools`、`xai-grok-tools-api`、`xai-grok-shell*`、`xai-grok-workspace*`、`xai-grok-mcp`、`xai-grok-plugin-marketplace`、`xai-grok-sampler`、`xai-grok-sampling-types`、`xai-grok-login`、`xai-grok-auth`、`xai-grok-secrets`、`xai-grok-telemetry`、`xai-grok-otel`、`xai-mixpanel`、`xai-grok-announcements`、`xai-grok-feedback`、`xai-grok-gboom`、`xai-grok-dashboard-store`、`xai-grok-voice`、`xai-grok-memory`、`xai-grok-session-search`、`xai-grok-diag-server`、`xai-computer-hub-*`、`xai-grok-bundle`、`xai-grok-models` |
| **待定** | `xai-grok-sandbox`（grok-pi 的 `sandbox-enforce` 是否仍有实际作用？工具执行在 Pi 进程内）、`xai-grok-hooks`/`xai-hooks-plugins-types`（`.grok-pi/hooks` 是否改由 Pi 扩展承担）、`xai-workflow`（见 EX-08）、`xai-grok-foreign-sessions`、`xai-grok-active-sessions`、`xai-prompt-queue`（见 PI-01）、`xai-grok-compaction`/`xai-compaction-transcript`、`xai-grok-session-events`、`xai-codebase-graph`、`xai-fast-worktree`、`xai-hunk-tracker`、`xai-gix-status`、`xai-fsnotify`、`crates/common/*` 的工具协议类 crate |

决策规则：被 Pager UI 直接使用、且不发起 Grok 服务调用的，保留；只服务于 Grok agent/工具运行时或 Grok 服务的，移除；两者都有的，先拆分再处理。

T0 核实结果见 [逐 crate 清单](20261003-pi-native-tui-CRATES.md)：锁定的 Pi normal/build 图为 796 packages，表中 87 个实际 workspace crate 中 56 个可达、31 个未链接；初始移除类仍有 15 个可达（含补入的 xai-mixpanel）。`shell*`/`workspace*` 包含被 UI 使用的 `shell-base`/`workspace-types`；sampling-types/models 同样需先抽中立契约。`xai-tracing*` 与测试支持不在生产图中；common tool-types 是 UI/diff 契约，tool-runtime 不在生产图中。上述引用计数包含 stock/test 源码，仅作为耦合证据，不能当作运行执行证明。

## 6. Pi 1.0 深度接入

依据：安装的 `@earendil-works/pi-coding-agent@1.0.0`（`dist/modes/rpc/rpc-types.d.ts`、`dist/core/extensions/types.d.ts`、`docs/rpc.md`、`docs/rpc-extension-ui.md`、`docs/slash-commands.md`、`docs/keybindings.md`、`CHANGELOG.md`）。实施阶段如需源码对照，应初始化 `pi-main` 子模块并锁定到与已安装版本一致的 tag。

### 6.1 Pi interactive 内置命令映射

| Pi 命令 | grok-pi 原生入口 | 通道 |
|---|---|---|
| `/model`、`/thinking`、`/scoped-models` | `/model` 选择器、thinking 切换（Shift+Tab）、scoped 模型编辑器 | RPC `set_model`/`cycle_model`/`set_thinking_level`；scoped 走 settings + reload |
| `/settings` | F2（Pi 页）+ `/pi-settings` | Pi `settings.json` 事务 + host-bridge reload |
| `/login`、`/logout` | 原生登录对话框 | pi-grok-auth 扩展 → `ModelRuntime` |
| `/new`、`/resume`、`/name`、`/session` | `/new`、`SessionPicker`、`/rename`、`/session-info` | RPC `new_session`/`switch_session`/`set_session_name`/`get_session_stats`；按需扫描会话目录 |
| `/tree`、`/fork`、`/clone` | tree / tree-map 视图、`/fork`、`/clone` | `navigateTree` 走 host-bridge；`fork`/`clone`/`get_tree` 走 RPC |
| `/compact` | `/compact` + 压缩 banner | RPC `compact`/`set_auto_compaction` |
| `/copy`、`/export` | `/copy`、`/export` | `get_last_assistant_text`、`export_html`；JSONL 导出走 host-bridge |
| `/import`、`/share`、`/bug` | 待评估；不提供时在矩阵中写明 | Pi 内置 TUI-only；如需提供走 host-bridge |
| `/trust` | 资源面板中的信任项 | host-bridge（`isProjectTrusted` + Pi 信任配置） |
| `/reload` | `/reload` | host-bridge `ctx.reload` + reload ACK |
| `/hotkeys`、`/changelog` | `/hotkeys`（显示实际键位）、欢迎页 Changelog（grok-pi 自身） | 本地 |
| `/quit` | `/exit` | 本地 |
| `/mcp`（内置扩展命令） | 通过 `get_commands` 透传 | RPC |
| skills / prompt templates / 扩展命令 | 斜杠补全按 `sourceInfo` 分组显示 | RPC `get_commands` |

### 6.2 功能要求

| ID | 要求 | 验收要点 |
|---|---|---|
| PI-01 | **队列交给 Pi。** 用 `prompt(streamingBehavior)`/`steer`/`follow_up`/`clear_queue`/`set_steering_mode`/`set_follow_up_mode` 和 `queue_update` 事件取代 `queue_bridge` 的自有队列。"未提交草稿编辑"留在 Pager 的 prompt 一侧，不复制 Pi 队列 | Alt+Enter 追加、Alt+Up 取回、`disposition` 三态、中断/清队列、会话切换；队列显示只来自 `queue_update` |
| PI-02 | "一轮结束"以 `agent_settled` 为准（`agent_end` 只用于展示阶段） | 扩展发起的后续工作不会出现幽灵运行态 |
| PI-03 | 斜杠目录 = 本地 UI 命令白名单 ∪ `get_commands`（扩展/skill/模板，带 `sourceInfo`）；资源变化后刷新 | reload / package 安装后补全即时更新 |
| PI-04 | 树导航、标签、reload、JSONL 导出、信任等 RPC 未暴露的能力统一走 **host-bridge 扩展**（见 EX-01） | 同一会话内 leaf 移动；不得用 fork/switch_session 冒充 |
| PI-05 | scoped models、默认模型、Pi 设置的编辑都写回 Pi 自己的配置文件，并经 reload 生效；外部编辑冲突时先重读 | Web 与 F2 同时编辑不会互相覆盖 |
| PI-06 | 扩展 UI 子协议完整映射：`select`/`confirm`/`input`（含 timeout）/`editor` → QuestionView；`notify` → toast；`setStatus` → 状态栏；`setWidget` → 编辑器上下方的 widget 区域；`setTitle` → 终端标题；`set_editor_text` → prompt | 每种请求类型都有 fixture 测试；取消、超时、会话切换时会撤销 |
| PI-07 | `registerMessageRenderer`/`registerEntryRenderer` 在 RPC 下不可用 → 约定 `customType` 与结构化 `details`，由 Pager 原生卡片渲染（todo、subagent、plan、goal、recap 等）；未知类型降级为通用卡片 | live 与 resume 回放一致 |
| PI-08 | compaction/retry 全部事件（含 `summarization_retry_*`）映射为 banner/状态；提供 `abort_retry` 入口 | 运行中切换、取消、失败回执 |
| PI-09 | `bash` RPC：`!cmd`（计入上下文）与 `!!cmd`（`excludeFromContext`）；`bash_execution_update` 流式；`abort_bash` | 与增强 Bash 扩展共存，无重复卡片 |
| PI-10 | 工具投影支持 Pi 1.0 工具元数据：`exposure`、`namespace`、`annotations`、`outputSchema`；codemode / tool_search / MCP 内置扩展的调用有可读卡片 | 隐藏工具不出卡片；嵌套 `executeTool` 正确归属 |
| PI-11 | 快捷键默认对齐 Pi：Ctrl+L 模型选择、Ctrl+P 循环模型、Shift+Tab thinking、Ctrl+T/Ctrl+O 切换 thinking/工具显示、Ctrl+G 外部编辑器、Ctrl+V 粘贴图片、Esc 中断、Alt+Enter 追加、Alt+Up 取回。可选读取 `~/.pi/agent/keybindings.json` 覆盖；和 Pager 原生键冲突时以本表为准，并在 `/hotkeys` 中显示 | 键位表测试 + PTY |
| PI-12 | 主题：grok-pi 主题仍由 Pager 负责；可选把 Pi 主题 JSON 映射到 Pager 色板（只读导入） | 不写 Pi 主题文件 |
| PI-13 | 资源：packages（install/remove/update）、skills、prompt templates、extensions 的启停与作用域（Global/Project/trust）沿用深度适配阶段成果，补齐 Pi 1.0 的 `builtin:<name>` 内置扩展显示 | 已有 DA-03/04 测试保持通过 |
| PI-14 | 模型元数据：virtual model 与实际派发模型分开显示；image/classifier 模型不进聊天选择器（延续 DA-05/06） | 已有测试保持通过 |
| PI-15 | 帮助、教程、欢迎页按 Pi 1.0 的概念组织，明确区分 "Pi 内置" 和 "grok-pi 附带扩展" | 教程合同测试 |

### 6.3 不提供（终态也不提供，需在矩阵中写明）

- Pi interactive 的 TUI 组件工厂（`ctx.ui.custom` 的任意组件、`setEditorComponent`、`setFooter`/`setHeader`、`addAutocompleteProvider`）：RPC 下是 no-op。Remote TUI 兼容层只保证已验证的组件范围（延续 DA-07/08）。
- Grok 的账号、计费、voice、训练数据设置、插件市场、Grok MCP 管理、工作区、分享、公告、反馈。

## 7. 扩展层整合

| ID | 要求 |
|---|---|
| EX-01 | 新建唯一的 **`pi-grok-host-bridge`** 扩展，用一条通道（注册命令 + 结构化 `notify`/`setStatus` 回执，或 Pi 1.0 已有的更合适官方通道）承载 navigateTree、setLabel、reload、trust、JSONL 导出、settings 事务等；现有 `tree_bridge`、`rpc_compat`、`native_commands`、`host_feature` 等零散注入合并进来 |
| EX-02 | 逐项删除 `pi-grok-rpc-compat` 对 `ExtensionRunner.prototype` 的私有 patch 和对 `process.stdout.write` 的包装；Pi 1.0 已有官方能力的直接改用官方 API，没有的写进矩阵 |
| EX-03 | `ctx.ui.custom` 只保留一个所有者（Remote TUI 与 rust-tui-bridge 二选一或合并） |
| EX-04 | 去掉所有文件轮询 IPC（shortcut-manager 50ms 轮询、rollback/workflows 的文件交换），改为扩展事件 + RPC 通道 |
| EX-05 | shortcut-manager 的配置迁到 `$GROK_HOME`，不再写 `~/.pi` |
| EX-06 | **Plan / Goal 移入 Pi 扩展**：状态用 `appendEntry` 持久化，提醒与续跑用 `before_agent_start`/`agent_settled` 等事件；adapter 的 `plan_mode.rs`、`goal_host.rs`、`loop_host.rs` 退化为投影 |
| EX-07 | 拆分 `pi-grok-bash/index.ts`（约 650 行）的职责；默认覆盖内置 bash 改为可配置，私有的空闲期 fallback 改走官方通道 |
| EX-08 | Rhai Workflow：二选一，在 T6 决策——(a) 迁到 Pi 扩展（TS 编排，Pi child sessions），提供 `.rhai` 迁移说明；(b) 保持可选功能并冻结，在矩阵中标为 grok-pi 自有。决策前维持默认关闭 |
| EX-09 | 所有注入扩展继续满足"injector 物化全部相对 import"规则（见 AGENTS.md 诊断章节），新增模块必须扩展 injector 单测 |

## 8. 验证体系

| ID | 守卫 | 替代/保留 |
|---|---|---|
| VF-01 | **依赖禁止名单**：§5.3 移除类 crate + 音频后端 + sentry/mixpanel/opentelemetry 导出器，不得出现在 grok-pi 的 normal/build 依赖图中 | 扩展现有 `pi_dependency_profile.py` |
| VF-02 | **端点扫描**：对产品二进制和 `crates/` 生产源码扫描 Grok 服务域名/密钥前缀；允许名单只含致谢与许可 | 新增 |
| VF-03 | **adapter 纯度**：`pi-grok-adapter` 不依赖 ratatui/crossterm/任何 Pager crate | 保留 |
| VF-04 | **入口白名单**：斜杠、命令面板、F2、Web catalog 的实际集合与白名单逐项相等 | 扩展现有 `verify_native_grok.py` |
| VF-05 | **Pi 合同**：mock RPC + 已安装 Pi 的 fixture（disposition、reload ACK、EOF、扩展 UI 各请求类型） | 保留并扩充 |
| VF-06 | **原生 PTY**：产品入口、设置保存/重开/回滚、队列、树导航、退出 | 保留并扩充 |
| VF-07 | Rust 语法检查（rustfmt 解析） | 保留 |
| VF-08 | 上游 blob 三层身份（`grok_uploaded_baseline_sha256.json`、sourceguard、negative） | **T0 退役**，被 VF-01~04 取代 |

VF-02 区分实际服务域名/URL 与 ACP 方法命名空间；`x.ai/session/...` 协议标识本身不是网络端点。源码与二进制均检查，以免只扫二进制漏掉宏编码的域名。过渡期 report 明确列出命中，不冒称已满足 END-04。

END-08 检查产品自有的静态文案；Pi 返回的 provider/model 名称、工具输出、用户数据和实际终端名称保持原样，不通过修改这些数据来制造品牌扫描通过。

验证层次保持分离：源码/守卫、合成 provider 传输、原生 PTY、真实 provider/OAuth 手工验收。回报时逐层说明，不以一层通过代替另一层。

## 9. 约束与非目标

- 不修改 Pi core，不把 Pi 当子模块编译进来；系统 Pi ≥ 1.0.0 仍是默认 host。
- 不删除或迁移用户的已有配置、凭据、会话数据；废弃的配置键只是被忽略，迁移另行说明。
- 不 push、不改远程仓库，除非用户单独授权；本地阶段提交按 PLAN 执行。
- 不追求与 Grok Build 的源码可合并性。
- 不为本次改造重写 Pager 的渲染核心；只做去业务化和 Pi 语义对齐。

## 10. 风险

| 风险 | 缓解 |
|---|---|
| 业务 crate 与 UI 类型纠缠，切除时引起大面积编译错误 | CB-04 先抽类型；每个 crate 单独一次提交；Pi profile 每步 check |
| 删除 stock profile 后，大量 Pager 测试依赖 stock 行为 | T7 前统计测试归属；stock 专属测试随功能删除，Pi 行为测试改为默认 |
| 队列迁移导致草稿编辑或 stable-id 能力退化 | PI-01 明确草稿只在 prompt 侧；先在 PTY 中录制当前行为作为基线 |
| Pi 次版本升级改变 RPC 或扩展 API | `pi_version.rs` 下限检查；合同 fixture 跟随已安装版本；矩阵记录版本 |
| 停止整体同步后，Grok Build 的终端兼容修复被遗漏 | GV-03 审阅流程定期执行，重点关注终端、输入、渲染路径 |
| 遥测移除后缺少故障信息 | CB-03 本地日志 + `/doctor` + 扩展 stderr 日志（`pi-rpc-stderr.log`） |

## 11. 交付

按 [PLAN](20261003-pi-native-tui-PLAN.md) 的 T0–T8 分阶段交付，每阶段：先在 PLAN 记录范围，实施，跑本阶段要求的验证，回写回执，本地提交。全部终态条件 END-01~09 满足后，本 SPEC 状态改为 `completed`，并更新 README、`docs/README.zh-CN.md`、`FEATURE_MATRIX`、`NATIVE_GROK_TUI_ALIGNMENT`、`VERIFICATION`。

## 2026-10-07 授权增量

按[四组裁撤 SPEC](20261007-pi-native-four-cuts-SPEC.md)/[PLAN](20261007-pi-native-four-cuts-PLAN.md)，退休自建 Eval runtime、实验 native-commands、第二套 rust-tui-bridge、startup profiler。此项先按用户指定顺序实施，不改变其他 T2–T8 的状态；当前正式 native commands、官方 Codemode、增强 Bash、唯一保留 Remote TUI 与历史会话呈现保留。
