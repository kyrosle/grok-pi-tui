---
id: "2026-10-03-pi-deep-adaptation-spec"
title: "Pi 1.0 深度功能与运行语义适配 SPEC"
status: "accepted"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# Pi 1.0 深度适配 SPEC

用户要求：落实上一轮提出的 P0–P4，先写 SPEC、PLAN，再创建 goal 实施；沿用已授权的阶段本地 commit，不 push。起点为干净 `main@84174917`。上一轮 [SPEC](20261002-pi-first-tui-SPEC.md) / [PLAN](20261002-pi-first-tui-PLAN.md) 已完成，本文件记录增量工作。

## 所有权与约束

- Pi 是唯一 agent core，Grok Pager 是唯一终端界面；adapter 保持 headless/library-only。保留生产 no-stock profile、产品状态隔离及现有可选功能默认值。
- 复用当前 Pi executable / public SDK / extension API。Pi core、用户 session JSONL、真实凭据和业务数据不作实现改写；不重建包管理器或第二个终端 renderer。
- 不把任意 Pi TUI factory 兼容写成已经保证。官方 RPC 的 no-op/UI 限制、实验 shim 与可验证原生映射分别记录。减少私有补丁时不得静默移除现有兼容能力。
- 各操作与显示采用 Pi 的真实状态、作用域和生命周期。内部 RPC 不要求逐个暴露为用户按钮。

### 2026-10-03：用户明确的鉴权职责收敛

- 用户明确：功能和鉴权都由 Pi 负责，grok-pi 中重复维护的业务逻辑应裁剪。登录桥只选择 Pi 提供的 provider/method、调用 Pi ModelRuntime、映射原生交互及刷新显示，不直接读写 MCP/凭据配置。
- 删除桥内 Radius 专用入口及登录后的 MCP 配置写入。Radius 仍作为 Pi provider 通过通用选择器登录；MCP 配置交给 Pi 自身支持的配置入口，保留现有文件，不迁移/删除用户数据。
- grok-pi 的鉴权开发验收限于薄桥委托、prompt/notify、响应、取消/超时/会话撤销。Pi 自身真实 OAuth 全流程不作为本轮 TUI 开发交付前置；未执行的真人流程仍如实记录为 Pi/provider 集成体验待验。

## 功能与验收要求

| ID | 要求 | 验收 |
|---|---|---|
| DA-01 | 消费 Pi `prompt/steer/follow_up` 成功响应的 `disposition`，区分 `started/queued/handled`；保留事件先于 response 的并发和 extension 独立 work 语义；旧 host 的缺失字段可使用有界兼容 probe | input handler 消费、普通启动、Pi 排队、慢启动、取消/clear_queue、事件先到的定向测试与 actual Pi fixture；无 ghost running/waiter 泄漏 |
| DA-02 | 审计 retry/compaction 控制与显示，补原生自动 retry/compaction 状态、切换及 retry 取消入口；按照 Pi RPC/设置持久化语义 | 正式 RPC 命令和失败回执、运行中状态刷新、session switch/reload，已有 retry/compaction 事件展示保留 |
| DA-03 | 原生资源面板提供 package install/remove/update/update-all，使用当前 Pi 官方 CLI/SDK；执行状态/错误/取消原生呈现，保持 Global/Project/trust/agentDir/cwd | 临时 local/npm/git fixture 或 fake executable 证明参数与作用域；actual Pi local package 生命周期证明；裸 `pi update` 不得误用于更新包 |
| DA-04 | package 操作和启停后形成 reload 闭环；区分声明已保存、安装完成和运行态已加载；重读配置防止 Web/外部编辑覆盖 | 安装/移除/更新后实际 registry/commands 变化；reload 拒绝或加载错误不标成功；version pin/filter/project precedence/local remove 不删源目录 |
| DA-05 | virtual model 选择与每次 dispatched physical model/thinking 分开呈现；会话费用按实际模型保留；reload/resume/tree 后状态准确 | 合成 virtual provider 的实际 Pi 事件→ACP→native 状态；模型路由/usage/context 回归；未知字段不凭空推断 |
| DA-06 | 验收官方 `models.generateImages()` / `models.classify()` 管线，保留结果、费用、错误和取消及图片 live/replay；聊天 model picker 继续仅接受 chat | 公共 ModelRuntime/Codemode 合成实现的 actual SDK/RPC 测试；原生图片结果与会话成本检查；真实 provider 成功独立记录 |
| DA-07 | 形成可核对的 Pi extension UI 支持表和诊断；深化可映射的 working/status/widget/editor 等语义；factory/raw-input 兼容限定到实际验证范围 | 官方 RPC 方法、原生映射、实验 host、unsupported 分别测试；新增相对模块被 injector 全量物化；native Pi 不受 shim 干扰 |
| DA-08 | 隔离且缩小私有 Runner/ctx.mode 补丁范围，保留必要 Remote TUI 行为与焦点/键盘/resize/dispose/cancel | 既有 Remote TUI tests、代表性第三方/custom fixture 和真实 PTY；关闭 shim 路径可独立启动；不承诺所有第三方组件兼容 |
| DA-09 | 更新逐项能力/证据矩阵、双语说明与精确当前源码声明，保持三层 identity，无目录豁免 | source guard/negative/syntax/mock、依赖图七禁止包为空、相关 Pi/stock check、生产 build 与组合 verify |
| DA-10 | 对照 Pi 与 grok-pi 的交互合同，验证原生 UI 映射、响应、取消、resize 和生命周期；真实 provider/OAuth/image/目标终端流程作为补充体验记录 | 用户明确 Pi 全部负责功能与鉴权后，开发完成以桥层和原生组件证据为准。未执行的真人流程如实 pending，不以 fixture 冒充，也不再作为本轮 TUI 开发 goal 的关闭前置 |

## 产品行为细节

- Package 更新使用 `pi update <source>` / `pi update --extensions`；install/remove 的项目声明使用 `--local`。官方 SDK 按 identity 更新多个作用域时 UI 不标成仅当前作用域。
- local package remove 仅解除声明，不删除用户目录。保留 npm/git version/tag/commit pin、对象 filters、autoload 与资源 delta。操作成功后重新加载清单，再经官方 reload 确认效果。
- Pi 配置与 Grok 外观配置的归属不变；项目信任和 CLI 工具/资源限制优先于开关。已有用户改动必须保留。
- 标准 UI 优先使用原有 QuestionView/toast/status/widget/PromptWidget/card；无法通过标准 RPC 表达的方法可经现有 extension bridge 适配，但必须显式标注其兼容范围。
- 真实模型调用和 OAuth 使用用户选择的 provider/账号与可检查的验收步骤。不得输出 token、自动完成真人 consent 或发布/分享测试会话。

## 交付

[PLAN](20261003-pi-deep-adaptation-PLAN.md) 记录每阶段代码、命令、结果与剩余项。按本次用户明确的 TUI/Pi 所有权澄清，开发完成条件为 DA-01–09 与 DA-10 的交互桥验收通过、重复鉴权配置逻辑裁剪并验证；真实 provider/OAuth/image/目标终端体验作为独立补充记录，不冒称已通过，不再阻塞本轮开发 goal。这是用户后续范围澄清，取代先前要求全部真人流程通过或明确延期才能关闭的条件。
