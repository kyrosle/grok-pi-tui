---
id: "2026-10-05-settings-language-plan"
title: "设置功能命名与中英文 PLAN"
status: "in-progress"
created: "2026-10-05"
category: "architecture"
---

# 设置功能命名与中英文 PLAN

依据 [SPEC](20261005-settings-language-SPEC.md)，起点 `main@f7a6c8d7`。沿用用户本地逐阶段 commit 授权，不 push。

| 阶段 | 内容 | 状态 |
|---|---|---|
| L0 | 调查设置注册/原生渲染/持久化/Web 目录；记录范围 | 完成 |
| L1 | 功能命名、双语文案、语言配置与原生设置集成 | 完成，368 条共享词条 |
| L2 | Web 配置共享文案与语言选择 | 完成，12 unit / 37 browser checks |
| L3 | 单元/配置测试、profile 检查、原生 PTY 与文档 | 完成，有界检查通过 |
| L4 | 本机 tpig 构建安装、交付记录 | 进行中 |

## 验证记录

- Web：`bun test extensions/pi-grok-web-config/tests/config-store.test.ts extensions/pi-grok-web-config/tests/settings-conflict.test.ts extensions/pi-grok-web-config/tests/language.test.ts`，12 pass / 0 fail / 200 assertions。日志 `/tmp/settings-language-web-unit-20261005.log`。
- 合成配置经真实 loopback HTTP + browser 回归：37 checks pass，包含共享语言覆盖旧浏览器偏好、auto/中文/English、重载持久化、中文搜索、外部冲突和 390/768/1440 宽度；无浏览器 runtime errors。证据 `.impeccable/review/regression.json`。首次 runner 等待隐藏 selector 失败，修正 fixture 后重跑通过；最终纯文案调整重过 unit parity。
- Native architecture：17 guards pass；negative guard exit 0。证据 `/tmp/grok-pi-settings-architecture-20261005.json`，日志 `/tmp/grok-pi-settings-architecture-guard-20261005.log`。
- 扩大 `--lib settings` 首轮 374 pass / 48 fail；修复新增测试 fixture/CJK extraction、运行时覆盖文案、测试辅助分发遗漏，并补两项既有 action、三个 section、五个模型槽位 reset。后续有界重跑排除旧 stock modal 与 Home 合同用例，最终有界检查 232 pass / 0 fail，exit 0；命令 `./scripts/cargo-shared.sh test -p xai-grok-pager --lib settings -- --skip views::settings_modal::tests --skip settings_slash_from_home_opens_settings`，日志 `/tmp/grok-pi-settings-native-tests-final3-20261005.log`。
- 36 个旧 `settings_modal` 失败与 1 个 `/settings` Home 失败有 HEAD 已存在的源码合同矛盾：旧测试初选 compact_mode/Appearance、synthetic Other row 与现行 Theme/tab/sidebar 不符；Home 仍期待 Settings，而命令早已打开 PiSettings。对应测试/渲染/input/命令入口与起点字节相同；没有执行 HEAD baseline，不能声称 baseline runtime 通过或失败。首轮完整日志 `/tmp/grok-pi-settings-native-tests-20261005.log`，源码证据 `/tmp/grok-pi-settings-existing-tests-source-20261005.json`。不宣称全 Pager/stock 测试通过。

- Pi binary unit：98 pass / 0 fail，no-default `jemalloc,sandbox-enforce`，日志 `/tmp/grok-pi-settings-bin-tests-20261005.log`。
- Pi / stock profile check 均 exit 0：22.63s / 49.30s，日志 `/tmp/grok-pi-settings-pi-check-20261005.log`、`/tmp/grok-pi-settings-stock-check-20261005.log`。
- `./build.sh` exit 0，48.55s；debug artifact 181737512 bytes，SHA `66c0a6f68069138fa76c60412d1f41a0bab1ba02e01b77e11830852b2974329a`。日志 `/tmp/grok-pi-settings-build-20261005.log`，artifact `/tmp/grok-pi-settings-debug-artifact-20261005.json`。
- Native PTY 6 cases 全部 nativeExit 0，binary SHA 前后一致：language-save/reopen（auto 中文、英文/中文选择、另一 locale 进程保留中文、auto 回英文、只读保存失败恢复英文）、settings-save/reopen/rollback、product-surface。命令 `node --experimental-strip-types crates/codegen/pi-grok-adapter/tests/pi_native_pty_smoke.ts language settings product-surface`（显式指定 fresh binary/SHA/artifacts）。证据 `/tmp/grok-pi-settings-pty-final3-20261005/report.json`。此前 runner 的 modal close/search 焦点等待及 modal 遮住 toast 的观测假设失败，已修测试等待；没有修改 Rust runtime 来绕过失败。
- Production dependency guard：796 packages，stock_runtime/audio_backends/forbidden 为空，29 pending removals 保持，非终态验收。证据 `/tmp/grok-pi-settings-dependency-20261005.json`。
- 翻译范围为配置界面；本轮没有真人 OAuth、模型推理、跨平台终端或全 Pager/stock 测试验收。
