# T1 Action 可达性清单（source inspection + Root scoped runtime 验证）

中央入口：`crates/codegen/xai-grok-pager/src/app/dispatch/router.rs:175` 的 `stock_product_action`，在 `dispatch:250`、`reconcile_foreign_resume_launch` 前拒绝。声明 63 个 Action 变体：56 个无条件 stock 变体 + `SuspendForEditor(refresh_agents_modal=Some(_))` 条件变体 + 6 个 feature-gated 变体。拒绝均只给 toast，返回零 Effect，不创建 modal/写 config/发 backend 请求。stock profile 不走该拒绝。

| 类别 / Actions | 实际目标或副作用 | 原已 guard | 本次 T1 |
|---|---|---|---|
| Login, Logout, SwitchAccount, CancelLogin, SubmitAuthCode, CopyAuthUrl, ShowRawAuthUrl, HideRawAuthUrl | dispatch/auth.rs → Authenticate/Logout/SwitchAccount/PollAuthUrl/SubmitAuthCode/CancelAuth，以及旧 Grok auth UI 状态 | 无统一 external guard | 中央拒绝；Pi auth 扩展的 live slash 命令仍走 Pi |
| CheckSubscription, OpenSupergrokUrl, RetryCreditLimitPrompt, ManageBilling, ShowUsage, OpenManagedConnectors | billing/status/effects → subscription、x.ai/billing、x.ai/usage 或 grok.com/connectors 链接 | ManageBilling 已 PS-02 guard；其余没有统一拒绝 | 中央拒绝；保留 Pi session stats/context |
| ShowPrivacyInfo, SetCodingDataSharing, PrivacyBannerOptIn, PrivacyBannerOptOut | status/effects → x.ai/privacy/setCodingDataRetention、consent state | 前两项已有 PS-02 guard；banner eligibility 没有 profile guard | 中央拒绝；event_loop rollout !external；app_view privacy_banner_should_show external=false |
| ShareSession | status::dispatch_share_session | 已全局禁用，零 Effect | 仍中央拒绝，避免以后恢复 stock 分享时进入 Pi |
| AnnouncementsHide, AnnouncementsShow, AnnouncementsOpenCta | PersistAnnouncementsHidden / stock promo URL | startup fetch已有 !external；Actions无guard | 中央拒绝；产品 ReleaseNotes 本地呈现保留 |
| OpenFeedbackModal, SubmitFeedbackModal, RequestFeedbackDraft, SendFeedback | notes/effects → Grok反馈 modal、draft 文件读写、提交/trace | 无统一 guard | 中央拒绝；palette 两种feedback变体不在白名单 |
| EnterRememberMode, SendRememberNote, SaveRememberNoteFromModal, OpenMemoryModal, PersistMemoryFullscreen | notes/effects → RewriteMemoryNote/SaveMemoryNote；OpenMemoryModal 把/memory发进Pi prompt | 无统一 guard | 中央拒绝；palette Memory不在白名单 |
| McpAuthTrigger, McpSetupSubmit, RefreshMcpList, UpsertMcpServer, DeleteMcpServer, ToggleMcpServer, ToggleMcpTool | router/effects → stock x.ai/mcp/* | 无 | 中央拒绝；Pi实际 /mcp live命令仍允许 |
| ExecuteHooksAction, ExecutePluginsAction, ExecuteMarketplaceAction, ToggleSkill, RequestBundleStatus, ViewCatalogEntry | stock x.ai/hooks/action、plugins/action、marketplace/action、skills/toggle 与 bundle/catalog | 无 | 中央拒绝；Pi资源走 OpenPiConfig/PiControlRequest，保留 |
| OpenExtensionsModal(tab) | transcript::extensions_modal_tab_fetches 原先无论tab同时FetchHooks/Plugins/MCP/Skills/Workflows/Marketplace | 无 | external仅接受实际live workflows命令的Workflows tab；仅FetchWorkflowsList；其他tab拒绝 |
| ReloadSkills | 原先同时FetchSkillsList+FetchWorkflowsList | 无 | external仅在单Workflows modal、live能力启用时FetchWorkflowsList；其他external调用无Effect |
| WorkflowLaunch, WorkflowManage | adapter/pi_adapter/agent.rs 有x.ai/workflow/launch/pause/stop实现 | handler确实存在；目录原未要求live capability | 保留；external以live workflows gate为权威；T6仍决定迁移或删除 |
| OpenNewWorktreeDialog, NewWorktreeSession, PickSessionInWorktree, PickContentSessionInWorktree, DashboardToggleWorktree, DashboardConfirmWorktree | 未适配Pi的stock worktree对话框/创建或resume路径 | external welcome已hide_new_worktree，但Actions未统一guard | 中央拒绝；普通Pi dashboard、子代理、cwd选择保留 |
| AcceptConsent, TrustFolder, OpenConsentLink, ConfirmWelcomeLocalWorkspaceAck(local-workspace), AgentTypeMismatchAnswered | stock启动consent/trust/workspace/agent选择路径 | production external启动不进入Grokauth；Actions无统一guard | 中央拒绝；Pi资源trust走PiControlRequest不受影响 |
| OpenConfigAgentsModal、ImportClaudeSettings/Confirm/Cancel/Dismiss(stock-runtime)；SuspendForEditor(refresh_agents_modal=Some(_)) | stock agent配置/导入/编辑后刷新stockagents | compile profile只能部分隔离；stock+external组合仍可达 | 中央拒绝；普通终端/Prompt/Pi配置editor保持 |
| OpenGboom, OpenHowtoGuides | stock gboom / Grok how-to链接picker | 无 | 中央拒绝；Pi help/hotkeys/tutorial/local diagnostics保持 |
| KillBgTask, KillSubagent, CancelScheduledTask, DemoteToBackground | adapter实际有x.ai/task/kill、subagent/cancel、scheduler/delete、terminal/background handlers | 已实际接Pi | 保留，未按x.ai前缀误删 |
| ToggleYolo, SetYoloMode, SetPermissionMode、voice及13项stock settings setter | stock权限/语音/配置写 | PS-01/02已有handler guard；registry目录allowlist已存在 | 保持原guard；CycleMode实际external→PiPlan保持 |

## 非Action产品开关

- event_loop 两个plugin_cta赋值均 !external；末尾旧赋值不再受GROK_PLUGIN_CTA=1覆盖。
- workspace_dashboard_enabled !external，GROK_WORKSPACE_DASHBOARD=1不能开启stock工作区dashboard。
- privacy_notice_rollout !external，加AppView visibility后备guard，GROK_PRIVACY_NOTICE_ROLLOUT=1不能显示训练banner。

## 目录边界

- External palette变体白名单：NewSession、Home、Quit、EditPromptExternal、KeyboardShortcuts、OpenSettings、SectionHeader；SlashCommand必须存在于当前profile/live registry；Workflows tab只在live workflows存在时出现。其他变体默认拒绝。
- registry明确持有external profile；未知mode_support=Unsupported，stock维持Both。Dynamic palette最终集合再次按live registry过滤。
- Workflow-only状态的render/键盘/鼠标/直调switch_tab共享仅Workflows的tab集合；不会发stock目录请求。
- 当前existingF2/Web settings白名单保持不变。

## 测试与验证边界

新增实际palette集合相等、unknownmode拒绝、stockActions无Effect且不改状态、Workflow feature/livegate与单一backend、训练banner强制override隐藏测试。rustfmt解析、git diff --check exit0；Cargo/build/PTy由Root统一运行，未调用真实账号/模型/网络。

## Root 最终回执

完整 external_ filter 60/0、bin98与组合verify通过，4个原生PTY/nativeExit0通过；代表性Action无副作用/实际palette集合/Workflow fresh-live/训练banner覆盖已执行。不是63种复杂Action构造均独立运行，也不是全Pager suite。详见总纲PLAN。
