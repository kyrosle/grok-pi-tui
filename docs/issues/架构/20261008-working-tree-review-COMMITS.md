---
id: "2026-10-08-working-tree-review-commits"
title: "当前改动审查与分批提交"
status: "complete"
created: "2026-10-08"
category: "architecture"
---

# 审查与提交记录

用户授权审查当前完整 working tree 并分批本地 commit。起点 `main@646abebbd80f8856a1baa05cbd39a0bcec81cff5`，index 为空；不 push、不发布新版本。

按依赖顺序拆分：四组重复运行时裁撤 → 可选 Durable 后端 → Pi 1.1 契约适配 → 原生 TUI 改进与终端状态 → 文档及参考检查水位。重叠文件通过 index 暂存相应阶段内容，保留 working tree 的完整后续改动。

重点审查：启动/退出进程、CLI 工具限制、Durable store/恢复/订阅生命周期、ACP 事件顺序、原生 viewer 与终端状态、打包许可及当前能力说明。沿用已有分层验收，新增修复运行对应回归；不把 synthetic provider 或 PTY 写成真人模型/OAuth 验收。

## 审查发现与验证

| 级别 | 发现 | 修复与实际回归 |
|---|---|---|
| P1 | Durable attach 在验证目标前 detach/close 当前 core；失败目标还会留下已关闭的全局 HTTP dispatcher。 | 先成功打开目标，再替换当前 core；失败时恢复 dispatcher。测试不存在的 conversation、store 和已打开目标内的无效 conversation，确认原订阅、locator 和 HTTP transport 保留。 |
| P1 | 后台 owner 可以转到其他 store，而 Unix socket 仍代表旧 store。 | 后台 owner 固定一个 store；跨 store attach 明确拒绝，重启时选择目标 owner。真实 socket-owner 回归通过。 |
| P1 | `--exclude-tools` 单独使用和已有 conversation 的 CLI/F2 工具限制可能被忽略；F2 内建工具偏好还会顺带移除 Subagent。 | 在 official conversation.configure 应用工具选择和排除，包括 resume；F2 的四个内建工具偏好保留独立 Subagent。重开 SQLite 后 bash 被排除、Subagent 保留及 no-tools 回归通过。 |
| P1 | abort recovery 只取消当前 conversation，其他 owner 的中断任务随后会继续。 | 用官方 inspect/abortSubmission/abortTask 在 resume 前标记整个 store 的任务；完成选择后清除旧 recovery 提示。重开另一 conversation 后 abort 原会话的中断工具，调用次数仍为一次。 |
| P1 | attach/new 在 adapter 接纳新 locator 前推送 snapshot，跨会话 replay 会被过滤。 | 先返回新 boot，adapter 更新 locator 后再 watch，并清理旧显示状态。真实 SDK→ACP 回归检查切回旧 conversation 的回答和工具结果完整出现。 |
| P2 | Durable `message_end` 的 assistant 错误被去重路径绕过，终端状态可能被报为成功。 | 在最终 assistant event 上投影语义状态，保留原生 status handler；native error/status 投影检查通过，真人 provider 错误仍属于手工验收边界。 |
| P2 | 裁撤 Eval 后，历史 export/fullscreen 仍有失效的 effects-first 分支，测试留下互相矛盾的断言。 | 移除死分支，保留历史摘要、源码与 viewer。相关历史检查3/0，Eval renderer8/0。 |
| 文档 | 架构说明仍把历史 byte-identity 当作当前源码约束，参考审阅仍写所有采用 pending。 | 改为当前 architecture guards；保留历史回执，登记实际采用 commit，说明已安装 pig 未包含本轮修复。 |

## 本地提交

| 批次 | Commit | 范围 |
|---|---|---|
| 1 | `88db6876a366fd56de387953dec91d19a0a369c0` | 四组重复运行时裁撤、Bash-only、历史 Eval 保留及对应 guards/测试 |
| 2 | `459d8c6e5fb7bdd5d073d3f5601129d574393265` | 可选官方 Durable SDK 1.1.0、F2/CLI、独立存储/恢复/owner、打包及本轮修复 |
| 3 | `8244ed369c78802dec7f9e6d5ca6f6d46fab850c` | Pi 1.1 工具/取消/耗时/价格契约、Remote cursor、登录 agentName |
| 4 | `f21287bff` | 改造移植 Dwsy 的 native viewer/trace/duration、OSC7501 与生命周期 |
| 5 | 本记录所在的后续 docs commit | README/矩阵/验证/总纲与参考审阅水位；前四批完整 SHA 也登记在 `docs/upstream/REVIEWED.json` |

重叠文件在 index 分阶段暂存，working tree 的完整后续内容保留。前几批在整理过程中调整过暂存边界和本次新建本地 commit 的父链；最终 SHA 以上表及 Git history 为准，没有改写任务开始前的历史或远端。

## 验证回执

- SDK14/0，日志 `/tmp/grok-pi-review-sdk-20261008.log`；Bash-only 四模块和 test harness 在临时目录使用实际公开 SDK，exit0。直接从仓库根运行 Node 最初因不具备根级 SDK module resolution 被拒，按 injector import closure 物化后通过。
- Adapter208/0 + non-ignored integration4/0；真实 Durable SDK→ACP2/0，日志 `/tmp/grok-pi-review-adapter-full-20261008.log`、`/tmp/grok-pi-review-durable-acp-20261008.log`。新 replay 回归使用有界等待实际 ACP 帧，不假设 RPC 返回时 UI 已消化全部帧。
- Production bin92/0；Bun auth/Remote TUI21/0、Web language2/0；native historical3/0、Eval8/0、Codemode15/0、transcript18/0、F2 Durable1/0、duration/trace/status各1/0。
- `./build.sh` exit0（49.34s），stock Pager check exit0；架构17/17、four-cut closure、negative guards和 dependency profile通过。pending29、terminalReady=false，未将过渡债务写成终态。
- 生产 debug binary SHA256 `eed8e5c7592be358d43444b872f68f4d6abc12aa3c17ce548cd558aaa2285567`。Classic PTY3/3全部 nativeExit0，日志 `/tmp/grok-pi-review-classic-pty-20261008.log`；首次因未提供 fresh-binary SHA 被 guard 拒绝，计算实际 SHA 后再执行通过。Durable live/reopen/F2/模拟OSC通过，日志 `/tmp/grok-pi-review-durable-pty-20261008.log`。
- Cargo 20 GiB free-space floor保持。扩大编译遇到空间守卫拒绝时仅清理生成的 Cargo package caches，随后检查通过；未清理源文件、业务数据或用户会话。没有跑完整10172个 native tests。
- Rust syntax3399/3399、failures0，报告 `/tmp/grok-pi-review-rust-syntax-20261008.json`。本地 Markdown 引用196项通过；`AGENTS.md`指向可选 `pi-main/AGENTS.md` 的一项依赖未初始化 submodule，保留此既有条件，没有改 Pi 源码。Diff/最终状态提交前复核。本轮没有重建优化分发包或覆盖 installed pig；公开v0.1.10和原 installed SHA保持。

未验证边界：真实 provider/OAuth、断电耐久性和真实终端晚到 reply/typeahead/跨终端体验；Durable 普通插件/MCP/Codemode/images/Plan/Goal/Loop等仍按 capability禁用。Expert Council inspect 的模型评估缺失；本次由 Main Agent完成审查，没有使用未评估的默认委派。
