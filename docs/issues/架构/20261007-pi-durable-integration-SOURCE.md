# Pi Durable 官方接入参考核对

核对日期：2026-10-07。固定 Pi 官方源码 `b2363841a525bec5bdfcf4361fa7a1730076f5c5`，commit日期 `2026-10-07T08:54:32Z`；coding-agent 与 durable 的 package.json 均为1.0.4。当前本机 coding-agent1.0.4，未安装 pi-durable；版本号相同不代表普通CLI已使用Durable。

此文件为设计证据，未执行官方Durable示例、安装依赖、调用真实模型或修改Pi源码。

| 参考 | 来源与关键事实 |
|---|---|
| [reference](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/coding-agent/src/experimental/durable/README.md) | 实验 coding-agent以单进程持有ModelRuntime、Harness、SQLite和TUI。明确缺少普通扩展、prompt templates、图片、login、会话选择器、fork/tree UI；不是普通RPC模式的自动升级。 |
| [runtime](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/coding-agent/src/experimental/durable/runtime.ts) | ModelRuntime.create、SettingsManager.create→Harness.open→root.viewState与taskGraph订阅；DurableController调用submit/abort/compact/configure；harness.resume恢复；close不把运行任务写成用户取消。 |
| [tui](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/coding-agent/src/experimental/durable/tui.ts) | runDurableTui只调用controller并订阅plain view；渲染pi.live/pi.inbox/pi.agent/pi.usage。可复用这种所有权划分，但不把Pi的TS terminal组件搬进pig。 |
| [setup](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/coding-agent/src/experimental/durable/harness-setup.ts) | 官方CodingTools、NodeExecutionEnv按cwd缓存、设置getter和HTTP初始化。例子使用一些coding-agent内部imports，不能当作稳定公开SDK接口。 |
| [sessions](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/coding-agent/src/experimental/durable/sessions.ts) | 目录按canonical cwd分组、每session一个SQLite；proper-lockfile保持单所有者，失效锁等待后恢复。示例位于Pi experimental目录，pig需另用产品隔离目录。 |
| [subagent](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/coding-agent/src/experimental/durable/subagent.ts) | 前台子会话归tool task所有，通过ownerTaskId找回原子会话和固定requestId；父调用取消传播到孩子。 |
| [prompt](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/coding-agent/src/experimental/durable/prompt.ts) | 按selected tools构建系统提示、载入AGENTS/context files与skills；其中system-prompt/tool contributions的private imports是公开API验证缺口。 |
| [events](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/durable/src/harness/events.ts) | watchEvents提供原子snapshot+commit派生事件批次；超出积压窗口时发snapshot。message_update是changes数组，tool output有trim/append/set；不能直接套classic事件解码。 |
| [tool recovery](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/durable/src/harness/tool.ts) | 持久intent先于execute；恢复只在已存策略与当前tool策略均safe时重放，默认unsafe，其他返回interrupted。不会让外部副作用自动获得exactly-once。 |
| [JSON transport example](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/durable/test/examples/19-json.ts) | 公开watchEvents或Conversation.watch可经JSONL输出；无需截获stdout、修改Pi RPC或读SQLite内部表。 |
| [inbox example](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/durable/test/examples/20-inbox.ts) | busy时支持steer/follow-up/reject、Submission.abort撤回队列项；queue权威为durable inbox，非adapter镜像。 |
| [background agent example](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/durable/test/examples/23-subagent-background.ts) | 背景anchor task持有子会话；reporter task用稳定requestId向父会话回报，避免重启后重复提交。 |
| [storage](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/durable/src/storage/sqlite/node.ts) | 官方Node SQLite adapter使用node:sqlite、WAL、默认synchronous=NORMAL；事务串行、5s busy timeout。process crash与断电耐久性需区别验收。 |
| [public exports](https://github.com/earendil-works/pi/blob/b2363841a525bec5bdfcf4361fa7a1730076f5c5/packages/durable/src/index.ts) | Harness、watchEvents、ConversationView、TaskGraph、ToolTask、registry、defineTool等是公开接口；固定SDK版本，不跨包读取src/internal。 |

## 当前 pig 的接入位置

- CLI：`grok_pi/cli.rs`，已增加互斥`--durable`/`--no-durable`与显式`--durable-background`，F2设置默认关闭。
- composition：`crates/codegen/xai-grok-pager-bin/src/bin/grok-pi.rs`。
- `run`目前在bootstrap前准备config skill、物化host桥接扩展和发现经典资源；模式分流需位于这些动作之前，默认分支维持当前PiAgent。
- transport：`crates/codegen/pi-grok-adapter/src/pi_rpc.rs`，普通Pi RPC专用，含bootstrap/relaunch/EOF语义。
- backend：`pi_adapter.rs`的PiBootstrap/PiAgent直接绑定PiRpc；不能把Durable的agent event当作相同协议。
- rendering：native Pager已具备prompt、SessionPicker、工具卡片、diff、QuestionView、任务面板和dashboard；应复用。
- 经典会话树以Pi SessionManager/leaf为权威；Durable以conversation/entry/fork关系为权威，导航语义需显式区分。
- 当前队列、Plan/Goal、Loop、Bash后台任务包含自有状态；新Durable模式不沿用它们来调度官方任务。
- F2已具备下次启动生效的设置：`xai-grok-pager/src/settings/registry.rs`的`SettingMeta.restart_required`，`settings/defs.rs`的`pi_bash`实例，以及`app/dispatch/settings/ui.rs`保存后的restart提示。`grok_pi/runtime_config.rs`已用`load_effective_config_disk_only()`读取启动配置；可复用这条链路增加模式设置，当前已增加`pi_durable`行，并区分当前运行模式与下次启动选择。

## 包兼容结论

普通coding-agent ExtensionAPI函数工厂与Durable defineExtension对象是两种契约。官方实验例子明确未接入普通扩展；源树中Durable package也没有MCP/Codemode实现路径。不能用当前13个包已安装来推导它们能在Durable下运行。

可以作为数据复用的包括主题JSON、技能Markdown和已批准的上下文文件；运行扩展、工具、context修改、子代理、MCP和Codemode分别做适配与恢复安全验收。13个已安装包的迁移表见SPEC，不执行这些插件以获取“只读”证据。

Node>=22.19.0满足两个SDK的声明。普通Pi1.0.4与Durable SDK应使用锁文件固定兼容组合，系统Pi安装版本不能单独作为Durable capability版本。追加只读核对：`npm view @earendil-works/pi-durable@1.0.4 version exports --json`确认该版本及公开env/tools/testing/SQLite子路径已发布；没有安装或执行SDK。

## 2026-10-08 published SDK 对照更新

原始源码快照 b2363841 的 package.json1.0.4 不等于当时发布的 npm1.0.4 API；部分内容已经是后来1.1.0的未发布改动。[三仓库审阅](../../upstream/20261008-pi-dwsy-grok-build-REVIEW.md) 现从正式 v1.0.4 SHA起检查。Host/lock/capability与包版本统一升级1.1.0，真实旧SDK临时store重开、时间字段与分页方向已验证，Node public exports继续使用。当前安装与验收见[适配PLAN](20261008-reference-adaptation-PLAN.md)，普通插件/高级能力缺口保留。
