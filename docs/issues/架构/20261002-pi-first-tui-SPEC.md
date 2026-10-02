---
id: "2026-10-02-pi-first-tui-spec"
title: "Grok 原生 TUI 吸收与 Pi 1.0 功能适配 Spec"
status: "accepted"
created: "2026-10-02"
updated: "2026-10-02"
category: "architecture"
---

# Grok 原生 TUI × Pi 1.0：Spec

用户授权：Grok Build 始终作为 TUI 来源吸收，功能始终按 Pi 语义适配；先完成 spec、plan，再以 goal 落实。本文件是本次实施的范围和验收依据，执行状态记录于 [PLAN](20261002-pi-first-tui-PLAN.md)。

## 产品与所有权

1. Grok Pager / Pager Render / Minimal / Markdown / Diff 是唯一终端界面。沿用其输入、渲染、QuestionView、工具卡片、图片、任务、会话 picker 和 dashboard 组件。
2. Pi 是唯一 agent core：模型、鉴权、上下文、会话、树、compaction、工具注册/调用、扩展、MCP 生命周期与权限语义属于 Pi。grok-pi 自有能力通过官方扩展 API 实现。
3. `pi-grok-adapter` 保持无终端依赖的库，承担 RPC ↔ ACP 和必要的交互投影。能力控制和执行不能落入第二个 agent runtime。
4. Grok 产品后端不因 TUI 更新而自动进入产品功能：云账号/会话、采样器、插件市场、memory dream、workspace/Remote Control/Agent Host 不作为 Pi 功能实现。
5. 保留现有 Voice、Mermaid、Eval、后台任务、Subagents、Workflow、Plan、Rollback 与配置状态。默认关闭的实验能力不在本次默认开启。已有真人/真实模型验收缺口继续标记。

## 已核对基线

- 工作目录：`/Users/kyros/WorkStation/grok-pi-tui`；实施起点 `main@222d614d`，工作树干净。实际 origin 为 `Dwsy/grok-pi-tui`。
- 系统 Pi `1.0.0`；采用 Pi 1.0 作为本次实现基线，不修改 Pi core。可选 `pi-main` 子模块未初始化，运行和契约核查使用实际系统包。
- Grok 完整已吸收基线 `37949780`，`SOURCE_REV=c4ea71cfdbcdb21e32e41bc25a0043d7d4836714`。调查时上游 tip 为 `2bdd1d6a`，待审阅 7 个 sync commit；选择性导入不等于整仓同步。
- 当前生产 normal/build 图：adapter 909、composition package 1013 个唯一 package；计数包含根 package，是依赖指标，不是耗时或体积收益。
- 起点存在 `replay.rs` 漏导入导致的 E0425。Verifier 的旧哈希、只扫描顶层 Rust 文件、Pi 源码路径等假设失效；不能把这些失效检查作为运行通过。

## 功能要求

| ID | 要求 | 验收 |
|---|---|---|
| FR-01 | grok-pi 的版本门禁、文档与检查统一使用 Pi 1.0；支持系统安装与 `--pi-bin` / `PI_BIN` | 版本测试、实际 Pi RPC 启动、正式构建 |
| FR-02 | Eval v2 从工具执行上下文使用官方 `ctx.executeTool()`；正常工具路径不安装私有 Runner capture、不重建工具或手调 hooks。外部 Eval MCP 没有工具执行上下文，显式开启时暂保留隔离的旧调用兼容边界 | 核心/扩展嵌套调用、参数错误、权限阻断、abort、并发和 background 检查；工具只执行一次；不伪造外部入口的 assistant message |
| FR-03 | Pi 内置 MCP 显式 opt-in，沿用现有资源 allowlist 和 Pi project trust；Pi 管理连接、OAuth、工具/资源注册 | 隔离本地 MCP server smoke；禁用时不连接；CLI 限制和 hidden exposure 不被绕过 |
| FR-04 | 正确区分 active、registered、callable 工具；Eval-only 顶层仍仅 Eval；阻止 Eval/Codemode/搜索工具递归编排 | 普通模式与 Eval-only 的实际工具目录与调用检查；持久会话加载后政策一致 |
| FR-05 | Codemode 图片沿用原生图片渲染/打开机制；保留 text、image、resource 和 nested-call 结构，live/replay 一致 | 图片载荷投影测试、原生组件检查和隔离 PTY smoke；不把 base64 变成普通文本 |
| FR-06 | 鉴权按 Pi ModelRuntime 适配，支持 Anthropic 复制码和 Radius；原生对话框承接 prompt/notify，核心路径逐步脱离 Pi 私有 TUI 组件 | 无凭据交互/取消检查；实际账户授权不作为自动测试；Radius MCP 配置只有用户在产品内选择后写入 |
| FR-07 | 取消使用 Pi `clear_queue` 清除外部残留，并保留本地可编辑队列的现有调度语义 | 两类队列取消、settle、重开 dispatch 检查；单条编辑/删除行为保持 |
| FR-08 | 配置、host-feature manifest、UI DTO/纯 helper 下沉到已有合适的轻量层；Workflow 的 Pi 必需契约与 Shell persistence 解耦 | 原入口兼容 re-export；配置 overlay/隔离检查；Workflow 生命周期和存储检查 |
| FR-09 | 建立真实编译隔离，逐项断开 grok-pi 不使用的 Grok 产品 runtime；保留 stock 默认路径用于共享源码回归 | `cargo tree` 证明选定依赖不再可达；Pi 与 stock build/check 通过；收益实测 |
| FR-10 | 上游按可适配的 TUI 功能组吸收，先记录 Changes，再迁移代码及必要依赖；无 Pi 语义的控件不暴露 | 每组有来源 SHA、采用/不采用原因、对应文件和验证；共享终端/输入/图像/渲染修复优先 |
| FR-11 | 来源和协议 verifier 精确检查保留的原生源码、声明接缝、递归 Rust 模块和实际 Pi 契约 | 不以目录级忽略、无条件成功或广泛放宽 allowlist 消除失败 |

## 裁剪与更新约束

- 先断依赖边，后依据可达图清理 workspace 成员。保留既有 native UI，不复制出第二套 frontend 或通用插件框架。
- F2 可选能力与编译 feature 分开：默认发行包里保留的功能仍能通过 F2 开启，不能留下启用后不可用的开关。
- 本地日志、trust、权限、回滚、原子配置写入、终端恢复不因裁剪削弱。保留 `~/.grok-pi` / `.grok-pi` 隔离、Pi session-root 与 active-leaf 语义。
- 不删除/迁移用户业务数据，不写真实 Pi/Grok 凭据或用户配置来完成自动验证。
- 用户于本阶段明确授权边实施边作本地 commit；各提交须有可检查范围与验证记录。仍不 push、创建 worktree 或直接 merge 上游 root commit；不修改 Pi core 或真实用户凭据/业务数据。
- 选择性 TUI 导入单独记录来源；`SOURCE_REV` 和完整 upstream base 不伪装为最新全仓基线。

## 交付与证据

每阶段交付可检查 diff、执行命令、退出码、结果及未验证边界。完成标准为功能与依赖要求落实、相关自动检查和生产构建通过、文档一致；真实模型推理、真实 OAuth 和不同终端的人工体验分别保留其证据级别，不以静态/fixture 成功替代。

已有分析继续复用：[依赖断连](20260823-grok-pi产品依赖断连分析.md)、[扩展兼容层债务](20260823-审计%20Pi%20扩展架构边界与兼容层债务.md)、[Eval/MCP](../adapter/20260930-Eval%20v2%20学习%20Pi%20Codemode%20并复用%20MCP.md)。
