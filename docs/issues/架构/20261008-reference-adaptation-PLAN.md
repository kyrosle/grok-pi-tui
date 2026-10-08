# 2026-10-08 参考审阅建议落实

依据：[三仓库审阅](../../upstream/20261008-pi-dwsy-grok-build-REVIEW.md) 与 [固定 SHA](../../upstream/REVIEWED.json)。引入阶段以 `main@646abebbd80f8856a1baa05cbd39a0bcec81cff5` 的 dirty tree 为实现基准，保留四组裁撤和 Durable 开发内容。不整仓 merge，不恢复自有 Eval，不改 Pi 源码或用户会话数据。后续用户授权 review 并分批本地 commit，见 [审查提交记录](20261008-working-tree-review-COMMITS.md)；未 push。

## 验收范围

| 阶段 | 实现与验收 | 状态 |
|---|---|---|
| A1 | 按 Pi 的 `--tools +name/-name` 语义判断桥接 admission；no-tools/exclude 仍优先，F2 不重写 CLI。 | complete, scoped development verification |
| A2 | `agent_settled.aborted` 映射 Cancelled，取消不自动恢复队列/Goal；兼容旧 Pi 缺字段。 | complete, scoped development verification |
| A3 | 改造移植 Dwsy `9a00b24e451ad3a22d688f7dd80711818e157261`（Codemode viewer / 回放耗时 / 小数 duration）和 `c6a4ff4cbf91d681ccb668d55fadfe21301f3523`（trace hint）；消费 Pi 官方 `durationMs` 和 Durable task 时刻，区别执行耗时与 task 墙钟跨度，旧数据可缺字段。 | complete, scoped development verification |
| A4 | Remote TUI 解析最新 fake-cursor marker，继续使用现有 native highlight；验证旧/新 Input、边界列、overlay/focus。 | complete, scoped development verification |
| A5 | Durable 的四个 Pi SDK 包一起升级 1.1.0、更新 lock 与 bundle；用临时 store 验证 SDK、旧版本 store 重开与恢复、取消/子任务和打包。不触碰用户 SQLite，不重写官方缓存/调度。 | complete, scoped development verification |
| A6 | 保留并显示 `cost.tiers`，实际费用继续来自 Pi usage；OpenAI 登录使用官方 `agentName`。 | complete, scoped development verification |
| A7 | 在现有 native terminal probe/lifecycle 支持 OSC 7501，自动探测/显式开关、状态去重、取消与退出清理；不发送用户 prompt/回答内容。 | complete, scoped development verification |
| A8 | 完成相关 Rust/Bun/SDK 检查、Pi 生产构建、原生 PTY/依赖守卫；更新 README / Feature Matrix / Verification / 审阅 adoption 记录。 | complete, scoped development verification |

## 验证策略

- 使用项目 `scripts/cargo-shared.sh`；编译共享输出只由 Main Agent 调度。
- 先跑改动路径的测试，再 adapter 与 composition 生产 profile 检查、默认 Pager 检查、`./build.sh`；有新增失败再扩大验证。
- SDK fixture 与测试会话只在临时目录，provider 用 synthetic/faux；模型/OAuth 真人体验与真实终端支持仍是独立证据。
- 三仓库 reviewed SHA 保留，adoption 更新与新提交审阅分别记录。移植来自 Dwsy，Grok Build 本轮无新提交。

## 执行记录

- 2026-10-08：开始 A1–A8。Dwsy 原始 patch 对本地四组裁撤后的 `scrollback/block.rs` 测试上下文不直接适用，按当前代码逐段移植。

- 2026-10-08：A1–A7 源码已落实。Adapter 208/0、生产 binary 92/0、Bun 23/0、SDK 11/0、Codemode native 14/0、额外原生耗时/trace/status 各 1/0、terminal parser 1/0；Durable ACP 2/0。最终旧标题替换回归 1/0。
- SDK 1.1.0 实际重开先由真实 1.0.4 SDK 创建的临时 SQLite；稳定 requestId/submission 身份与原 manifest 保持，用户 store 未触碰。另验证分页 `next` cursor 方向约束、task timestamps 与工具 durationMs。
- Debug 构建与默认 stock Pager check exit0。最终 debug SHA `c218255284102ed1046f8acced850914e3244f9962ecafb6a94b48252bd576db`。Classic native PTY 3/3 见 `/tmp/grok-pi-reference-classic-pty-final-20261008/report.json`；Durable live/reopen/F2 与模拟支持终端的 OSC query→working/done/clear 均通过，证据 `/var/folders/lh/z16j0yfx541cm_wsg95lm92c0000gn/T/grok-pi-durable-pty-jlJkom`。
- 初次 Durable PTY 的恢复用例把历史文本当成新回复、提前发 exit；已改成等待回放完成和唯一新回复。OSC 最初 done 被 idle-title assignment 覆盖；完成状态改在原标题/进度替换之后排队，保留原 stale-title 回归。
- 磁盘守卫未降低：先用项目工具清理生成的 incremental roots，再对 `xai-grok-pager` 做定向 `cargo clean -p`。未清理源代码、用户业务数据、会话或配置。后续 Cargo `CARGO_INCREMENTAL=0`；默认 20 GiB 守卫保留。
- Architecture guard 17/17 与 four-cut closure PASS；依赖图 796，forbidden/stock runtime/audio 空，pending29、terminalReady=false，未把该历史债务写成完整终态。

- 原生 fullscreen PTY 补充发现：Codemode 同时有图片输出时，旧统一 graphics guard 在无图形协议终端阻止文本查看器。已在正常文本 viewer 之前保留其能力，只对 media-only fallback 检查 graphics；真实 Codemode Enter/关闭验证通过，`/tmp/grok-pi-reference-viewer-pty-final-20261008/report.json` 与 `codemode-fullscreen.txt`。
- 优化构建第一次 exit0/7m44s。因上述 native guard 修复，最终优化产物随后重新链接并验证；未复用第一次包冒充最终版本。
- Rust syntax 3395/3398 parsed、failures0；新 terminal module 另外经 rustfmt 与 Rust 编译/测试验证。未运行整套 10171 个 Pager 测试，实际执行范围如上。

- 最终普通 viewer 回归 16/0；带图片 Codemode 的无 graphics 路径单独 1/0。原 image-only fixture 曾用 Other（已有 text viewer），改为真正不含文本的 Read image，以验证原生媒体 fallback 的边界。
- 随包 SDK 初次发现 Node cp 默认把 `.bin` relative links 变成开发机 absolute links；现清理 generated dist 且 `verbatimSymlinks:true`。最终包经 tar `data` filter 完整解包、全部 symlink relative、4 SDK package.json 与 capabilities1.1.0、dylib closure/严格 ad-hoc codesign 检查通过。
- 最终安装 artifact：binary 54262832 bytes，SHA256 `3332e9d82cf729b8c922c78efc715ef4ca25fec8d16835d87d01f79aca619693`；archive `/tmp/grok-pi-0.1.10-reference-adaptation-macos-arm64.tar.gz` SHA256 `2b980442dd3686875037fc1593d2ce8089449fbfe99fc1a578410564bdfa5f02`。优化构建最后4m24s exit0，包内 Codemode fullscreen native PTY1/1。
- 已按此前本机覆盖授权更新 `/Users/kyros/.local/bin/grok-pi`，pig symlink保持；版本 `0.1.10+dirty`，未重写公开v0.1.10 Release。旧 bin/lib 备份 `/Users/kyros/.local/share/grok-pi/backups/20261008-reference-17766546`，原 SHA `c9da5cbce0c0ce6e3227bd66ea12733a85f8101fba5abe9577ce4547b000cf11`。Grok/Pi设置 hash不变。安装后三项经典 native PTY全部nativeExit0，证据 `/tmp/grok-pi-reference-installed-pty-20261008/report.json`；安装后 Durable live/reopen/F2/OSC native验证亦通过。

- 安装后的实际 bundled host/SDK（通过 ESM public package exports 和临时 faux fixture）再次完成 live、reopen、F2 与 OSC 验证，证据 `/var/folders/lh/z16j0yfx541cm_wsg95lm92c0000gn/T/grok-pi-durable-pty-tLuXoU`；不是仅使用仓库 node_modules。所有 A1–A8 在本次 scoped development 验证范围完成。真人 OAuth/模型、断电耐久性及所有终端体验仍未验证，Durable 普通插件和其他既有兼容缺口不因此宣称完成。
