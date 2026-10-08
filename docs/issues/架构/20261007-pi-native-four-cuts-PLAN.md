---
id: "2026-10-07-pi-native-four-cuts-plan"
title: "Pi 原生 TUI：四组重复实现裁撤 PLAN"
status: "complete"
created: "2026-10-07"
category: "architecture"
---

# 四组裁撤 PLAN

依据 [SPEC](20261007-pi-native-four-cuts-SPEC.md)，起点 clean `main@646abebb`，Goal 已建立。按 R0–R5 顺序实施，源码删除使用 `trash`；不 commit/push/发布或安装。

| 阶段 | 工作 | 状态 |
|---|---|---|
| R0 | 基线、调用闭包、SPEC/PLAN 与 Goal | 完成 |
| R1 | 移除 Eval runtime/入口；Bash 独立；保留历史卡片与官方 Codemode | 完成，保留历史卡片，最终 UI 回归见 R5 |
| R2 | 移除实验 native-commands/注入器/开关 | 完成，正式命令白名单保留 |
| R3 | 移除第二套 rust-tui-bridge/注入器/开关 | 完成，现有 Remote TUI 保留 |
| R4 | 删除未注入的 startup profiler | 完成 |
| R5 | 文档、聚焦测试、构建/check、guards、原生 PTY 与最终 diff | 完成，详见回执 |

## 检查顺序

1. 查完所有调用与动态注册后修改；扩展 injector 与 authored TS 相对导入闭包同步更新。
2. 扩展：真实 Pi RPC 加载与 Bash 前台/后台/超时/清理回归；Codemode 正向与 Eval 不可用负向检查。
3. Rust：grok-pi bin tests、adapter tests，相关 Pager settings/工具投影/历史 Eval/Codemode/Remote UI 有界测试。
4. 构建：`./build.sh`、生产 no-default `jemalloc,sandbox-enforce` check、stock binary check；使用 `scripts/cargo-shared.sh`。
5. 原生 PTY：重建 binary 的设置/产品入口、Bash 与 Codemode、历史消息/保留 Remote TUI。使用独立 Grok/Pi 临时目录，不读取真实凭据。
6. 更新当前文档与架构守卫；保留历史回执，不把源码检查写成真人 provider 成功。不为删除过时功能保留运行其旧 worker 的测试。

## 基线

- 四组独立 TS 候选文件 3,633 行；Eval 核心五模块 2,582 行。其他注入、设置、协议和历史渲染耦合另计。
- 生产 dependency guard：796 packages，stock_runtime/audio_backends/forbidden 为空，pending=29，passed=true、terminalReady=false。
- 既有全 Pager/stock settings 测试存在历史失败；先运行本次所触及的有界回归，失败与归因如实记录。
- 已安装 `/Users/kyros/.local/bin/pig` 的版本/文件/配置不属于本次修改目标。

## 阶段回执

随每阶段完成补充源码变更、检查命令、退出状态、证据路径及未验证边界。

### R1

- Bash-only 四模块 bundle；五个 Eval runtime 模块与三个旧 Eval harness/demo 用 trash 删除。移除配置字段、F2/Web/actions/slash、Eval-only adapter 策略，保留旧载荷/卡片读取。
- Bash runtime 回归 exit0：前台、后台、missing IDs、取消、超时、自动转后台、CLI限制；旧 Eval 环境变量不重新启用工具。真实独立物化 bundle 在 Pi RPC 加载 exit0，官方 Codemode 存在且 Eval 不存在。
- 生产 check exit0，25.58s；bin94/0；Web12/0/200 assertions。日志 `/tmp/grok-pi-four-cuts-r1-check3.log`、`/tmp/grok-pi-four-cuts-r1-bin-tests.log`、`/tmp/grok-pi-four-cuts-web-tests.log`。
- 旧 Eval-only runtime fixture 改为官方 Codemode/MCP 正向：normal/enabled/deferred/search-excluded/excluded/disabled，6/6、exit0；工具校验/阻断/隐藏、MCP资源与 CLI 限制保留。日志 `/tmp/grok-pi-four-cuts-rpc2.log`。首轮 Codemode 未激活，按 Pi 官方 defaultTools配置修正 fixture后通过；没有修改 Pi源码。
- 首两轮 check 发现共享 PiBuiltinTools 遗留字段/默认判断，已完成调用闭包清理后通过。

### R2

已用 trash 删除 native-commands扩展/injector；移除启动、资源、子代理注入、生命周期持有及实验开关。正式 PI_GROK_NATIVE_COMMANDS 命令白名单是另一职责，保留；正式 /model /resume /reload 与官方 RPC目录不改。最终编译/PTY 见 R5。

### R3

已用 trash 删除 rust-tui-bridge扩展/injector；移除启动、资源、子代理注入与生命周期持有。保留现有 Remote TUI、shortcut-manager及其已验收frame/key/focus链路，不扩大到剩余私有hooks。最终Remote UI回归见R5。

### R4

用 trash 删除未被产品物化/注入的 pi-grok-00-profiler。保留历史来源清单，无生产替代或新增依赖。

### R5 最终回执

- 最终 bin90/0；adapter lib205/0，加非忽略 integration4/0；原有7个 requires-installed-Pi integration 仍按各自 ignore 合同处理，不冒称全部运行。日志 `/tmp/grok-pi-four-cuts-bin-final.log`、`/tmp/grok-pi-four-cuts-adapter-tests.log`。
- 相关 Pager settings232/0（明确排除既有 settings_modal tests和settings_slash_from_home_opens_settings）；历史 Eval卡片8/0。日志 `/tmp/grok-pi-four-cuts-settings-tests.log`、`/tmp/grok-pi-four-cuts-history-render.log`。没有宣称全 Pager/stock test suite通过。
- Web12/0/200 assertions；网站两份文案字典的 TypeScript语法通过，没有运行网站完整构建。日志 `/tmp/grok-pi-four-cuts-web-final.log`。
- 实际 Bash harness通过前台/后台、missing IDs、取消、超时、自动转后台、CLI排除与退出清理；嵌套sleep PID被终止后ESRCH，旧Eval环境变量无作用。日志 `/tmp/grok-pi-four-cuts-bash-final.log`。
- 真实Pi1.0.4+本地synthetic provider/MCP：Codemode普通、enabled/deferred、search-excluded、excluded、disabled共6场景通过；没有真实推理。日志 `/tmp/grok-pi-four-cuts-rpc2.log`。
- 显式运行installed_pi_live_replay_and_dialog_teardown：1test/5场景通过（Codemode、Subagent、signal、timeout、EOF），真实SDK子会话和live/replay图像/输出一致。报告 `/tmp/grok-pi-four-cuts-projection.json`，日志 `/tmp/grok-pi-four-cuts-projection.log`。
- `./build.sh` exit0/34.74s，生产 no-default jemalloc,sandbox-enforce。新debug binary `181501768` bytes、SHA256 `814114cf9f781cafe47088a62306c6c11c42aa28749515cabea91647bb08eb21`，版本 `grok-pi 0.1.10+dirty [stable]`；源为base `646abebbd80f8856a1baa05cbd39a0bcec81cff5` 上未提交的本次差异。报告 `/tmp/grok-pi-four-cuts-artifact.json`，日志 `/tmp/grok-pi-four-cuts-build.log`。R1生产check及最终stock binary check均exit0；stock check43.40s，日志 `/tmp/grok-pi-four-cuts-stock-check.log`。
- 新binary native PTY9/9、全部nativeExit0、SHA前后一致：Bash、Codemode、Remote UI、product-surface、settings-save/reopen/rollback、language-save/reopen。保留Remote TUI正常，退役实验环境变量置1仍不能注入旧入口。报告 `/tmp/grok-pi-four-cuts-pty-final/report.json`。初轮Bash fixture在首请求对undefined prior content调用includes失败；只修fixture判定，再以同一binary通过，未改Rust runtime绕过失败。
- architecture17 checks、negative guard、four-cuts source/import-closure guard、mock8 checks通过；语法解析3395个现存tracked Rust，3个本次明确删除源文件列入removedTrackedFiles，无失败。语法守卫只跳过Git明确D的缺失源，未对其他缺失文件放宽。报告 `/tmp/grok-pi-four-cuts-architecture.json`、`/tmp/grok-pi-four-cuts-syntax.json`、`/tmp/grok-pi-four-cuts-mock.json`。
- production dependency guard passed=true，796 packages、pending29、stock_runtime/audio_backends/forbidden均为空，terminalReady=false。本增量没有声称业务依赖终态。报告 `/tmp/grok-pi-four-cuts-dependencies.json`。
- 四组相关模块、注入与设置入口退休；旧会话载荷和卡片保留。双语README、矩阵、对齐说明、教程、Changelog与总体SPEC/PLAN登记同步；README明确v0.1.10下载包早于此改动。
- 本地差异和whitespace检查通过；无commit/stage/push/worktree/新Release/安装。已安装pig仍为grok-pi0.1.10 stable，SHA d4e38e7a202ff21e651f56339aed74ae5401b6b8001e89612dace11a2468cc99；未操作用户配置、凭据或会话。

本四组裁撤完成。真人OAuth/provider推理、另一台Mac/其他平台、原CPU占满现场和总体T2–T8均未作为本次验收结论。

## 后续授权：覆盖本机 pig（2026-10-07）

用户明确要求覆盖当前pig，撤销本机安装这一项原先的范围限制；其他commit/push/发布限制保持。以相同生产features构建优化release-dist，CGU16/LTO off/debug0/jobs4，exit0/1m25s，stamp `grok-pi 0.1.10+dirty [stable]`。复用macOS打包脚本，重定位两个非系统dylib并进行ad-hoc签名/strict验证；未发布新版本。

旧binary和动态库备份 `/Users/kyros/.local/lib/grok-pi/backups/four-cuts-exir8ft5`；库按文件原子替换，binary先在目标bin目录stage并验证help/签名，再os.replace覆盖。pig/pi-grok符号链接保留。新binary 60458752bytes，SHA256 `c9da5cbce0c0ce6e3227bd66ea12733a85f8101fba5abe9577ce4547b000cf11`；fresh login shell通过PATH执行pig为本地开发版。安装前后Grok UI配置与Pi settings的hash相同，没有操作凭据或会话。

真实已安装pig再次native PTY：Bash、Codemode、Remote UI、product-surface，4/4、nativeExit0、SHA不变。证据 `/tmp/grok-pi-four-cuts-installed-pty/report.json`、`/tmp/grok-pi-four-cuts-installed.json`，构建日志 `/tmp/grok-pi-four-cuts-local-release-build.log`。原Goal源码验证与发布v0.1.10记录保持历史，不将本地dirty版本称作新正式Release。
