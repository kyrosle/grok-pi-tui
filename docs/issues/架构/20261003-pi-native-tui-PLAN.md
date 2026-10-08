---
id: "2026-10-03-pi-native-tui-plan"
title: "grok-pi 总体改造：裁剪 Grok Build 业务，成为 Pi 原生 TUI PLAN"
status: "in-progress"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# grok-pi 总体改造 PLAN

依据 [SPEC](20261003-pi-native-tui-SPEC.md)。起点 `main@7bbd748a`。每阶段一个或多个本地提交，不 push。阶段内如果范围较大，可以另开子 issue，但要在本表登记。

## 基线（2026-10-03，`7bbd748a`）

| 指标 | 基线 | 当前 T0/T1 | 来源 |
|---|---|---|---|
| grok-pi normal/build 依赖包数 | 796 | 796；29 removal pending | 当前锁定 production tree / dependency guard |
| Pager 中 external 分支（不含测试） | 约 166 处，31 个文件 | 166匹配行/34文件（含声明/注释，非AST分支计数） | `external_agent\|is_external\|UiProfile::External\|BuiltinCommandProfile\|ExternalUiProfile` 扫描 |
| 仍被 grok-pi 链接的 Grok 业务 crate | telemetry/otel、login/auth/secrets、announcements、feedback、gboom、dashboard-store、voice（不含音频后端）、foreign-sessions 等 | 15移除类+14telemetry exporter packages；待T2切除 | 审计 + `Cargo.toml` |
| 注入的 Pi 扩展 | 23 个（`extensions/`） | 未重组，待T4 | 目录 |
| 已知失败测试 | 扩大 `external_` 扫描时 5 个（Ctrl+O ×2、dashboard toast 前缀 ×1、foreign session ×2） | 原5项修复；external_ 60/0 | 产品入口裁剪 PLAN / 当前test log |
| Pi host | 系统 Pi 1.0.0；`pi-main` 子模块未初始化 | 系统Pi1.0.0，未改Pi源 | `git submodule status` |

每阶段结束时更新这张表的"当前值"列（在回执里写）。

## 阶段总览

用户 2026-10-07 要求设计 Pi Durable 深度接入，并明确采用“默认保持当前行为、Durable显式开启”的运行模式：[SPEC 草案](20261007-pi-durable-integration-SPEC.md) / [PLAN](20261007-pi-durable-integration-PLAN.md) / [官方源码核对](20261007-pi-durable-integration-SOURCE.md)。本子项已进入实验实现，自动验收和兼容缺口见子PLAN；拟通过F2持久设置（下次启动生效）或CLI单次覆盖选择官方Harness的headless后端，复用原生Pager。出厂默认Pi RPC持续保留，未开启时不初始化Durable。普通扩展、Codemode/MCP、会话及任务所有权按模式逐项验收；T3/T4/T6的经典模式工作保持，新模式见子PLAN。本项不安排出厂默认切换，也不改变下表阶段完成状态。

用户 2026-10-07 授权优先落实[四组裁撤 SPEC](20261007-pi-native-four-cuts-SPEC.md)/[PLAN](20261007-pi-native-four-cuts-PLAN.md)，四组子项已完成（90 bin、205 adapter、9 native PTY；未发布）。native-commands 与第二套 rust-tui-bridge 已从该子项退出；T4 后续只整合仍必要的桥接。本子项不代表整个 T4/T6 完成。

用户 2026-10-05 明确优先安排 T5 的设置交互增量：[设置功能命名与中英文 SPEC](20261005-settings-language-SPEC.md) / [PLAN](20261005-settings-language-PLAN.md)。该子项已完成（功能命名、配置页中英文、本机 tpig 交付）；不改变其他阶段的完成状态。

| 阶段 | 目标 | 对应 SPEC | 依赖 | 状态 |
|---|---|---|---|---|
| T0 | 治理切换：Grok Build 改为参考仓库，验证体系换成架构守卫 | GV-01~07、VF-01~08 | — | 完成，T0 验证通过 |
| T1 | 产品入口收口：白名单化，补齐残留，处理 5 个失败测试 | PR-01~03 | T0 | 完成，60/0 与原生PTY通过 |
| T2 | 编译期切除 I：遥测、voice、login/auth、公告、反馈、gboom、dashboard | CB-01~05 | T1 | 待开始 |
| T3 | Pi 协议对齐 I：队列、settled、命令目录、bash、compaction/retry、工具元数据 | PI-01~03、PI-08~10 | T1 | 待开始 |
| T4 | 扩展层整合：host-bridge，去 prototype patch 和文件轮询 | EX-01~05、EX-09、PI-04 | T3 | 待开始 |
| T5 | Pi 交互对齐 II：树/标签、scoped models、设置、扩展 UI、自定义卡片、快捷键、主题、帮助 | PI-05~07、PI-11~15、§6.1 | T4 | 待开始 |
| T6 | 所有权迁移：Plan/Goal 移入 Pi 扩展；Bash 拆分；Workflow 决策 | EX-06~08 | T4 | 待开始 |
| T7 | 移除 stock profile，物理删除业务 crate，清零 external 分支 | END-01~03、PR-04、CB-01 | T2、T5、T6 | 待开始 |
| T8 | 终态验收、文档、发布 | END-01~09 | T7 | 待开始 |

T2 和 T3 可以并行（一个改依赖图，一个改 adapter），但要分别提交，冲突在 `event_loop.rs` 时以先合入者为准再变基。

---

## T0 治理切换

**任务**

1. remote：`git remote rename upstream grok-build`；`git remote set-url --push grok-build DISABLED`。（本地可逆配置，按本次总纲授权执行；不修改远程仓库。）
2. 新建 `docs/grok-build/REVIEWED`：写入 `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8` 和 Source-Revision `559751fd…`（最近一次已审阅的范围终点）。
3. `docs/upstream/UPSTREAM_CHANGELOG.md` 移到 `docs/grok-build/archive/`，新建 `docs/grok-build/REVIEW_LOG.md`（条目格式：范围、提交、逐条定性表 `移植/改造/跳过/观察`、理由、目标路径、移植提交）。
4. skill：`.pi/skills/upstream-changelog/` → `.pi/skills/grok-build-review/`，改写步骤：
   - `REVIEWED` 第一行是完整提交 SHA，第二行记录 Source-Revision；范围取第一行 SHA 到 `grok-build/main`，不得把整个文件直接作为 Git ref；
   - 按路径预分类：业务路径（shell、workspace、tools、agent、mcp、telemetry、login、auth、voice、memory、plugin-marketplace…）自动标"跳过"；UI 路径（pager、render、markdown、diff、mermaid、tty、textarea）标"待评估"；
   - 只写记录，不移植；更新 `REVIEWED` 需在移植评估完成之后。
5. AGENTS.md：
   - 删除 `base`、"Grok Build source is read-only"、"Upstream sync workflow" 整节；
   - 新增 "Grok Build reference review" 小节（GV-01~07）；
   - "Architecture invariants" 增加：Grok 业务不进入产品；验证以架构守卫为准；
   - 当前 SPEC/PLAN 指针改为本文件。
6. 验证体系：
   - `pi_dependency_profile.py`：禁止名单扩为 SPEC §5.3 移除类全部 crate，但当前仍链接的先列为 `pending`（报告、不失败），每切除一个就移到 `forbidden`；
   - 新增端点扫描脚本（VF-02），先以报告模式运行，记录现有命中作为基线；
   - `verify.sh` 去掉上游 blob identity / sourceguard / negative 步骤；`grok_uploaded_baseline_sha256.json` 与 `test_native_identity.py` 移到归档目录（用 `trash` 或 `git mv`，不 `rm`）；
   - 保留 syntax、mock 合同、依赖图、PTY。
7. `SOURCE_REV` 归档到 `docs/grok-build/archive/SOURCE_REV`（`git mv`），README 与 VERIFICATION 中的相关说明同步。
8. 核实 SPEC §5.3 分类：对每个 crate 做 `cargo tree -i` 与引用扫描，结果回写 SPEC。

**验证**：`./verify.sh`（新版）通过；`./scripts/cargo-shared.sh check -p xai-grok-pager-bin --bin grok-pi --no-default-features --features jemalloc,sandbox-enforce`；stock check 仍通过（T7 前保留）。

**退出条件**：AGENTS.md 不再出现"上游同步/只读"约束；`verify.sh` 中没有 blob identity；端点扫描基线和 crate 分类已记录。

**提交**：`docs: adopt Grok Build as a reference repository`、`build(verify): replace upstream identity with architecture guards`。

---

## T1 产品入口收口

**任务**

1. `event_loop.rs:1411` plugin CTA、`:1417` workspace dashboard：加 `!external_agent`。
2. `views/modal.rs`：命令面板过滤改为白名单函数 `external_palette_command_allowed`——允许的 `PaletteCommand` 变体逐个列出；`SlashCommand(text)` 只允许当前 `BuiltinCommandProfile` 白名单中的命令或 Pi 提供的命令；去掉 "Send Feedback"。
3. `slash/registry.rs:202`：Pi profile 下未知命令返回不支持，而不是 `Both`。
4. `dispatch/router.rs:1626` `OpenMemoryModal`：Pi profile 下提示不可用并返回空。
5. 全量扫描所有 `Action` 分支，确认 SPEC PR-03 列出的业务类 Action 在 Pi profile 下全部被拦截（产出一张 Action 清单写进回执）。
6. 5 个已知失败测试：逐个判断是测试预期过时还是 grok-pi 行为错误，修测试或修代码；回执写明每个的结论。
7. 每项补守卫测试（与 `external_profile_never_enables_or_dispatches_voice` 同处）。

**验证**：`./scripts/cargo-shared.sh test -p xai-grok-pager external_`（应全部通过）、grok-pi bin tests、PTY 产品入口 case、`verify_native_grok.py`。

**退出条件**：命令面板、斜杠、F2、Web catalog 全部为白名单；`external_` 测试 0 失败。

**提交**：`fix(tui): close remaining Grok product entries in the Pi profile`。

---

## T2 编译期切除 I

按以下顺序，每个 crate（或强耦合的一组）一次提交：

| 顺序 | 目标 | 做法 |
|---|---|---|
| 1 | `xai-grok-telemetry`、`xai-grok-otel`（含 sentry、mixpanel） | 在 Pager 内引入本地 `telemetry` 门面模块（空实现 + tracing），stock 下转发到原 crate；grok-pi bin 直接去掉依赖。`startup::enter`、`PendingStartup`、`session_ctx::log_event` 等调用点改走门面 |
| 2 | `xai-grok-voice` | 把 Pager 用到的 shared types 抽到 `xai-grok-shared`（或 Pager 内部模块），voice crate 变成 `stock-runtime` 可选依赖；Pager 的 voice 模块整体 `cfg(feature = "stock-runtime")` |
| 3 | `xai-grok-login`、`xai-grok-auth`、`xai-grok-secrets` | `AcpConnection.auth_manager` 改为 `cfg(stock-runtime)` 字段；`is_api_key_auth` 等 Grok 鉴权状态在 Pi profile 下不存在 |
| 4 | `xai-grok-announcements`、`xai-grok-feedback`、`xai-grok-gboom`、`xai-grok-dashboard-store` | 对应视图/Action/Effect 加 `cfg(stock-runtime)` |
| 5 | 待定类逐项决策：`foreign-sessions`、`active-sessions`、`sandbox`、`hooks` | 先在回执里写清楚决策和理由，再动手 |

每步：被切除的 crate 从 `pending` 移到 `forbidden`；删除对应的运行时 `external_agent` 分支（被 `cfg` 替代的那部分）。

**验证**（每次提交）：grok-pi check + bin tests；stock check；依赖图（包数下降并记录）；端点扫描命中数下降并记录。阶段末：`./build.sh`、`./verify.sh`、PTY。

**退出条件**：上表 1–4 全部在禁止名单中；端点扫描只剩 T7 才能清理的 stock 代码路径（需列出）。

---

## T3 Pi 协议对齐 I

**任务**

1. **PI-01 队列**：
   - 先用 PTY 录制当前队列行为（追加、编辑、取回、清空、中断）作为基线；
   - `pi_adapter` 把用户输入按 `streamingBehavior` 发给 Pi，队列显示改为只消费 `queue_update`；
   - `queue_bridge.rs` 中复制 Pi 队列的部分删除，只保留草稿/UI 关联；extension input interception 改走官方 `input` 事件；
   - Pager：Alt+Enter = follow-up，Alt+Up = 取回最后一条（`clear_queue` 返回内容回填 prompt），`set_steering_mode`/`set_follow_up_mode` 进 F2 Pi 页。
2. **PI-02**：运行态结束以 `agent_settled` 为准；`pi_lifecycle.rs` 测试补"扩展在 agent_end 后继续工作"的用例。
3. **PI-03**：斜杠目录合并 `get_commands`，按 `sourceInfo`（extension/skill/prompt）分组；reload ACK 与 package 操作后刷新。
4. **PI-08**：补 `summarization_retry_*` 事件映射与 `abort_retry` 入口。
5. **PI-09**：`!`/`!!` 前缀 → `bash` RPC（`excludeFromContext`）；`bash_execution_update` 流式到工具卡片；Esc → `abort_bash`；与 pi-grok-bash 扩展的卡片去重。
6. **PI-10**：`tool_projection` 识别 `exposure`/`namespace`/`annotations`/`outputSchema`；codemode、tool_search、MCP 调用的卡片标题与摘要。

**验证**：`./scripts/cargo-shared.sh test -p pi-grok-adapter`；`pi_disposition.rs`、`pi_lifecycle.rs` 新用例；mock 合同新增队列/bash/retry 事件；已安装 Pi fixture；PTY 队列 case 与基线对比。

**退出条件**：adapter 中不再有独立的 prompt 队列状态；PI-01/02/03/08/09/10 都有测试。

**提交**：按任务各一次。

---

## T4 扩展层整合

**任务**

1. 新建 `extensions/pi-grok-host-bridge/`（TS）和对应 injector `grok_pi/host_bridge_extension.rs`：
   - 命令/通道：`navigateTree`、`setLabel`、`reload`、`trust`、`exportJsonl`、`settingsTransaction`、`scopedModels`；
   - 统一回执格式（成功/失败/取消，带请求 id）；
   - injector 单测断言全部相对 import 被物化。
2. 合并：`tree_bridge`、`native_commands_extension`、`host_feature_extension`、`rpc_compat_extension` 中仍有必要的部分迁入 host-bridge，其余删除。
3. EX-02：逐个列出 `pi-grok-rpc-compat` 的 patch → 官方替代 / 删除 / 无法替代（写入矩阵）。目标：零 prototype patch、零 `process.stdout.write` 包装。
4. EX-03：`ctx.ui.custom` 统一到 remote-tui（或合并 rust-tui-bridge），另一个删除；DA-07/08 的 Remote TUI 测试保持通过。
5. EX-04/05：shortcut-manager 改用扩展事件，配置迁到 `$GROK_HOME/shortcuts.json`（读旧 `~/.pi` 位置一次作为迁移，不删除旧文件）；rollback/workflows 的文件交换改为 RPC/事件。

**验证**：每个扩展的 `bun test`；`pi -ne --mode rpc --extension <bundle>/index.ts` 独立加载 exit 0；adapter tests；bin tests；`pi-rpc-stderr.log` 无新错误；PTY tree/reload case。

**退出条件**：SPEC END-07 满足。

---

## T5 Pi 交互对齐 II

**任务**

1. §6.1 映射表逐行落地；无法提供的（`/import`、`/share`、`/bug` 等）在 FEATURE_MATRIX 写明原因。
2. PI-05：scoped models 编辑器（Provider → Model 勾选/排序），写 Pi settings 并 reload；F2/Web 同时编辑时先重读再写。
3. PI-06：扩展 UI 请求类型逐个 fixture（含 `timeout`、`setWidget` 位置、`setTitle`）。
4. PI-07：定义 `customType` 卡片注册表（todo、subagent、plan、goal、recap、btw…），live/resume 回放一致；未知类型使用通用卡片。
5. PI-11：快捷键表对齐 Pi；实现 `~/.pi/agent/keybindings.json` 可选读取；冲突解决规则写进 `/hotkeys`。
6. PI-12：可选 Pi 主题只读导入。
7. PI-15：教程/帮助/欢迎页重写：按 Pi 概念分章，标注"Pi 内置 / grok-pi 附带扩展"。

**验证**：bin tests（含 tutorial 合同）；PTY：模型选择、scoped models、树导航、快捷键、扩展对话框；`verify_native_grok.py` 白名单。

**退出条件**：SPEC END-05 满足；FEATURE_MATRIX 与 §6.1 一一对应。

---

## T6 所有权迁移

**任务**

1. Plan：新建/改造 `pi-grok-plan-mode` 扩展持有 Plan 状态（`appendEntry`），adapter `plan_mode.rs` 只做投影；Pager 的 Plan 循环改为发命令给扩展。
2. Goal/Loop：`pi-grok-goal`、`pi-grok-loop` 持有目标、提醒与续跑（`before_agent_start`/`agent_settled`）；`goal_host.rs`、`loop_host.rs` 退化为投影。
3. 已有会话的 Plan/Goal 状态：读取旧格式并迁移到 entry（只追加，不改写旧 JSONL）。
4. Bash（EX-07）：拆分 `pi-grok-bash/index.ts`；"默认覆盖内置 bash"改为 F2 可配置；移除私有空闲期 fallback。
5. Workflow（EX-08）：写决策记录（迁移到 Pi 扩展 vs 冻结为可选），用户确认后执行；决策前保持默认关闭。

**验证**：扩展 `bun test`；adapter tests；resume 旧会话 fixture；PTY plan/goal case。

**退出条件**：SPEC END-06 满足（Workflow 按决策结果）。

---

## T7 移除 stock profile，物理删除

**前置**：T2、T5、T6 完成；用户确认执行（此阶段不可逆程度最高）。

**任务**

1. 统计 Pager 测试归属：stock 专属 / 两者共用 / Pi 专属。stock 专属测试随功能删除，共用测试改为 Pi 行为。
2. 删除 `xai-grok-pager` stock 二进制和 `stock-runtime` feature；`grok-pi` 成为 `xai-grok-pager-bin` 唯一 bin。
3. 删除 `UiProfile`、`ExternalUiProfile`、`BuiltinCommandProfile::Grok`、`external_agent_active()`；按 PR-04 清零全部 external 分支（Pi 分支保留为唯一实现）。
4. 用 `git rm` 从 workspace 删除 SPEC §5.3 "移除"类 crate；更新 `Cargo.toml` workspace members 与 `Cargo.lock`。
5. 删除失去用途的 stock 资源（Grok 教程、Grok 欢迎页素材、Grok 设置项定义）。
6. 评估 crate 改名（GV-07），默认不改。
7. 依赖禁止名单中的 `pending` 清空；端点扫描改为强制失败模式。

**验证**：`./build.sh`；`./verify.sh`；全部 Pager/adapter/bin 测试；依赖图；端点扫描；PTY 全 case。

**退出条件**：SPEC END-01~04 满足。

**提交**：分多次（测试归属、profile 删除、crate 删除、资源清理），每次都能单独编译通过。

---

## T8 终态验收与发布

1. 逐项核对 SPEC END-01~09，写回执。
2. 文档：README、`docs/README.zh-CN.md`、FEATURE_MATRIX（中英）、NATIVE_GROK_TUI_ALIGNMENT、VERIFICATION（中英）、CHANGELOG。
3. 用户可见文案扫描（END-08）。
4. 手工验收清单：真实 provider 对话、OAuth 登录、树导航、队列、扩展对话框、Windows/Linux/macOS 各一轮（真实验收与开发验收分开记录）。
5. 发布由用户决定（版本号、是否 beta）。

---

## 回执

### 2026-10-03：接管总纲与 T0/T1

- 用户提供的总纲已原文保存并接受，提交 `d7ca4d0b`；完成依据是 END-01~09，前面的局部检查点不代表总工程完成。
- 本地 remote 改为 `grok-build`，push URL 为 `DISABLED`；origin 未改，未 fetch/merge/port/push。
- 审阅水位核对为 `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`，原提交 trailer 为 `559751fdcec02d413e4c57c8832ab275e4f44980`；记录、SOURCE_REV 和最终旧 identity 材料归档。新 skill 逐条决定移植/改造/跳过/观察。
- [逐 crate 清单](20261003-pi-native-tui-CRATES.md)覆盖 87 个实际 crate：56 linked、31 absent；移除类 15 个 linked，混用的 UI 契约须先抽取。T0 policy 将 29 个已链接业务/exporter 名字显式 pending；已缺席或新增业务依赖现在就拒绝，不能借通配 pending 混入。
- 17 项新架构/合同守卫与 negative exit 0；Rust 3397 文件语法解析通过。端点旧 artifact 基线：source 213、binary 52 主命中，protocol 1260 单列；`enforced:false/passed:null`，不是 END-04 通过。旧 SHA `210efc…`，没有用它冒称新源码 artifact。
- T1 [Action 清单](20261003-pi-native-tui-ACTIONS.md)：63 个 stock 产品 Action 声明受中央拒绝；环境变量不能开启 plugin/workspace/privacy；palette 与未知命令 fail-closed。可选 Workflow 只显示单 tab、仅发真实已有的请求，运行能力以 live catalogue 为准，不被旧 raw UI 配置否决。
- 完整 `external_` filter 最终 exit 0，60 passed/0 failed；`/tmp/grok-pi-native-t1-external-tests-final3-20261003.log`。先修 8 个新代码编译诊断及 1 个新增测试 target 类型诊断；首个实际运行 59/1 暴露 Execute fixture 初始 fold state，修 fixture 后 60/0。没有删掉原断言。
- 旧 5 fail：Ctrl+O 使用真实 hunks，并显式设置 excluded Execute 的初始 Expanded；dashboard 使用规范 glyph；stock 外部来源拒绝 native FTS，Pi PSM 保留 fresh/stale 校验与按 id/cwd 加载，新增对应测试。
- 新 `./verify.sh` 实际 exit 0：`/tmp/grok-pi-native-t0-t1-verify-20261003.log`。17 架构守卫、negative、3397 Rust 语法、8 mock/33lines、Pi/stock checks、adapter207+非ignored disposition1/reloadACK1/EOF2、bin98及2个声明native单测均通过；其余actual专项仍ignored。本次不是全部Pager suite验收。
- T0 提交 `bb968e41`，总纲接受提交 `d7ca4d0b`；T1 实现提交由 Git log 记录。治理/守卫与产品入口分别提交，T2~T8 仍待办。
- 正式 `./build.sh` exit0、28.02s；artifact SHA `3b1f03c49e96d70e95a0b6eb4de7bbc21495a3b2ec1129b7535ff4216013755f`、181635384bytes；`d7ca4d0b`+dirty冻结T1 source stamp，不冒称最终cleanHEAD。`/tmp/grok-pi-native-t1-build-20261003.log`、artifact JSON同名前缀。
- 新nativePTY 4cases（product-surface、settings-save/reopen/rollback）全部nativeExit0，SHA前后未变。product case强制voice/pluginCTA/workspaceDashboard/privacy rollout环境开关；F2产品项不可见、合法UI设置保留、保存/重开/失败回滚通过。`/tmp/grok-pi-native-t1-pty-20261003/report.json`。
- 新binary endpoint报告 source213/binary52、protocol1259单列，enforced:false/passed:null；`/tmp/grok-pi-native-t1-endpoints-20261003.json`。Dependency 796/forbidden=[]/pending29/terminalReady=false，不冒称 END-02/04。

T2~T8 尚未实施；队列/Plan/Goal/扩展整合、业务 crate 切除、唯一产品/零 external 分支等终态条件仍未满足。
