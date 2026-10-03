---
id: "2026-10-03-pi-product-surface-plan"
title: "面向 Pi 的 Grok 风格 TUI：产品入口裁剪 PLAN"
status: "in-progress"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# 产品入口裁剪 PLAN

依据 [SPEC](20261003-pi-product-surface-SPEC.md)，从干净 `main@13697f9c` 实施；本地 commit，不 push。代码写入分文件并行，Cargo/build 由 Root 单一执行，来源声明在源码冻结后更新。

| 阶段 | 工作 | 状态 |
|---|---|---|
| P0 | 删除 external voice 能力与 Grok AuthManager 创建；保护 stock profile | 进行中 |
| P1 | 收敛 external settings/目录/操作，裁剪 Grok 产品入口 | 进行中 |
| P2 | 参考 Pi interactive 支持，统一帮助、教程和双语文档 | 待完成 |
| P3 | 定向回归、两种 profile、生产 build、原生 PTY、精确来源及本地提交 | 待完成 |

## 执行记录

- 当前 main、工作树干净、HEAD `13697f9c`。前轮只读审计核对了安装 Pi 1.0.0 的公开 API。
- Voice 入口可达 Grok AuthManager/xAI STT；F2 coding_data_sharing 仍发送无 Pi handler 的 stock 请求。先处理明确产品残留，再核对同类入口，不删除用户配置或上游 stock 源码。

## 单独保留的所有权债务

- adapter-owned queue 与 extension input interception：应交 Pi 官方队列；当前 stable-id 编辑能力没有官方 RPC 等价物，迁移需保留未提交草稿边界。
- Plan/Goal：状态、提醒与续跑仍有 adapter authority，后续应搬入 Pi extension；Pi 1.0 没有等价内置 Plan/Goal。
- 可选 Workflow：Grok Rhai runtime 仍负责编排，Pi SDK 负责 worker；迁移需处理已有 `.rhai` 与暂停/取消/恢复语义。
- 增强 Bash/Eval：属于 Pi 扩展能力，但默认覆盖和私有 off-turn fallback 仍需进一步解耦。

这些条目不作为已完成迁移报告；本轮用户明确删除的 voice 及 Grok 产品入口必须实际裁剪并验证。
