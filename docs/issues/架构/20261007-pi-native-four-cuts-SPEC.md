---
id: "2026-10-07-pi-native-four-cuts-spec"
title: "Pi 原生 TUI：四组重复实现裁撤 SPEC"
status: "accepted"
created: "2026-10-07"
category: "architecture"
---

# Pi 原生 TUI：四组重复实现裁撤

用户授权：按审计顺序创建 SPEC、PLAN，以 Goal 落实四组裁撤。起点 clean `main@646abebbd80f8856a1baa05cbd39a0bcec81cff5`，系统 Pi 1.0.4，已安装 pig 为已发布 v0.1.10。本增量服从 [总体 SPEC](20261003-pi-native-tui-SPEC.md)，不宣称总体 T2–T8 完成。

## 范围与顺序

1. 移除 grok-pi 自建 Eval v1/v2：Node/Python kernel、后台 Eval、工具桥、Eval MCP/Tokenizer、注入源与运行环境变量、激活开关及 Eval-only 策略、模型提示中的 Eval 路由、F2/Web/slash 操作入口。
2. 移除默认关闭的实验性 `pi-grok-native-commands` 及 Rust injector、启动开关。保留正式原生 `/model`、`/resume`、`/reload` 等入口。
3. 移除默认关闭的第二套 `pi-grok-rust-tui-bridge` 及 injector/开关。现有 `pi-grok-remote-tui` 为唯一保留的 custom-component 宿主。
4. 用 `trash` 删除没有产品注入入口的 `pi-grok-00-profiler`，不新建生产替代工具。

## 保留边界

- 增强 Bash 的前台/后台执行、task_name、超时、自动转后台、进程树清理、get_task_output/wait_tasks/kill_task 和原生任务面板保留。
- Pi 官方 `builtin:codemode`、`builtin:mcp`、`builtin:tool-search` 的加载及 CLI 权威限制保留；不强制用户只用 Codemode。
- 历史 Eval 消息继续可读。保留必要的旧载荷解析和原生卡片渲染，不加载历史执行器、不改写 Pi JSONL。
- 不改 Pi 源码，不删除用户配置、凭据、会话、已安装共享 npm 依赖。旧配置中 Eval 字段可被忽略；新设置目录不再提供这些字段。
- 不裁撤增强 Bash、Todo/Subagents/Plan/Goal/Loop/Workflows、现有 Remote TUI、auth/export/config 等必要桥接；队列、业务 crate 和私有 hooks 的其他债务不扩大进本增量。
- 四组裁撤最初仅授权当前 checkout 本地编辑、检查、构建。2026-10-07 用户后续明确授权覆盖本机 pig：备份旧安装、安装本次本地构建并验证，保留用户状态。不 commit、push、创建 worktree或发布新 Release。

## 验收

| ID | 条件 |
|---|---|
| CUT-01 | 新会话无 grok-pi Eval 工具、Eval kernel/MCP/source 注入；遗留环境变量或 TOML 不重新启用 Eval |
| CUT-02 | 增强 Bash 独立 bundle 相对导入闭包完整，可在真实 Pi RPC 加载；前台/后台/输出/取消回归通过 |
| CUT-03 | F2、Web、命令菜单、帮助、当前教程不提供 Eval 或退役实验入口；历史记录可保留明确标注 |
| CUT-04 | Pi Codemode 的工具执行与原生呈现继续有效；历史 Eval live/replay 解析的必要兼容保留 |
| CUT-05 | 退役三组扩展和两个 injector 物理移除，主入口没有对应开关/加载分支；只有一个保留的 custom UI 宿主 |
| CUT-06 | 更新中英 README、FEATURE_MATRIX、VERIFICATION、Changelog 和总体 PLAN 的子项登记 |
| CUT-07 | 聚焦检查、生产 Pi build/check、stock check、依赖/架构 guards、原生 PTY 和 diff 检查有实际退出码与记录 |

实现与阶段回执见 [PLAN](20261007-pi-native-four-cuts-PLAN.md)。移除自建 Eval 是所有权收敛，不是此前高 CPU 的已证实修复；另一台 Mac、真人 OAuth/provider 调用仍独立验收。
