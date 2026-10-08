---
id: "2026-10-07-pi-durable-integration-plan"
title: "Pi Durable 深度接入计划"
status: "in-progress"
created: "2026-10-07"
category: "architecture"
---

# Pi Durable 深度接入计划

依据 [SPEC](20261007-pi-durable-integration-SPEC.md) 和 [官方源码核对](20261007-pi-durable-integration-SOURCE.md)。用户已授权按本PLAN实施，采用F2默认关闭的配置与CLI覆盖。保留当前dirty四组裁撤与本地pig，不commit/push/worktree、不执行收费模型、不迁移真实会话；安装固定SDK到项目运行时目录，测试数据使用隔离临时目录。

## 阶段与退出条件

| 阶段 | 工作 | 退出条件 | 当前状态 |
|---|---|---|---|
| D0 | 固定官方源码、参照TUI所有权、能力缺口、SPEC/PLAN和可选mode规则 | 设计可审阅；明确F2持久配置/CLI覆盖、关闭保证、会话边界及实际代码接入位置 | 完成 |
| D1 | 公共API与安全恢复spike | 锁定SDK、存储/HTTP/auth/prompt接口、safe/unsafe恢复事实可运行；不偷导内部路径 | 完成，SDK公开API/SQLite恢复实测 |
| D2 | F2配置/CLI模式解析、启动分流、最小headless host与独立协议 | 配置默认false，关闭分支不初始化Durable；faux provider、SQLite、单owner、admission/状态/watch/close故障注入通过 | 完成，F2/CLI/独立host已接通 |
| D3 | ACP投影、会话mode边界和原生交互 | 新模式下snapshot/流/工具/队列/任务图/模型/压缩可用，重连不重复；入口与恢复提示准确 | 完成，host→ACP与原生PTY已通过 |
| D4 | 工具/插件/数据兼容 | 逐项capability和实测覆盖，不运行未适配旧扩展；旧store保持完整 | 部分完成；工具/静态数据/能力隔离可用，插件与高级功能待适配 |
| D5 | 后台owner与任务所有权 | UI detach不取消后台任务，前台abort/后台abort语义、进程树和恢复决策可验证 | 完成当前范围：显式Unix owner，重连；无launchd监督 |
| D6 | 两种模式的平台交付 | 真人体验、版本升级/退出路径、文档和macOS包；DU-01~12按证据层验收；出厂配置持续为Pi RPC | 实验包自动验收完成；真人provider/OAuth及高级适配未测 |

每阶段回写实际源码、退出码、store/SDK版本和证据。可选mode约束已登记总体SPEC/PLAN；实现中的具体协议和运行时图继续回写，不把此设计当作T2–T8完成。未来默认切换不属于本项。

## D1 公共API和恢复

1. 以固定官方1.0.4为参考，核对发布的exports与源码；实验文件路径仅是研究材料。为host建立最小锁文件，复用官方包和现有工具链，不复制Pi源码。
2. 验证ModelRuntime、SettingsManager、Harness、NodeExecutionEnv和SQLite的公开组合；检查代理、HTTP超时/SSE setup缺口。先用faux provider验证，无真实密钥。
3. 源码缺少Codemode/MCP的Durable接入；单独验证公开嵌套任务执行路径和现有官方client可否适配。无公开入口则记录阻塞，不造第二套工具运行时。
4. 明确旧ExtensionAPI与Durable Extension契约，以及image/tool/UI/资源信任限制。首版只安装明确Durable-compatible的registry项。
5. 存储测试区别进程崩溃与断电假设，锁排他/失效锁恢复、安全pragma和升级策略均记录。

输出：可运行的小spike、API/capability核对表、恢复测试。没有这些证据不先编写巨型兼容层；默认模式始终保持Pi RPC。

## D2 最小后端

1. 复用F2现有registry/持久化链路，增加`[ui].pi_durable`，默认false、`restart_required=true`和中英文说明。`grok_pi/cli.rs`增加互斥`--durable`/`--no-durable`；`runtime_config`按CLI→effective config→false解析。`grok-pi.rs::run`在经典桥接物化/资源发现/bootstrap之前分流。关闭时保持现有PiAgent，SDK不存在仍能启动，不能自动创建Durable目录/DB/锁或启动Node host。新模式逐项校验参数，未支持的`-e`/passthrough等明确报错。
2. 提议新增 `runtime/pi-durable-host`与composition的`grok_pi/durable_host.rs`，只配置并调用官方SDK。renderer/键盘都留在Rust。构建时锁定依赖并生成带许可的host包，仍只交付一个grok-pi产品入口。
3. 接入product-isolated store；一个store只开一个owner。共享Pi auth/models/settings通过公开接口，credentials不写SQLite。
4. 独立handshake和typed commands；backend/store/conversation/task/request/submission的身份不混用。不宣称它是官方Pi RPC。依赖/版本/锁错误不得触发经典Pi自愈或模式回退。
5. 复用公开snapshot/watch；传输重试保留submission requestId。连接断开时恢复观察，而非重新提交上一条输入。
6. 首版UI-owned生命周期：pause/close保留pending，abort才终止工作；对应CLI帮助和原生菜单明确。

退出：关闭时隔离守卫DU-10、F2保存/CLI覆盖的DU-12，以及headless真实SQLite/本地synthetic工具的DU-02~04通过，再进入UI实现。单独新增设置或CLI开关不算Durable支持完成。

## D3 原生界面

1. `pi-grok-adapter/src/durable` 的ACP实现只投影；现有PiAgent持续作为出厂默认后端。composition由解析后的F2设置/CLI覆盖选择新模式。
2. raw snapshot重置历史/活动状态，message changes与tool output trims正确转换；稳定backend实体ID防重放重复行。
3. 复用PromptWidget、SessionPicker、工具卡片/diff、banner、QuestionView和任务/dashboard组件；不存在另一套TS TUI或ASCII fallback。
4. 队列直接读取官方inbox；已提交输入撤回用Submission API。草稿留在UI，不把原queue_bridge套进新模式。
5. 任务视图展示TaskGraph的owner、等待、背景anchor；可切换到子conversation。Esc作用当前conversation，不默认杀全部背景任务。
6. 模型/thinking/configure和compaction状态真实往返；usage由官方doc。会话树按Durable fork关系显示，capability声明不等同经典leaf navigation。
7. `--continue`和`/resume`只查当前后端；两种模式拒绝跨后端locator。两种恢复提示分别包含`--durable`/`--no-durable`，不受后来保存配置改变影响；不写入或转换普通Pi JSONL。
8. F2两种模式都可保存模式开关，区分当前运行与下次启动选择；沿用restart提示，不立即迁移会话或取消任务。其他slash、palette、F2、Web配置、快捷键按host capabilities一起过滤，未适配插件和经典配置不被执行或写入；`--print-capabilities`按启动模式解析返回实际支持。

## D4 兼容和迁移

按用户日常能力优先：

| 优先 | 范围 | 完成标准 |
|---|---|---|
| 1 | read/write/edit/bash、中文设置/模型、会话/任务/取消 | 参数/结果/diff/限制和恢复策略都有真实host→ACP/PTY |
| 2 | Codemode、MCP、tool-search、资源信任、图片 | 采用官方公开能力，嵌套任务归属/取消/去重正确；不自建Eval或MCP client |
| 3 | auth/OAuth、templates/skills、web配置 | 共享Pi凭据生命周期，原生交互取消正确，无另存密钥 |
| 4 | Web Access、Multi-edit、记忆/Curator、Ponytail等 | 工具/数据/hooks分别适配，不笼统标“所有Pi包兼容” |
| 5 | Plan/Goal/Loop/Subagents与外部团队 | 选定一个owner体系，checkpoint/报告ID持久，旧timer/进程不冒称可恢复 |

旧会话在默认模式原样列示；Durable模式只列自己的store，关闭Durable后旧历史继续可用。默认不导入。若以后需要导入，先指定新store、仅复制finalized语义与来源映射，使用官方事务API，验证原文件hash不变；不搬迁in-flight活动句柄。

## D5 后台执行

同一个headless host改为显式后台owner；一个store一个本地socket，前端只是client。macOS优先评估原生launchd服务管理，不新建通用daemon框架。断开UI、owner暂停、用户abort分别验证。

参考官方前台子会话例子和23-background：ownerTaskId找回子会话，固定requestId避免重启后重复spawn/报告，anchor/reporter表达后台工作。执行环境必须验证父进程崩溃后的shell/MCP子进程事实，不按PID猜测或自动重复外部副作用。

安全恢复决策写在Durable应用doc/官方hook中；如果工具可能部分执行，先阻止盲目危险重试，再以原生对话框让用户明确继续、检查或终止。首次实施不扩展远端/Cloudflare多租户接口。

## 验证矩阵

| 层 | 关键用例 |
|---|---|
| SDK no-inference | real SQLite、持久task计数、相同requestId、safe/unsafe、close/reopen、fork/owned children、cancel-wait不cancel-work |
| 故障注入 | admit前后kill、ACK丢失、tool intent/结果提交边界、slow consumer snapshot、owner重复打开、未知子进程/副作用 |
| host→ACP | message变化、output trim/set、历史reset、inbox/submission完成、TaskGraph、跨conversation切换 |
| native PTY | prompt/steer/follow-up、撤回、Esc/close/detach、任务图、restart恢复、模型/压缩、tool/diff、旧会话入口 |
| 真人 | 当前provider/OAuth、网络代理、真实工程写入/工具、长任务离线/恢复；对平台和SDK版本分别记录 |
| 数据安全 | 原JSONL和credentials hash、目录/锁/socket权限、备份/版本升级、无静默格式转换 |
| 模式隔离 | F2关闭/未配置或CLI强制关闭时，在SDK缺失时仍走Pi RPC；Durable目录/进程零初始化；现有插件/PTY回归；跨mode locator拒绝、`--continue`/恢复命令准确 |
| F2配置 | false默认、开启/关闭保存与失败回滚；CLI覆盖及互斥；项目effective config优先级；重启提示、当前模式不变、两种模式的设置行均可见 |

历史设计阶段仅核对文档；实施阶段按下表运行实际SDK/Cargo/PTY。每层通过只代表该层，源码SPIKE/静态检查不等于已支持无人值守或默认替换。

## 本轮设计回执

- 已固定官方main `b2363841a525bec5bdfcf4361fa7a1730076f5c5`，下载研究材料到 `/tmp/pi-durable-design-reference`，仅作只读参考。
- 已阅读experimental durable的runtime/controller/view、TUI、锁/会话、prompt/settings、前台Subagent；以及公开event/watch、recovery、inbox、背景Subagent、SQLite和tool replay代码。
- 当前pig安装和已有四组裁撤修改保持；没有运行参考示例、安装SDK、修改业务数据库、调用真实provider或实施新后端。
- 已回读三份设计文档；5处本地引用、14处固定源码链接已对照文件及官方git tree核对，Markdown代码围栏与空白检查通过。已在总体PLAN登记草案；`git diff --check`通过。以上仅为文档检查，尚无Durable运行时验收。
- 后续补充：按用户要求确定可选mode为交付目标，取消默认切换阶段，补齐启动分流、参数/会话边界、capability入口及DU-10/11。当前代码入口已核对，运行时实现尚未开始。补充后的引用/围栏/空白与总纲登记检查通过，`git diff --check`通过；未执行运行时测试。
- F2补充：取消CLI唯一入口限制；复用已存在的`restart_required`设置和保存提示，以F2默认关闭的持久选择+CLI单次覆盖接入。补齐DU-12和恢复命令对已保存配置的隔离。设置/后端代码仍未实现，当前pig未改变。

## 实施回执（2026-10-07）

- 已实现`runtime/pi-durable-host`，锁定四个官方SDK为1.0.4。Harness/registry/CodingTools/NodeExecutionEnv/ModelRuntime/SettingsManager/SQLite均走公开exports；HTTP仅用Undici正式API，无内部路径或prototype patch。
- 运行模式在任何经典桥接物化前分流；`[ui].pi_durable=false`为出厂默认。F2复用保存/回滚/重启提示；CLI单次覆盖，跨后端session标识拒绝。
- 独立JSONL协议与headless DurableAgent接入现有ACP/Pager。SDK提交与完整事件批次的UI屏障分开，防止回执提前导致丢显示。官方inbox直接投影，支持steer/follow-up与撤回；编辑/重排不伪造为已支持。
- `/tasks`复用原生Picker，展示task/owner/等待关系，Enter使用现有工具卡片查看记录；会话列表含当前store的子会话。官方owned Subagent使用固定requestId，取消等待不取消工作。
- 恢复仅在安全策略满足时自动继续；不安全工具需明确continue/abort。SIGKILL的工具intent和一次外部效果实测不会被重复执行。使用官方SQLite默认WAL/NORMAL，仅证明进程崩溃恢复，不是断电保证。
- 显式后台owner使用同一controller和Unix socket，单owner/单UI，detach不abort，空闲退出。不会自动部署launchd任务。
- Rust adapter unit206/0，bin91/0，F2专项1/0、设置中英文4/0；真实host→ACP与后台连接2/0；SDK测试和原生PTY见验证记录。生产隔离check及stock check通过。
- 已发布v0.1.10与已安装`0.1.10+dirty`保持，未commit/push/安装新二进制/迁移真实会话/执行收费模型。

剩余工作：普通扩展和Codemode/MCP/图片/Plan/Goal/Loop的公开接口与契约适配，完整树/fork/UI/auth，launchd自动重启和真人体验。当前模式按capability明确禁用这些项，不能把实验交付称为全部PLAN或总体T0–T8完成。

平台交付回执：优化构建`0.1.10+durable-dev`成功，macOS arm64包位于`/tmp/grok-pi-0.1.10-durable-dev-macos-arm64.tar.gz`，SHA256 `2c29c8ccde023d8e454bd3b43dfeb7d2da5a58367a18376be0f95b5d9c238ecd`。包含重定位并签名验证的libiconv/libz，以及完整公开SDK布局和许可。已安装pig仍为SHA256 `c9da5cbce0c0ce6e3227bd66ea12733a85f8101fba5abe9577ce4547b000cf11`。

最终补充：SDK 9/0；forced SIGKILL 的 signal 明确断言为SIGKILL，旧锁等待后恢复，外部效果仍仅一次。打包后的真实SDK原生PTY通过（live/F2保存/继续当前模式/重启恢复/退出），证据`/tmp/grok-pi-durable-packaged-final-pty.log`。正常Pi RPC在不存在Durable host路径时通过product-surface PTY；另Codemode/signal/timeout/EOF四例通过。退出前取消UI回执等待、保留RPC用于pause/detach，避免关界面后等待不存在的renderer回执。

## 2026-10-08 参考适配后续

Host四个官方SDK升级1.1.0，并随本机优化开发pig安装；用户Grok/Pi设置不变。已知1.0.4 store通过官方SQLite重开，未知版本拒绝，未改用户store。[参考适配PLAN](20261008-reference-adaptation-PLAN.md)记录11SDK、2ACP、真实旧SDK fixture、原生 live/reopen/F2/OSC与打包/安装证据。此增量没有完成普通插件、MCP/Codemode/images/Plan/Goal等既有Durable适配工作。
