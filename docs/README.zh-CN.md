# grok-pi — Pi 原生 Rust TUI

总纲以 [SPEC](issues/架构/20261003-pi-native-tui-SPEC.md) 和 [PLAN](issues/架构/20261003-pi-native-tui-PLAN.md) 为准：最终仅保留 grok-pi，Pi 为唯一内核，删除 stock profile 与 Grok 业务依赖。Grok Build 只作 UI 参考仓库。T0–T8 正在推进，先前入口裁剪是历史检查点，当前证据见[验证报告](VERIFICATION.md)。


> 在 Grok Pager 原生终端 UI 中使用 Pi 模型、工具与会话。

[下载最新版本](https://github.com/kyrosle/grok-pi-tui/releases/latest) · [English](../README.md) · [功能矩阵](FEATURE_MATRIX.md) · [架构说明](NATIVE_GROK_TUI_ALIGNMENT.md) · [验证记录](VERIFICATION.md) · [更新日志](CHANGELOG.zh-CN.md) · [Changelog (EN)](../CHANGELOG.MD)

> **Pi core、原生 Pager UI、可配置扩展。** Grok Pager 提供终端体验；Pi 提供模型、Provider 鉴权、工具、会话和运行控制。Todo、Subagents 等由 bundled Pi 扩展提供，分别标注默认值与能力边界。

`grok-pi` 将 Pi Agent Runtime 接入 Grok Pager。external 产品入口不提供 Grok voice/STT/TTS、账号/计费、训练/retention 或 stock agent/plugin/MCP 控制；F2、命令面板与 Web 宿主设置目录共用这一边界。Provider 鉴权使用 Pi `/login`、`/logout`，无需 Grok 账号。

**开发源码说明：**本次四组裁撤尚未发布；可下载的 v0.1.10 仍是裁撤前版本。使用新行为需构建当前 checkout。

## 安装（macOS Apple Silicon）

**v0.1.10** 提供 **macOS 14（Sonoma）及更高版本**的 Apple Silicon（M1/M2/M3/M4 及后续芯片）预编译包，无需 Rust、源码仓库或 MacPorts。Intel Mac、Linux 和 Windows 的二进制发布暂未提供。

先安装 [Pi](https://pi.dev) **1.0.0 或更高版本**（使用 npm 安装需要 Node.js **22.19.0+**），再安装 grok-pi：

```bash
npm install --global @earendil-works/pi-coding-agent
curl -fsSL https://github.com/kyrosle/grok-pi-tui/releases/latest/download/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
pig
```

安装器将 `grok-pi` 和随包动态库安装到 `~/.local/bin`，并创建 `pig`、`pi-grok` 符号链接。日常直接使用 `pig`，无需 `tpig` 启动器。macOS 默认 zsh 可在 `~/.zshrc` 中添加一次以下内容，使新终端也能找到命令：

```bash
export PATH="$HOME/.local/bin:$PATH"
```

可用 `GROK_PI_INSTALL_DIR` 覆盖安装目录。固定安装本次版本：

```bash
curl -fsSL https://github.com/kyrosle/grok-pi-tui/releases/download/v0.1.10/install.sh | \
  GROK_PI_VERSION=v0.1.10 sh
```

下载包：[`grok-pi-macos-aarch64.tar.gz`](https://github.com/kyrosle/grok-pi-tui/releases/download/v0.1.10/grok-pi-macos-aarch64.tar.gz)。Release 同时提供 SHA-256 校验文件，以及随包动态库的许可证和源码归档。启动后通过 Pi `/login` 登录 Provider。设置使用 `~/.grok-pi` 和项目 `.grok-pi`；旧 `~/.tpig` 测试设置不自动迁移。

## 可选 Durable 模式（开发源码）

F2 → Agent → **Durable 模式（实验性）**保存 `[ui].pi_durable`，默认 `false`，下次启动生效，当前会话仍使用原后端。`pig --durable` / `pig --no-durable`可单次覆盖保存值。

```bash
pig --durable
pig --durable --continue
pig --durable-background  # Unix：显式后台 owner，关闭界面后继续执行
```

此模式使用官方 Pi SDK 1.1.0 与隔离 SQLite，支持已提交的消息/工具输出、队列、任务图（`/tasks`）、前台子任务和恢复。不安全工具中断后先暂停；检查副作用后，用 `/durable-recover continue` 或 `/durable-recover abort`作明确决策。后台 owner 一次接受一个界面，可重连；没有界面、任务已空闲30秒后退出。普通模式退出会暂停工作，下次打开恢复。

普通Pi插件、Codemode、MCP、图片、Plan/Goal/Loop及经典树导航尚未适配到此实验模式，默认Pi RPC保持现有功能。鉴权复用公开ModelRuntime，先在普通Pi中 `/login`。用 `GROK_PI_BUILD_DURABLE=1 ./build.sh`构建随包host；macOS打包流程会携带依赖及许可，需要系统Node.js 22.19+。已发布v0.1.10尚不包含此模式；本机已验证的2026-10-08开发构建包含此模式。实现和兼容缺口见[Durable SPEC](issues/架构/20261007-pi-durable-integration-SPEC.md)与[PLAN](issues/架构/20261007-pi-durable-integration-PLAN.md)。

## Pi 1.1 适配（开发源码）

Pi 1.1 支持 `pig --tools +codemode,-bash`，显式 CLI 选择优先于 F2。原生 Codemode 卡片可回车打开全屏查看器，恢复后保留官方执行耗时；折叠工具条目有 trace 时显示快捷键。模型详情保留分档价格，实际费用仍由 Pi 计算。Remote TUI 兼容旧版和新版光标标记。

支持的终端经探测后接收原生 OSC 7501 状态；`PI_PROGRAM_STATUS=1` 强制开启，`PI_PROGRAM_STATUS=0` 关闭。上报仅含应用名称和状态。Durable SDK 1.1.0 可通过官方 SQLite API 重开已知 1.0.4 会话，保持原 manifest；未知 SDK 版本拒绝打开。上述源码变更尚未发布到 v0.1.10。[实现与验证](issues/架构/20261008-reference-adaptation-PLAN.md)。

## 启动

在任意项目目录下直接运行：

```bash
pig
# 也可使用 grok-pi、pi-grok
```

默认使用 PATH 上的 `pi`，并以当前工作目录作为项目目录。继续上一会话：`grok-pi --continue`。

常用命令：

```bash
grok-pi --help
grok-pi update --check
grok-pi update
grok-pi update --channel beta    # 持久化到 ~/.grok-pi/config.toml
grok-pi update --channel stable  # 切回 stable；默认即 stable
```

更新通道是 grok-pi 产品隔离状态，持久化在 `~/.grok-pi/config.toml` 的 `[update].channel`。`stable` 为默认通道且永远不会选择 prerelease；`beta` 跟踪合法的 `-beta.N` GitHub prerelease，但当正式版 semver 更高时也会继续升级。`grok-pi --version` 会显示当前通道，`grok-pi update --check --json` 会返回解析后的 `channel`。

## 能力概览

| 领域 | 能力 |
|---|---|
| Agent Runtime | Pi 模型、Provider、工具、扩展、skills、会话、重试和压缩 |
| Provider 鉴权 | 原生 UI 薄桥委托 Pi ModelRuntime login/logout；provider methods、凭据与 MCP 配置由 Pi 所有。Radius 保留通用登录，桥内不写 `mcp.json`、不维护 provider 专用配置流程。 |
| 模型管理 | `/pi-models` 提供原生 Provider → Model → Details 编辑器，含安全 `models.json` 事务、备份/恢复、Pi 热重载和 typed 激活；`/model` 保留为快速切换器 |
| 浏览器配置工作台 | `/pi-config web` / `/pi-models web`：模型/Provider、资源路径、Pi 设置及过滤后的 Pager UI 设置目录，提供中英界面。由 Pi 扩展提供带 token 的本地回环服务。 |
| 终端 UI | Grok Pager 输入、斜杠补全、Markdown、工具卡片、diff、对话框和 scrollback |
| 产品教程 | `/tutorial`（别名 `/tour`、`/onboarding`）展示 18 个能力域：Pager 原生控制、Pi Provider/模型/工具/会话、扩展/Skill/Package 与可选自动化，分别说明边界 |
| Remote TUI 兼容 | 实验 host 在 Pager 中承载受支持的 Pi `ctx.ui.custom` 交互；默认开启，兼容性仍按组件验证 |
| 增强 Shell 执行 | Bundled Pi Bash 扩展提供后台任务、输出限制、超时和进程树清理 |
| 并行工作 | Bundled Subagents 扩展使用 Pi child session，默认开启，提供原生任务视图与产品隔离 agent 定义。可选 Subagents V2 增加稳定 `/root/...` path、peer messaging、nested spawn 和 `.grok-pi/teams` / `~/.grok-pi/teams` preset |
| Rhai Workflow | 使用 Pi worker 的可选 `xai-workflow` 宿主，默认关闭（F2 **Pi workflows**）；脚本目录 `~/.grok-pi/workflows` 与 `<repo>/.grok-pi/workflows`。编排语义尚未迁入 Pi core。 |
| 会话流程 | Resume、树导航、标签、回顾、上下文查看和会话选择器 |
| 资源管理 | `/pi-config` 管理 Pi 资源并通过当前 Pi 官方 CLI 安装、移除和更新 package，随后重载并显示实际运行 registry。Global/Project 信任、filters 和 pins 保持 Pi 所有；移除 local 声明不删源目录。 |
| Pi 运行控制 | `/pi-runtime` 查看运行状态、切换自动 retry/compaction，并取消 retry 等待；实际 compaction 状态与配置中的 retry policy 分开标注。 |
| 更新 | 产品隔离的 `stable` / `beta` GitHub Release 通道；持久化到 `~/.grok-pi/config.toml`，后台检查、`grok-pi update`、`--check --json` 与目标 tag 安装器下载均感知通道 |

先前深度适配检查点通过自动 build/verify 与四次原生 PTY；一次配置 default 的真实 SDK chat 返回 OK，credential/config 字节未改。这些属于历史结果，本轮产品入口验证独立记录在[VERIFICATION](VERIFICATION.md)。真人 Provider 原生 UI、OAuth、真实图片和目标终端体验保持独立验收层。

Provider/model 行为、鉴权、工具、会话、retry 与 compaction 以 Pi 为权威，Pager 呈现控制和结果。Todo、Plan、Goal、增强 Bash 与 Subagents 属于 grok-pi 扩展或集成，并非 Pi 内置功能。adapter queue interception、Plan/Goal 状态和可选 Rhai 编排仍是当前 PLAN 记录的所有权工作，本轮 UI 裁剪不宣称它们已迁移。

命令按职责组织：

| 领域 | grok-pi 入口 |
|---|---|
| Pi 模型与 Provider | `/model`、`/effort`、`/login`、`/logout`、`/pi-models` |
| Pi 会话与上下文 | `/new`、`/resume`、`/rename`、`/session`、`/tree`、`/fork`、`/clone`、`/compact` |
| Pi 资源与运行控制 | `/pi-config`、`/reload`、`/pi-runtime`；已加载的 extension、prompt 与 `/skill:name` 命令 |
| 原生终端 UI | `/settings` / F2、`/theme`、`/hotkeys`、`/tutorial`、`/copy`、`/find`、`/transcript` |

集成参考 Pi interactive 的命令和设置组织，保留 `/effort`、`/rename` 等 Pager 名称。F2 使用原生 Pager 设置面，不复制 Pi interactive settings 组件。

F2 → 外观 → 设置语言可以选择“跟随系统”（默认）、“简体中文”或“English”。设置名称、说明、选项和操作提示跟随所选语言，搜索支持中英文。选择保存在产品 `config.toml` 的 `[ui].language`，值为 `auto`、`zh-CN` 或 `en`，与 Web 配置页共享。功能使用“团队协作”“任务依赖与提醒”“代码执行模式”等名称；原有 `*_v2` 配置键和执行模式持久值保持兼容。此语言设置覆盖配置界面。

详细行为和有意边界见[功能矩阵（中文）](FEATURE_MATRIX.zh-CN.md) / [English](FEATURE_MATRIX.md)。

Package 变更重算policy控制的startup admission；输入不变走官方Pi重载，输入改变则official dispose后重启并用公开API恢复session、leaf、model、thinking。没有persistent sessionFile或user-message leaf无法安全恢复时，保持saved/deferred，待完成响应或用户重启；最终nativefixture验证了组合install/remove闭环与history保全。

Package 命令完成、Pi 已重载、实际 command/tool registry 条目属于不同状态。Pi 1.0 RPC 未暴露 resource loader 的完整 errors，因此界面保留 package load status 未验证。Web 设置编辑器保留保存前重读，并要求服务端 revision（`If-Match`）；保存时已发生的外部变化会被拒绝。短暂 compare/replace 窗口仍不能排除外部编辑器同时写入。

`/model` 保持虚拟模型选择；Pi 报告实际派发模型和 thinking level 时，原生状态单独显示。图像和 classifier 模型走 Pi 官方 ModelRuntime，聊天选择器仅接受 chat 模型。合成 SDK/RPC 检查与真实 provider、OAuth、终端验收分开记录，见[深度适配执行记录](issues/架构/20261003-pi-deep-adaptation-PLAN.md)。

## 架构

```mermaid
flowchart LR
    User[终端用户] <--> Pager[Grok Pager\n原生 TUI]
    Pager <--> ACP[ACP]
    ACP <--> Adapter[pi-grok-adapter\nJSONL RPC ↔ ACP]
    Adapter <--> Pi[Pi\nAgent Core]
```

集成包含三个边界：

- **Grok Pager** 负责终端生命周期、输入、渲染、对话框和所有可见 UI。
- **Pi** 负责 Agent loop、模型、Provider、工具、扩展和会话。
- **`pi-grok-adapter`** 是 headless JSONL RPC ↔ ACP 桥接层，不拥有终端，也不渲染第二套 UI。

不修改 Pi 源码。主要集成使用公开 RPC 与 Extension API；实验 Remote TUI compatibility host 对 stock RPC 未暴露的能力还使用有界 host hooks。

`/pi-ui-capabilities` 列出标准原生映射、有界映射、实验 Remote TUI 与 unsupported 方法。Working message/visibility/indicator 映射到原生 status；动画 indicator、持久 header/footer/widget factory 和 raw input/editor replacement 仍受 Pi RPC 边界限制。实验 mode facade 仅在实际 Remote TUI host 存在时启用，不保证全部第三方组件兼容。

自定义组件打开期间独占键盘输入，包括字母操作、粘贴和普通 `Esc`（由组件处理）。
组件打开的原生 Pi 输入框或确认框临时优先接收输入。终端能够区分该组合键时，
`Ctrl+Shift+Esc` 可强制关闭失去响应的远程组件。传输 metadata 按 Grok-Pi 进程隔离，
已关闭组件的过期输入会被丢弃。插件应使用 Pi 的按键解析函数处理带修饰键的输入；
直接比较原始字符串无法识别全部终端编码。
普通 Shift+字母按下会转发为大写字符（如 `S`），兼容 Pi 组件的字面量快捷键；
其他修饰键按下保留修饰信息，重复和释放事件仍单独编码。

## 配置

稳定的内置桥接扩展默认启用；实验性原生命令需要显式开启。

| 变量 | 默认值 | 用途 |
|---|---:|---|
| `PI_GROK_REMOTE_TUI` | `1` | 启用 Pi `ctx.ui.custom` 组件 |
| `PI_GROK_BASH` | `1` | 启用内置 Pi Bash 集成 |
| `PI_GROK_SUBAGENTS_V2` | `0` | 在 Pi subagents 上启用可选 V2 team tools（`spawn_team`、稳定 agent path、peer messaging、nested spawn）；与 F2「Pi subagents V2」开关等效 |
| `GROK_HOME` | `~/.grok-pi` | 用户状态根目录（与 stock Grok 的 `~/.grok` 隔离） |
| `GROK_PROJECT_DIR` | `.grok-pi` | 仓库内项目配置/workflows/hooks 目录名 |
| `GROK_PI_NO_AUTO_UPDATE` | 未设置 | 禁用后台更新检查 |

Subagents V2 的 team preset 使用 JSON，放在 `<repo>/.grok-pi/teams` 或 `~/.grok-pi/teams`（项目覆盖全局，全局覆盖 bundled preset）；agent profile 继续使用对应 `agents/` 目录里的外置 Markdown。示例：

```json
{
  "name": "implementation",
  "description": "Implementation plus review",
  "members": [
    { "name": "implementer", "agent": "general-purpose", "task": "Implement: {{task}}" },
    { "name": "reviewer", "agent": "explore", "task": "Review: {{task}}" }
  ]
}
```

启动 grok-pi 前用 F2「Pi subagents V2」开关或设置 `PI_GROK_SUBAGENTS_V2=1`；用 `/subagent-teams` 查看 preset。`spawn_team` 启动整组 preset，`spawn_team_agent`、`team_send_message`、`team_followup_task`、`team_wait`、`team_list`、`team_interrupt` 提供底层协作面。Rhai Workflow 仍负责确定性编排；Team V2 负责 session-scoped、可跨单次 run 复用的 agent identity 和 peer messaging。

Rhai Workflow **默认关闭**（F2 → Agent → **Pi workflows**，改完后需**整进程重启**）。细节见 [功能矩阵](FEATURE_MATRIX.zh-CN.md)、[AGENTS.md 产品态隔离](../AGENTS.md#product-state-isolation)。

Herdr 生命周期上报**默认关闭**。可在 F2 → Agent → **Pi Herdr integration** 中开启，然后重启。详见 [Herdr 设置指南](usage/grok-pi-herdr.zh-CN.md)。

使用 `--no-extensions`（`-ne`）可关闭 Pi 扩展自动发现；显式 `-e` 路径与 grok-pi 宿主桥接仍会加载。使用 `--no-bridge-extensions` 可关闭内置宿主桥接；组合两个开关可实现完全无扩展启动。Pi 启动参数可放在 `--` 之后直接传递：

```bash
grok-pi -- --model openai/gpt-4o
```

## 从源码构建

环境要求：Rust **1.92.0**、Node.js **22.19.0 或更高版本**、npm，以及系统 Pi 安装。

```bash
./build.sh
./target/debug/grok-pi
# 或: PI_BIN=pi ./run-local.sh
```

项目内 Cargo 命令应使用 `./scripts/cargo-shared.sh`：默认启用增量编译，
生成的 target 默认上限为 128 GiB，并在剩余空间低于 20 GiB 前停止。
可用 `CARGO_TARGET_MAX_GIB` 覆盖容量上限；周期性 maintenance 会先清理
incremental 缓存，若已超限的 target 仍过大则执行 `cargo clean`。只有明确确认
风险时才覆盖 `CARGO_MIN_FREE_GIB`；单次命令可用 `CARGO_MAINTENANCE=0`
跳过命令前 maintenance。运行中的 disk guard 持续检查剩余空间，target 容量
检查按 maintenance 周期执行。

运行验证：

```bash
./verify.sh
```

静态检查与运行时验收的区别见[验证记录](VERIFICATION.md)。

## 文档

- [功能矩阵](FEATURE_MATRIX.zh-CN.md) —— 支持的行为与有意边界（[English](FEATURE_MATRIX.md)）
- [四组裁撤 SPEC](issues/架构/20261007-pi-native-four-cuts-SPEC.md) —— 工具编排使用 Pi Codemode，自建 Eval 已退休
- [Subagents V2 使用指南](usage/subagents-v2.zh-CN.md) —— 可选 team 协作、稳定 path、preset、队列语义、回滚与排障（[English](usage/subagents-v2.md)）
- [架构对齐](NATIVE_GROK_TUI_ALIGNMENT.md) —— 组件所有权、协议映射和迁移说明
- [验证记录](VERIFICATION.md) —— 已完成检查与环境阻塞项
- [更新日志](CHANGELOG.zh-CN.md) / [Changelog (EN)](../CHANGELOG.MD) —— 版本历史（中英）
- [贡献指南](../CONTRIBUTING.md) —— 贡献流程

## 许可证

项目及上游声明见 [LICENSE](../LICENSE) 和 [THIRD-PARTY-NOTICES](../THIRD-PARTY-NOTICES)。

## 功能开关 → 会禁用的 Pi 扩展

当 grok-pi **原生能力开启**时，宿主资源准入可能 block 已知冲突的 Pi 包，避免工具名/职责撞车。内置默认表：[`crates/codegen/xai-grok-pager/assets/native_feature_conflicts.toml`](../crates/codegen/xai-grok-pager/assets/native_feature_conflicts.toml)。运行时外挂（免 rebuild）：`$GROK_HOME/native-feature-conflicts.toml`，再 `$GROK_PROJECT_DIR/native-feature-conflicts.toml`（packages **并集**；非空 `reason` 覆盖）。用户资源策略的 `allow` 仍可豁免。

```mermaid
flowchart LR
  A[内置默认] --> M[合并]
  B[用户外挂] --> M
  C[项目外挂] --> M
  M --> T[冲突表]
  T --> P[功能开时禁用]
```

| 功能开关 | 如何开启 | 默认 | 会禁用的包（npm） |
|---|---|---:|---|
| **Q&A**（`pi_ask_user_question`） | F2 → Agent → Q&A（需重启） | 关 | `@juicesharp/rpiv-ask-user-question` |
| **Q&A 桌面通知**（`pi_ask_user_question_notifications`） | F2 → Agent → Q&A desktop notifications | 开 | — |
| **Pi goal mode**（`pi_goal`） | F2 → Agent → Pi goal mode（需重启） | 关 | `pi-codex-goal`、`@narumitw/pi-goal`、`@misunders2d/pi-goal`、`pi-goal`、`pi-goal-x` |
| **Pi workflows**（`pi_workflows`） | F2 → Agent → Pi workflows（需重启） | 关 | `@quintinshaw/pi-dynamic-workflows` |
| **Pi subagents**（`pi_subagents`） | F2 → Agent → Pi subagents（需重启） | 开 | `pi-subagents`、`@tintinweb/pi-subagents`；原生 `/subagents` 管理隔离的项目/全局 Markdown agent 定义。V2 另用 F2「Pi subagents V2」开关或 `PI_GROK_SUBAGENTS_V2=1` 开启；`/subagent-teams` 发现 project/global/bundled JSON preset |
| **`/btw`**（`pi_btw`） | F2 → Agent → Pi /btw（需重启）；已保存答案可用 `/btw-history` 查看 | 关 | `pi-btw`、`@narumitw/pi-btw`、`@juicesharp/rpiv-btw` |
| **用户消息 Markdown**（`pi_user_markdown`） | F2 → Agent → Markdown user messages | 开 | — |

Pi Codemode 为 F2 可选工具，加载 Pi 官方 `builtin:codemode`；Pi MCP 使用官方 MCP 与工具搜索扩展。Pi 负责工具执行、校验、连接、OAuth 与取消。增强 Bash 保留任务管理，Python 计算可通过 Bash 执行。

当前开发源码移除自建 Eval v1/v2、Eval-only/MCP 模式、`/eval-display`、重复实验 `/pi-*` 选择器、第二套 Rust TUI Bridge 和启动 Profiler。历史 Eval 会话卡片继续可读。见[四组裁撤 SPEC](issues/架构/20261007-pi-native-four-cuts-SPEC.md)与[PLAN](issues/架构/20261007-pi-native-four-cuts-PLAN.md)。
