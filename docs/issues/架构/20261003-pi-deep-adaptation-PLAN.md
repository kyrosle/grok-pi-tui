---
id: "2026-10-03-pi-deep-adaptation-plan"
title: "Pi 1.0 深度适配 PLAN"
status: "in-progress"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# Pi 深度适配执行 PLAN

依据：[SPEC](20261003-pi-deep-adaptation-SPEC.md)。起点干净 `main@84174917`，实际系统 Pi 1.0.0。授权本地实现和分阶段 commit，不 push/worktree，不改 Pi core 或真实业务数据。

| 阶段 | 要求 | 执行与验收 | 状态 |
|---|---|---|---|
| P0 | DA-01/02 | 生命周期 response/event 竞态、队列 disposition；retry/compaction 控制和原生状态；adapter focused tests、actual Pi 与入口 check | 待实施 |
| P1 | DA-03/04 | 原生 package 操作→Pi 官方 CLI/SDK→重读→reload→registry；作用域/信任/错误/取消；fake CLI + actual local package + PTY | 待实施 |
| P2 | DA-05/06 | selected/dispatched model、费用；public image/classifier actual管线与原生输出；模型/session回归和运行证明 | 待实施 |
| P3 | DA-07/08 | 标准/实验/unsupported UI 能力表，原生映射和私有补丁收敛；Remote TUI回归、代表custom fixture与PTY | 待实施 |
| P4 | DA-09/10 | 精确 identity/docs、相关 Pi/stock checks、build、verify、对照 fixture；真实provider/OAuth/终端验收 | 待实施 |

## 执行约束

- root 维护 SPEC/PLAN、提交和唯一 Cargo runner；子代理按文件范围并行。独立读取可并行，Cargo/构建与重叠写入按依赖顺序运行。
- 先复用已有 adapter/tests、native modal/action、extension transport 和 Pi APIs；不创建通用兼容框架、包管理器或第二 renderer。
- 每完成阶段记录实际 exit、log、proof layer 和 commit。现有 664-phase 身份声明保持上一轮证据；新增/变动源码逐文件审阅声明，不覆盖历史源基线。
- 使用共享 Cargo 输出和既有 20GiB free floor；只在失败或新增修改需要时重验。
- 真实模型与 OAuth 的用户选择不阻塞独立实现；真人 consent 不自动代办。必要真实验收缺失时列出具体步骤，不能把 synthetic PASS 当成真实 PASS。

## 日志

### 2026-10-03：SPEC / PLAN

- 已确认工作树干净、branch main、HEAD84174917；上一轮 goal 已完成，本轮创建独立增量 goal。
- 已调查 lifecycle 丢弃成功 disposition、资源面板缺 package 生命周期闭环、virtual dispatched metadata/非chat模型 proof、RPC UI no-op和现有实验 host 私有补丁。
- 先实施 P0；P1、P2 的独立模块并行准备。真实验收环境与账号选择在可检查的验收脚本就绪后向用户确认。
