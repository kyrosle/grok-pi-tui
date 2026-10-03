# DA-10 真人验收步骤

对应 [SPEC](20261003-pi-deep-adaptation-SPEC.md) / [PLAN](20261003-pi-deep-adaptation-PLAN.md)。这是待执行的验收材料，不是完成证明。先记录用户选择的 provider/model、终端名称/版本与 binary SHA；目前用户尚未选择环境或明确延期。

## 独立验收环境与 OAuth

本机可用 iTerm、Apple Terminal、Warp；现有 Pi 默认 chat 模型是 `openai-codex/gpt-6.1-sol`。在所选终端运行下面的启动示例；模型可替换为用户选择的模型。真实凭据不复制到验收目录。

```sh
MANUAL_PI_ROOT="$(mktemp -d /tmp/grok-pi-manual.XXXXXX)"
mkdir -p "$MANUAL_PI_ROOT/project" "$MANUAL_PI_ROOT/pi" "$MANUAL_PI_ROOT/grok"
export PI_CODING_AGENT_DIR="$MANUAL_PI_ROOT/pi"
export GROK_HOME="$MANUAL_PI_ROOT/grok" GROK_PROJECT_DIR=.grok-pi
GROK_PI_BIN=/Users/kyros/WorkStation/grok-pi-tui/target/debug/grok-pi
shasum -a 256 "$GROK_PI_BIN"
cd "$MANUAL_PI_ROOT/project"
"$GROK_PI_BIN" --model openai-codex/gpt-6.1-sol --thinking minimal --no-extensions --no-skills --no-context-files --tools codemode -e builtin:codemode -e /Users/kyros/.pi/agent/npm/node_modules/pi-cache-graph/index.ts -- --no-prompt-templates --session-dir "$MANUAL_PI_ROOT/sessions"
```

1. `/login`：在原生 QuestionView 选择目标 provider/OAuth，先取消一次，确认输入焦点恢复。
2. 再次登录，由本人浏览器完成 consent/code。测试凭据只写入上述独立 Pi 目录；不记录登录 URL、code、token 或 accountID，不用 `/logout` 修改真实账号凭据。
3. 新临时会话发送“只回复 OK，不调用任何工具”，记录返回、模型、usage/cost。已有单次真实 SDK chat 不替代这次 production TUI 验收。

## 真实 image / classifier

先由用户配置有权限的 provider，使用官方可用目录选择模型。目前已有配置未提供可用 image/classifier 凭据路径；缺失时保留 pending，不判为零能力。让 agent 仅用 Codemode 输出 `models.getAvailableOfType("image")` / `models.getAvailableOfType("classifier")` 的 provider/id，再按用户选择执行各一次。下面的 IDs 是官方 Pi 1.0 文档示例，必须先确认可用，不能默认调用或自动换模型。

```js
const m = await models.getModelOfType("image", "openrouter", "google/gemini-2.5-flash-image");
const r = await models.generateImages(m, { input: [{ type: "text", text: "One plain blue square on white." }] });
text({ stopReason: r.stopReason, usage: r.usage });
if (r.stopReason === "stop") for (const b of r.output) if (b.type === "image") image(b);
```

```js
const m = await models.getModelOfType("classifier", "openrouter", "typesafe/jev-1.13");
const r = await models.classify(m, { state: { message: "Approved." }, questions: { approved: { type: "bool", instructions: "Is this approval?", criteria: { true: "Approval", false: "No approval" } } } });
text({ stopReason: r.stopReason, answers: r.answers, usage: r.usage });
```

确认原生图片显示/Open Image、classifier 结果与会话费用；不打印 base64、不批量生成、不自动重试。记录取消和错误反馈；不能把失败或没有可用模型写成成功。

## 真实第三方组件与目标终端

启动示例显式加载的是本机已有 `pi-cache-graph`，未安装新包。若该入口不存在，先选择另一已安装、可检查的扩展，不自动下载。

1. `/cache graph`：检查真实 custom UI 的 `1/2/3`、`v`、`r`、上下导航。
2. 窗口宽度 `120 → 64 → 120`：检查重排、焦点、边界，输入不能落入隐藏 composer。
3. `Esc` / `q` 关闭后输入普通草稿；检查输入 capture 释放、旧 frame 清理及再次打开。画面消失本身不证明 dispose；结合生命周期/输入恢复证据记录结果。
4. 检查 Ctrl+C 与正常退出后的终端恢复。截图/录屏只包含脱敏 UI，不共享含 `auth.json` 的整个临时目录，不导入或改写真实业务会话。

每个结果记录：commit、binary SHA、Pi/终端/扩展版本、provider/model、步骤、预期、实际、usage、证据路径和 pass/fail/pending。OAuth、真实图片/classifier、第三方或目标终端未完成项逐项保留；用户明确同意延期或真人验收完成之前，不关闭整体 goal。

调用形状来自实际 system Pi 1.0 的 `docs/codemode.md` / `docs/models.md`；`/cache graph` 与按键来自已安装 `pi-cache-graph/index.ts` / `src/index.ts`。本材料仅静态核对，没有执行上述真人步骤。
