---
id: "2026-10-03-pi-deep-adaptation-plan"
title: "Pi 1.0 深度适配 PLAN"
status: "in-progress"
created: "2026-10-03"
updated: "2026-10-03"
category: "architecture"
---

# Pi 深度适配执行 PLAN

依据：[SPEC](20261003-pi-deep-adaptation-SPEC.md)。起点干净 `main@84174917`，实际系统 Pi 1.0.0。授权本地实现和分阶段 commit，不 push/worktree，不改 Pi core 或真实业务数据。

| 阶段 | 要求 | 执行与验收 | 状态 |
|---|---|---|---|
| P0 | DA-01/02 | disposition、事件并发和有界旧 host probe；官方runtime controls | 自动SDK/RPC/nativePTY PASS |
| P1 | DA-03/04 | 官方CLI/SDK声明base、admission重算、officialreload或EOF/restore、Web条件保存 | 最新生产native install/remove/registry/history/cancel PASS；明确safe defer/loader/CAS边界 |
| P2 | DA-05/06 | selected/dispatched/usage，official image/classifier | 合成actualSDK/RPC/live-replay/nativecard PASS；真实图片仍DA10 pending |
| P3 | DA-07/08 | UI支持表、workingstatus、scoped facade、RemoteTUI生命周期 | namedcontracts/actualnative resize/input/dispose PASS；不保证所有第三方 |
| P4 | DA-09/10 | exactsource、profiles/graph/build/verify/4PTY、realSDKchat | DA09自动PASS；单次SDK真实chatPASS，DA10其余pending，整体goal不complete |

## 执行约束

- Root 维护总体授权与审计；Cargo/build/最终 commit 只有一个集成 runner（最初 Root，现明确委任 `/root/pi_full_coverage_check`）。其他子代理按文件范围并行；独立读取可并行，Cargo/构建与重叠写入按依赖顺序运行。
- 先复用已有 adapter/tests、native modal/action、extension transport 和 Pi APIs；不创建通用兼容框架、包管理器或第二 renderer。
- 每完成阶段记录实际 exit、log、proof layer 和 commit。现有 664-phase 身份声明保持上一轮证据；新增/变动源码逐文件审阅声明，不覆盖历史源基线。
- 使用共享 Cargo 输出和既有 20GiB free floor；只在失败或新增修改需要时重验。
- 真实模型与 OAuth 的用户选择不阻塞独立实现；真人 consent 不自动代办。必要真实验收缺失时列出具体步骤，不能把 synthetic PASS 当成真实 PASS。

## 日志

### 2026-10-03：SPEC / PLAN

- 已确认工作树干净、branch main、HEAD84174917；上一轮 goal 已完成，本轮创建独立增量 goal。
- 已调查 lifecycle 丢弃成功 disposition、资源面板缺 package 生命周期闭环、virtual dispatched metadata/非chat模型 proof、RPC UI no-op和现有实验 host 私有补丁。
- 先实施 P0；P1、P2 的独立模块并行准备。真实验收环境与账号选择在可检查的验收脚本就绪后向用户确认。

### P1：Package / reload / Web revision

- Native modal 保留 Global/Project 资源树、搜索、预览、启停和继承覆盖；增加 `i` 安装、`d` 移除确认、`u` 单 identity 更新、`U` 全 package 更新。首次 actual snapshot 到达前禁止修改；busy/cancel/failed/reload-pending 分开呈现。adapter 使用当前 SpawnConfig 的 Pi executable/prefix/env/cwd，不带 RPC 启动参数，不构造新 package manager。
- 官方 CLI 负责 source identity、npm/git pins、对象 filters、Project precedence 与 local source 存储。`update_all` 明确使用 `pi update --extensions`，不运行裸 `pi update`。实际官方 extension snapshot 通过 fresh SettingsManager/DefaultPackageManager 读取 declarations，并用当前 `pi.getCommands()/getAllTools()` 读取 registry；正常结果是 `registry_refreshed`、`reloaded:true`、`loaded:null`、`loadStatus:unverified`。
- `python3 crates/codegen/pi-grok-adapter/tests/pi_packages_contract.py`：exit 0。独立临时 agent/project/local package、`PI_OFFLINE=1`，没有模型调用/真实 package 写入；证明安装声明与运行 command 分离、reload 后 command 出现、filters/未知字段保留、Project 覆盖、更新目标、local remove 不删源目录。
- Invalid relative import fixture：Pi 1.0 的 official `ctx.reload()` 仍返回成功，实际 registry 无 fixture command，原始 `stderr=[]`、RPC `extension_error=[]`。Pi loader 的内部 errors 不在公开 RPC 中；stderr checkpoint 只读本次有界可见诊断，不据此声称完整捕获。未引入第二个 loader，也未修改 Pi core；界面不把 registry refresh 标成全部 package 加载成功。
- Web 原本已有 GET preflight + semantic equality；本轮新增服务端 settings raw-byte revision、HTTP `If-Match`、写入前同步比较（缺版本 428、已变化 409）。`bun test extensions/pi-grok-web-config/tests/config-store.test.ts extensions/pi-grok-web-config/tests/settings-conflict.test.ts`：7 pass、0 fail、195 assertions，exit 0；证明 native/package 变更后的旧 draft 被拒绝，fresh draft 保留未知字段。
- 公开 SDK 未 export FileSettingsStorage transaction；同步 compare/rename 阻止 server 内交叠写入并拒绝已发生的外部变化，但不能完全排除绕过 server 的外部同时写入。不把原子 rename 写成全外部 CAS。

### P4：来源与证据收尾进行中

- 保留 3797 个原始 Git blobs、冻结 `222d614d` 历史 4479-file integration、上一 phase `84174917511690447ba32915571e9748b5e10ad4` 的 Git 来源与旧记录；新 phase 逐文件声明，不允许目录豁免。源代码仍在统一验证修复期，最终 SHA pin 等待 root 明确 `SOURCE FREEZE`。
- 本轮真实 provider、OAuth/浏览器、实际图片生成及目标终端真人体验尚未验收。synthetic/fixture PASS 不代替 DA-10；用户未明确延期前整体 goal 不标 complete。

### Root runner checkpoint（后续修复仍需重验）

- `./scripts/cargo-shared.sh test -p pi-grok-adapter --lib`：202 passed，exit 0，log `/tmp/grok-pi-deep-adapter-lib-20261003.log`。
- `./scripts/cargo-shared.sh test -p pi-grok-adapter --test pi_disposition`：1 test / 4 scenarios，exit 0，log `/tmp/grok-pi-deep-disposition-20261003.log`；`--test pi_lifecycle -- --ignored`：actual Pi 1 test，exit 0，log `/tmp/grok-pi-deep-lifecycle-20261003.log`。
- `./scripts/cargo-shared.sh test -p pi-grok-adapter --test pi_model_projection -- --ignored --nocapture`：actual Pi virtual dispatch/context/cost + image/classifier Codemode live/replay ACP，1 passed，exit 0，log `/tmp/grok-pi-deep-model-projection-20261003.log`。
- Production `check -p xai-grok-pager-bin --bin grok-pi --no-default-features --features jemalloc,sandbox-enforce`：exit 0 / 38.52s，log `/tmp/grok-pi-deep-production-check-20261003.log`。这些日志为该次 source checkpoint；后续 ACK/scoped probe/offline/canonical-cwd 修复及最终 build 需要验证最新字节。
- Pager `--no-default-features --lib pi_control`：被 167 个 feature-dependent 既有 lib-test compile errors 阻断，log `/tmp/grok-pi-deep-native-controls-20261003.log`；未扩大范围修无关 fixtures。root 改用 default-feature focused native test；生产隔离仍由独立 no-default check/graph 证明。
- Root 增加 official reload `responsePath` 业务 ACK：缺失或 false 拒绝；RPC 成功 disposition 不冒充 reload lifecycle 完成。hidden loader.errors 仍未知。handled 晚 response 只完成目标 slot/reservation，legacy probe 绑定 operation id，不能清继任工作。package CLI 单独保留 `--offline`→`PI_OFFLINE`，并接受同一 canonical cwd 的 macOS 路径别名。
- Sourceguard 窄加 production `runtime_command` 的显式 `set_auto_retry/set_auto_compaction/abort_retry` 映射，拒绝缺 generator/未知命令/未知 computed type，并对照 installed Pi declarations；不是任意动态 Rust 调用的全覆盖扫描。共享 helper 的 4 个负向 drift 检查通过；mock 新增此 contract，8 checks / mock exit 0，report `/tmp/grok-pi-deep-mock-runtime-20261003.json`。
- Default-feature native focused tests 均 exit 0：`pi_control` 2、`runtime_arguments` 1、`views::pi_config` 16、`package_modal_routes` 1、`physical_context_window` 1、`session_info_renders_physical_model` 1、`remote_tui_resize_is_scoped` 1。log `pi_control` 为 `/tmp/grok-pi-deep-native-controls-default-20261003.log`，其余为 `/tmp/grok-pi-deep-native-<filter，将 :: 换为 ->-20261003.log`。test target 共 10145，但本轮未运行全 suite；default features 是共享 native 测试环境，不代替 production no-default check/graph。
- Latest adapter lib 重验：206 passed，exit 0，log `/tmp/grok-pi-deep-adapter-lib-final-20261003.log`；包含 4 个 handled-response races、operation-scoped 有界 probe、late cancel/settled/reservation 回归。202 是前一 checkpoint，不记作最终计数；full adapter suite、ACK integration 与 actual lifecycle 仍由 root 当前 runner 执行。

### SOURCE FREEZE / latest automatic targets

- Root 明确冻结功能、extension、native、injector 与全部 fixture 源码；如后续 PTY 发现问题必须显式解冻，不改实现偷更新 hash。docs 仅填实际结果时单独审阅并重 pin。
- Full adapter suite exit 0：lib 206、`pi_disposition` 1、`pi_reload_ack` 1；其余 ignored targets 不据此声称已执行。log `/tmp/grok-pi-deep-adapter-suite-20261003.log`。
- 单独 actual targets 均 exit 0：`pi_lifecycle -- --ignored` 1（latest ACK/control/offline source）、`pi_native_projection -- --ignored` 1 target / 7 scenarios、`pi_model_projection -- --ignored --nocapture` 1。logs `/tmp/grok-pi-deep-lifecycle-final-20261003.log`、`/tmp/grok-pi-deep-native-projection-final-20261003.log`、`/tmp/grok-pi-deep-model-projection-20261003.log`。
- Latest 生产隔离 binary tests 96 / exit 0，log `/tmp/grok-pi-deep-bin-final-20261003.log`；前述 default native 7 个 filters 均 green。正式 `./build.sh`、graph、终端 PTY 与最终 verify 尚进行中，不能用这些组件 tests 代替。
- 正式 `./build.sh` 完成：exit 0 / 48.97s；binary 184,553,448 bytes，SHA-256 `a288dc04ab49b93630d1bb9c7ba72d255b2d6e95c349d1bc786119790f39b808`。proof `/tmp/grok-pi-deep-artifact-20261003.json`、log `/tmp/grok-pi-deep-build-20261003.log`。源 stamp 是 precommit `fe7515f5` + dirty frozen source，不声称 clean 最新 HEAD；不是受控 cold-build benchmark。fresh 4-case PTY、stock check/graph/combined verify 后续单独记录。
- 此 checkpoint sourceguard 21/21、identity negative（含 review-base/runtime drift）、recursive rustfmt 1659 files / failures={}、mock 8 checks / 33 lines / stderr empty，全部 exit 0。Reports `/tmp/grok-pi-deep-source-final-20261003.json`、`/tmp/grok-pi-deep-syntax-final-20261003.json`、`/tmp/grok-pi-deep-mock-final-20261003.json`；90 explicit reviewed files、phase716，原始3797/历史4479/native830/3147 protected 均保持。后续解冻修复使此结果属于 checkpoint，而非最终新 source。
- Stock binary check exit 0 / 1m55s，log `/tmp/grok-pi-deep-stock-check-20261003.log`；fresh production graph exit 0，**806 packages**、`stock_runtime=[]`（七禁止包全空），report `/tmp/grok-pi-deep-dependency-profile-20261003.json`。上一 phase 805 不沿用为本轮数量。

### Fresh native package 失败与显式解冻

- First readiness fixture 在 `/new` Starting session 时发 runtime command，产品正确拒绝。只解冻 PTY fixture 等待 active-ready，保留失败截图；生产 binary 不因这项 fixture 修复重建。
- 更关键的 package case：install 声明存在，但实际 registry 无新 command/factory，不能以 registry_refreshed 绕过业务缺口。Root 显式解冻 P1 composition/startup resource seams；调查强制 `-ne` / fixed explicit resource paths 对 reload 的影响，必须保留 resource policy、用户 CLI no-extensions 和 trust。
- Installed Pi 1.0 的官方 `resources_discover` 仅提供 skillPaths/promptPaths/themePaths，不含 extensionPaths；不能伪造官方 extension 热加载入口。新实现须 focused proof、新 build 与再次 SOURCE FREEZE，旧 pins/旧 `a288dc…` build 保存为 checkpoint。
- 同旧 binary 的 models/remote/runtime PTY 通过，packages 失败；最终 new-source/PTY/combined verify pending。整体 goal 保持进行中。

### Resource admission correction / 再次冻结与验收

- Composition 新 `resource_refresh.rs` 从官方 SDK resolvedPaths 合并既有 native auto discovery，重算当前 cwd 的 product-isolated admission policy；仅替换自己生成的 extensions/prompts/themes 参数，保留用户显式资源、`no-*` 限制、scope/trust 与已启动 native feature caps。adapter 保持 headless，planner 由 composition 提供，不引入 UI 依赖。
- Admission 未改变时走原 official `ctx.reload()` + 业务 ACK；启动输入改变时，官方 EOF 关闭旧 extension/child scope，再以新 admission args 重启同一 Pi host，使用公开 session/tree/model/thinking API 恢复。会话 JSONL 不作直接改写；没有 persistent sessionFile 的内存历史，以及公开 navigateTree 会移走 user-message leaf 的情况，明确返回 saved/deferred，保留原 process/context，待用户完成响应或手动重启。
- Latest isolated binary tests 97 / exit 0，log `/tmp/grok-pi-deep-resource-bin-20261003.log`。后续第一 RPC fixture 因期望漏默认 `--mode rpc` 失败，保留 `/tmp/grok-pi-deep-resource-rpc-final-20261003.log`；精确修测试 expected prefix，不改变业务断言。
- Latest resource transport targets 均实际 exit 0：`pi_rpc_resource_restart` 2（EOF dispose 完成与失败不假 ACK），log `/tmp/grok-pi-deep-resource-rpc-final2-20261003.log`；actual `pi_resource_restore` 2（persistent session/leaf/model/thinking、新 factory/command，以及 memory/user-leaf defer），log `/tmp/grok-pi-deep-resource-restore-final-20261003.log`。factory trace 换成真正 newline 后才使用 latest 字节验收，未沿用首次草稿结果。
- 唯一 runner 再次明确 SOURCE FREEZE 当前生产与 tests/fixtures；records worker 逐文件 review/pin 后 sourceguard/negative/syntax/mock。新正式 build log `/tmp/grok-pi-deep-resource-build-final-20261003.log` 尚运行；fresh artifact/4 PTY/combined verify 等实际回执，不能提前 PASS。最终一次完整 commit 包含实现、tests、行为 docs、自动验收 receipts 和 exact manifest，DA-10 继续 pending。
- 第二 resource build 已 exit 0 / 37.12s，184,688,280 bytes，SHA `48a533aa433261f7b029157c904ae633c534e6ee25d523e680d3cad7423e0347`，proof `/tmp/grok-pi-deep-resource-artifact-final-20261003.json`。随后发现 SDK `source=auto, origin=top-level` 被误映射为 Settings；此 SHA 和该轮 PTY 保留为 origin 修复前 checkpoint。
- Root 仅显式解冻 `resource_refresh.rs` 修 origin：Package 优先、`source=auto` 为 Auto、其余为 Settings；同 path 的 Settings 禁用覆盖 Auto 的纯回归不读取 HOME。focused `resource_refresh` 2 tests / exit 0，log `/tmp/grok-pi-deep-resource-origin-final-20261003.log`。PTY driver 等真实 paste 文本显示再 Enter，未削弱业务断言。唯一 runner 再次冻结当前候选，下一新 build/4 PTY/verify 实际结果尚 pending。
- Origin 修复后 delivery build 已实际 exit 0 / Cargo 4.74s：184,688,440 bytes，SHA-256 `99610bdaa42330880b916f2107f23dd2a7b8ef46143ee71ff8c3e58da028e7a9`，artifact `/tmp/grok-pi-deep-artifact-delivery-20261003.json`。当前 delivery 4 PTY 运行中，不能提前通过；48a533 与 a288 保留旧检查点。只由唯一集成 runner 运行 build/Cargo/commit，当前 production/fixture 不再修改，除非新业务失败显式解冻。

- Delivery 99610b 的 factory/argv/registry hotreload 已实际通过，但 native notice 把 unverified boundary 放末尾，max140 modal 左pane会裁掉边界；此artifact留为可读性修复前checkpoint。Root只解冻finish_operation notice copy，前置 `Load unverified · registry`，协议状态不变；fixture增加exact visibleboundary断言，其他业务assert不削弱。focused native资源modal16tests/exit0，log `/tmp/grok-pi-deep-pi-config-boundary-final-20261003.log`。唯一runner再次明确最终候选SOURCE FREEZE；下一build/4PTY/verify仍待实际回执。
- Visible-boundary build 1965124f01ca2aae9c076a9faf12674e97d23497fcea6a7ba7e4af0e4f11a0e7 /184,688,344bytes/16.89s、combined verify均exit0，但fresh native remove暴露真实declaration base bug：SDK global source为相对agentDir的`../local-package`，CLI相对cwd执行产生No matching package。verify通过不代替此业务失败；该build/verify保留checkpoint。
- Root仅解冻local已声明remove/update source normalization：从fresh SDK installedPath/settings base生成CLI source，Npm/git/pins/filters不改，install仍相对cwd。3unit tests/exit0（`/tmp/grok-pi-deep-local-package-base-final2-20261003.log`），actual生产package_action install→rawrelative remove→registry无command/local源保留/sessionId,file,leaf,model,thinking保持，与旧restore/defer合计3actual/exit0（`/tmp/grok-pi-deep-resource-restore-base-final-20261003.log`）。后续Project missing-path fallback再按官方`.pi/settings.json`目录核对，不用cwd假替代。

### DA-10：单次真实 SDK chat 有界子证据

- Root依据已授权P4范围及用户Pi已有default选择，授权一次`openai-codex/gpt-6.1-sol` short chat；provider optional偏好未回复不另加费用审批。禁refresh/login/credential-command/persistent写；只内存已有freshOAuth、无session/tools、SSE/no retries、20s总deadline、观察到64-byte文本超阈abort。Codex server不支持maxTokens32硬cap；32仅建议，不保证输出<=32，不能伪称服务端硬约束。
- Nonembedded脚本 `tests/pi_real_chat_smoke.mjs`：preflight requests0/hash unchanged；单次execute实际exit0、HTTP200、requests1、resultOK/stopstop、thinking minimal→sentlow、input21/output5/total26/reasoning0、SDKcatalog cost0.000092（非billing receipt）、abortBoundary null、真实files bytehash不变。Credentials modify/delete在调用callback前直接拒绝，未发生refresh/login/write。
- 这只是official SDK真实provider chat proof，不是native TUI、OAuth真人流程、real image或目标终端真人验收。只更新DA-10 chat子项，其余pending，未获用户明确延期不关goal；独立测试脚本不改shippingbinary。

- Real SDK safe stdout（唯一execute观察值，无第二次调用）已原字段转录 `/tmp/grok-pi-real-chat-sdk-20261003.json`；argv `node crates/codegen/pi-grok-adapter/tests/pi_real_chat_smoke.mjs --execute-once`，cwd `/Users/kyros/WorkStation/grok-pi-tui`，exit0。无execute的preflight requests0/真实文件hash不变，`node --check` syntax PASS。

- 最后同DA03流校正：missing installedPath fallback读取fresh SDK显式packageSettingsBases；Project目录由publicCONFIG_DIR_NAME推导，不硬编码cwd或.pi。Project继承Global包的update捕获选中resource实际source_scope；remove/install仍目标viewScope，backend严格匹配而非歧义fallback。3unit/3actualresource/17nativeconfig均exit0，logs以package-settings-bases/resource-restore-settings-bases/inherited-package-scope-final标记。production+fixtures+独立realchat脚本再次SOURCE FREEZE；下一freshbuild/4PTY/combinedverify仍待runner实际回执。

### Final automatic acceptance / 最终记录

- Latest正式build exit0/Cargo22.34s，184,693,160bytes，SHA `9c46d5c6d0b9a3b85657b2fbb5ecc2bcd8e2256a571347863f03db36115c74ad`；artifact `/tmp/grok-pi-deep-artifact-settings-scope-final-20261003.json`，log `/tmp/grok-pi-deep-build-settings-scope-final-20261003.log`。Source stamp为precommit完整`fe7515f528e7cb6ace46ed8dfa7edbe6b32fcc5e`+dirtyfrozen settings-scope，不冒称cleanfinalHEAD/coldbenchmark；最终commit接收同一shipping源码，doc-onlyreceipts不变binary。
- 同SHA前后不变的4nativePTY均exit0：runtime/retrycancel；真实官方localpackage install/remove、registryfactory/command与session/file/leaf/branch/model/thinking保全、local源未删、取消PIDdead；virtual/physicalmodel+nativeimage/classifiercost；RemoteTUIinput/resize/dispose。报告 `/tmp/grok-pi-deep-pty-settings-scope-final-20261003/report.json`，真实xterm/headless过程/captures保留；旧失败证据不覆盖。
- 最终combined `./verify.sh` actualexit0并输出Allverificationpassed，log `/tmp/grok-pi-deep-verify-settings-scope-final-20261003.log`，完整archive `/tmp/grok-pi-deep-verify-settings-scope-final-20261003/`。Pi/stock两profilecheck、production806/七禁止runtime空、adapter207lib+disposition1/ACK1/EOF2非ignored、bin98、额外2native单testfilters、source21/21/phase722、recursive1662/failures空、mock8/33lines/stderrempty通过；actualignored三resourcecases及lifecycle/nativeprojection/modelprojection另实际运行，不算成verify执行。
- Latest专项logs：`/tmp/grok-pi-deep-package-settings-bases-20261003.log`3unit、`/tmp/grok-pi-deep-resource-restore-settings-bases-20261003.log`3actual、`/tmp/grok-pi-deep-inherited-package-scope-final-20261003.log`17native。Default完整native testtarget10146未全跑，早期no-defaultfixture167编译错误仍保留。所有source/行为/receipts与exactmanifest同一个compileclosed本地commit；不push。
- DA-10仅SDK真实chat子项通过。真实provider的nativeUI、真人OAuth/browser consent、真实image、代表第三方及目标终端真人体验未完成/未获明确延期，整体goal仍进行中。

### DA-10：真人执行材料

- 自动交付 `main@108e415e` 后保持工作树和证据核对。补充 [真人验收步骤](20261003-pi-deep-adaptation-MANUAL.md)：用户选择环境、独立状态目录、本人 OAuth、production chat、官方 Codemode image/classifier 示例和已安装第三方组件的导航/resize/退出证据。
- 本阶段仅静态核对并准备材料；没有新增真实模型调用或账号授权，没有把准备步骤当成 DA-10 完成。环境选择/凭据路径/真人操作仍待用户提供或明确延期。
