---
id: "2026-10-03-pi-product-surface-plan"
title: "面向 Pi 的 Grok 风格 TUI：产品入口裁剪 PLAN"
status: "completed"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# 产品入口裁剪 PLAN

依据 [SPEC](20261003-pi-product-surface-SPEC.md)，从干净 `main@13697f9c` 实施；本地 commit，不 push。代码写入分文件并行，Cargo/build 由 Root 单一执行，来源声明在源码冻结后更新。

| 阶段 | 工作 | 状态 |
|---|---|---|
| P0 | 删除 external voice 能力与 Grok AuthManager 创建；保护 stock profile | 范围完成 |
| P1 | 收敛 external settings/目录/操作，裁剪 Grok 产品入口 | 范围完成 |
| P2 | 参考 Pi interactive 支持，统一帮助、教程和双语文档 | 范围完成 |
| P3 | 定向回归、两种 profile、生产 build、原生 PTY、精确来源及本地提交 | 范围验证完成；本地提交由 Root 执行，hash 见 Git log |

## 执行记录

- 实施起点为干净 `main@13697f9c`。前轮只读审计核对了安装 Pi 1.0.0 的公开 API。
- Voice 入口可达 Grok AuthManager/xAI STT；F2 coding_data_sharing 仍发送无 Pi handler 的 stock 请求。先处理明确产品残留，再核对同类入口，不删除用户配置或上游 stock 源码。

## 本轮回执

| 项目 | 结果 | 证据与边界 |
|---|---|---|
| 范围记录 | 已提交 | `76e7a5a87b9b6fb9a292021d7877666a69471cce`，仅新增本轮 SPEC/PLAN；实现及范围验收已完成，最终实现提交由 Root 执行并以 Git log 为准。 |
| P0 / voice | 源码完成 | external profile 不创建 Grok AuthManager；slash、快捷键、Welcome、F2 与诊断均阻断 voice。stock 源码和已有配置/凭据保留。 |
| P1 / settings | 源码完成 | external registry 保留通用 UI/Pi 控制；palette 与 Web catalog 同步裁剪 stock 产品入口，旧 action 与不支持的 Web key 在写入前拒绝。 |
| P2 / Pi 参考与文案 | 完成 | 安装 Pi 1.0.0 `docs/slash-commands.md`、`settings.md`、`tui.md`、`rpc-extension-ui.md` 及 interactive settings 实现；双语 README/矩阵、Alignment、grok-pi tutorial 和 AGENTS pointer 更新，stock tutorial 未改。 |
| Web 配置回归 | PASS | `bun test extensions/pi-grok-web-config/tests/config-store.test.ts extensions/pi-grok-web-config/tests/settings-conflict.test.ts`：8 pass、0 fail、205 expect，exit 0。Root 工具 session `92332` 回执；未单独保存日志。 |
| 精确来源 / negative | PASS | sourceguard 21/21、negative exit 0；47 个本轮文件逐项审阅、743 phase pins、696 无关旧 phase entries 不变；祖先 3797、历史 4479、native 830、旧 review-base `84174917` 保持，errors/unfrozen 为空。`/tmp/grok-pi-product-sourceguard-20261003.json`、`/tmp/grok-pi-product-negative-20261003.log`、`/tmp/grok-pi-product-identity-review-20261003.json`。 |
| Rust syntax | PASS | rustfmt 解析 1670 个 Rust 文件，failures 为空、exit 0；`/tmp/grok-pi-product-syntax-20261003.json` 与同名 `.log`，不改 source。 |
| Pi production check（audio 裁剪前） | PASS，检查点 | no-default `jemalloc,sandbox-enforce`，exit 0，37.42s；`/tmp/grok-pi-product-check-20261003.log`。 |
| Binary tests 首次运行 | 97 pass / 1 fail | 98 个测试中的 tutorial contract 要求字面量 `Pi agent core`，文案行折拆开导致失败；Root 只修正 tutorial 01 行折。第二次仍 97/1，因 tutorial 14 如实说明官方 Pi package 操作，旧 `does not install` 断言过时；仅改为现有 `does not migrate` 边界，保留其他断言。第三次 98/0 通过；两次为文案合同修正。 |
| Binary tests（audio 裁剪前） | PASS，检查点 | 98 pass、0 fail、exit 0；`/tmp/grok-pi-product-bin-tests-final2-20261003.log`。 |
| 本轮新增 external guards | PASS（5 项） | no-Grok-auth connection、voice、registry、settings actions、palette assertions 均在 Pager lib 的 passing 项中。 |
| 扩大 external_ 扫描 | FAIL，49 pass / 5 fail | 54 个 tests，正常与单线程复验相同：Ctrl+O 两项、dashboard toast prefix 一项、foreign session 两项。7 个相关 test/调用链文件与 `13697f9c` 完整字节相同；源码归因为空 hunks 不可折叠、已有 toast prefix 与 Pi PSM filter 语义差异。未跑 baseline runtime；归因 proof `/tmp/grok-pi-product-existing-test-failures-20261003.json`；`/tmp/grok-pi-product-external-tests-20261003.log`、`/tmp/grok-pi-product-external-serial-tests-20261003.log`。 |
| Production audio feature | 源码完成 | Pager 的 `voice/audio` 仅由 `stock-runtime` 启用；Pi 保留 shared types，不启用 microphone backend。依赖图增加四项 audio 禁止名单，保留原七项 stock guard。最终 Pi/stock profile 与 build 按新 feature 复验；此前 stock check 27.72s 仅属检查点。 |
| 正式 build | PASS | 新 audio feature 下 `./build.sh` exit 0、40.67s；`/tmp/grok-pi-product-build-20261003.log`。artifact 181,618,712 bytes，SHA-256 `210efc228712895125cd1082e07878f61aeafa220775c0579e397eb0dffa5e2d`；`76e7a5a8` + dirty frozen source stamp，不是未来 clean HEAD。proof `/tmp/grok-pi-product-artifact-20261003.json`。 |
| Production graph | PASS | normal/build 796 packages（前一检查点 806），stock_runtime 与 audio_backends 均为空；`/tmp/grok-pi-product-graph-20261003.json`、exit 0。数量不代表 cold build 或跨平台运行验收。 |
| 原生 PTY | PASS（4 cases） | product-surface、settings-save/reopen/rollback 全部 nativeExit 0，binary SHA `210efc…` 前后不变；`/tmp/grok-pi-product-pty-20261003/report.json`。强制 `GROK_VOICE_MODE=1` + legacy voice 配置仍无 voice/retention F2 入口，保留 UI；原 config bytes 不变、无 Grok auth.json。 |
| 最终组合 verify | PASS（本轮范围） | 新 audio feature 下 `./verify.sh` exit 0，结束 `All verification passed.`；`/tmp/grok-pi-product-verify-final-20261003.log`。Pi check 24.32s、stock check 1.38s、graph 796/双空；adapter lib 207 + disposition 1 / reloadACK 1 / EOF 2（非 ignored）、bin 98、两项指定 native 单 tests；sourceguard 21/743/errors 与 unfrozen 空、syntax 1670、mock 8 checks/33 lines/stderr 空均通过。实际专项 ignored tests 未在本轮重跑；artifact SHA `210efc…` 不变。 |

## 单独保留的所有权债务

- adapter-owned queue 与 extension input interception：应交 Pi 官方队列；当前 stable-id 编辑能力没有官方 RPC 等价物，迁移需保留未提交草稿边界。
- Plan/Goal：状态、提醒与续跑仍有 adapter authority，后续应搬入 Pi extension；Pi 1.0 没有等价内置 Plan/Goal。
- 可选 Workflow：Grok Rhai runtime 仍负责编排，Pi SDK 负责 worker；迁移需处理已有 `.rhai` 与暂停/取消/恢复语义。
- 增强 Bash/Eval：属于 Pi 扩展能力，但默认覆盖和私有 off-turn fallback 仍需进一步解耦。

这些条目不作为已完成迁移报告。本轮 voice 与 Grok 产品入口范围已完成；扩大 external_ 扫描的 5 项失败仍独立保留，不宣称全部 Pager tests 通过。
