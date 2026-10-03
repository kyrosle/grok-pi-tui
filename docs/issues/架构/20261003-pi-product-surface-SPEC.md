---
id: "2026-10-03-pi-product-surface-spec"
title: "面向 Pi 的 Grok 风格 TUI：产品入口裁剪 SPEC"
status: "accepted"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# 面向 Pi 的 Grok 风格 TUI

用户明确 voice 不需要，应删除 Grok 产品相关能力；产品定位由 Grok Build TUI 转为 Grok Build-style TUI for Pi。起点为干净 `main@13697f9c`，沿用分阶段本地 commit 授权，不 push。

## 要求

- PS-01：grok-pi 不提供 voice/STT/TTS 命令、快捷键、F2 设置或欢迎/教程入口，不为外部 Pi connection 创建 Grok AuthManager。Pi production 不启用 microphone audio feature；共享 voice types 保留，stock-runtime 继续启用 audio。已有配置和凭据不删除、不迁移；stock Grok 构建保留自身能力。
- PS-02：共享 external 设置目录仅暴露通用终端 UI、已接 Pi 的控制和 Pi 扩展功能；裁剪 SpaceXAI 训练/retention、Grok account/billing、stock agent/plugin/MCP 等产品入口。过滤必须覆盖 F2、palette 与 Web 配置目录，不能只隐藏单个页面。
- PS-03：参考安装 Pi 1.0 的 interactive/RPC/extension 支持与命令组织，统一产品文案及帮助。登录、模型、会话、资源、工具、retry/compaction 以 Pi 为权威，原生 Pager 负责呈现；额外 Pi SDK 扩展如 Subagents/Todo/Plan/Goal 明确其扩展性质，不冒称 Pi 内置功能。
- PS-04：保留 stock 默认 profile 和 Pi production no-default profile。精确逐文件更新来源声明，不改 Pi core、不新增 renderer、不宽免 source verifier、不修改用户业务数据。

## 验收

相关 settings/profile/connection 测试证明：external 无 voice 与 stock 产品设置，保留 Pi 和通用 UI 设置，stock 行为保留；Pi production binary tests/check/build、stock check、依赖图禁止七个 stock runtime 及 `cpal` / `alsa-sys` / `coreaudio-rs` / `coreaudio-sys` microphone backend；sourceguard/negative/syntax 按变更运行。原生 PTY 使用临时配置验证 slash/F2 目录及退出；不调用真实 STT、OAuth、模型或账号服务。

本次裁剪针对 Grok 产品入口与服务绑定。上一轮审计发现的队列/Plan/Goal/Workflow 所有权债务单独记入 PLAN；它们不是现成 Pi RPC 等价物，不能通过隐藏 UI 冒称已迁移。
