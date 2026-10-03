#!/usr/bin/env node
/** One explicitly requested SDK chat, never a production TUI/OAuth proof. */
import { readFileSync, existsSync, realpathSync } from "node:fs";
import { dirname, join, delimiter } from "node:path";
import { pathToFileURL } from "node:url";
import { createHash } from "node:crypto";

const PROVIDER = "openai-codex";
const MODEL = "gpt-6.1-sol";
const DEADLINE_MS = 20_000;
const TEXT_LIMIT_BYTES = 64;
const execute = process.argv.includes("--execute-once");
const report = {
  proof: "official Pi ModelRuntime real-provider SDK only",
  provider: PROVIDER, model: MODEL, executeRequested: execute, requests: 0,
  serverOutputTokenCapSupported: false, suggestedMaxTokens: 32,
  deadlineMs: DEADLINE_MS, textLimitBytes: TEXT_LIMIT_BYTES,
  refreshAllowed: false, loginAllowed: false, persistenceAllowed: false,
};
class GuardStop extends Error { constructor(code) { super(code); this.code = code; } }
const stop = (code) => { throw new GuardStop(code); };
const containsCommand = (value) => typeof value === "string" ? value.trimStart().startsWith("!")
  : Array.isArray(value) ? value.some(containsCommand)
    : value && typeof value === "object" ? Object.values(value).some(containsCommand) : false;
const digest = (path) => existsSync(path) ? createHash("sha256").update(readFileSync(path)).digest("hex") : null;
const numbers = (source, keys) => Object.fromEntries(keys.flatMap((key) =>
  typeof source?.[key] === "number" && Number.isFinite(source[key]) ? [[key, source[key]]] : []));

let files = [], before = [], timer, abortReason;
try {
  const requested = process.env.PI_BIN ?? "pi";
  const found = requested.includes("/") ? requested
    : (process.env.PATH ?? "").split(delimiter).map((path) => join(path, requested)).find(existsSync);
  if (!found) stop("pi_host_missing");
  const entryDir = dirname(realpathSync(found));
  const dist = entryDir.endsWith("/bundle") ? dirname(entryDir) : entryDir;
  const { ModelRuntime, getAgentDir, VERSION } = await import(pathToFileURL(join(dist, "index.js")).href);
  const { InMemoryModelsStore } = await import(pathToFileURL(join(dist, "../../pi-ai/dist/index.js")).href);
  report.piVersion = VERSION;
  process.env.PI_OFFLINE = "1";
  process.env.PI_TELEMETRY = "0";
  const agentDir = getAgentDir();
  files = ["auth.json", "models.json", "models-store.json", "settings.json"].map((name) => join(agentDir, name));
  files.push(join(process.cwd(), ".pi/settings.json"));
  before = files.map(digest);
  const stored = existsSync(files[0]) ? JSON.parse(readFileSync(files[0], "utf8"))[PROVIDER] : undefined;
  const configuration = existsSync(files[1]) ? JSON.parse(readFileSync(files[1], "utf8")) : {};
  if (containsCommand(stored) || containsCommand(configuration.providers?.[PROVIDER])) stop("credential_command_forbidden");
  report.credentialExists = !!stored;
  if (stored?.type !== "oauth" || !stored.access || !stored.refresh) stop("configured_oauth_missing");
  const assertFresh = () => {
    if (!Number.isFinite(stored.expires) || stored.expires <= Date.now() + 5 * 60_000 + DEADLINE_MS)
      stop("fresh_oauth_requires_human_login_or_refresh");
  };
  assertFresh();
  report.freshWithoutRefresh = true;
  // Never call a modify callback: it could perform a refresh before a rejected
  // write. Reads return only the selected credential from process memory.
  const credentials = {
    async read(id) { if (id !== PROVIDER) return undefined; assertFresh(); return structuredClone(stored); },
    async list() { return [{ providerId: PROVIDER, type: "oauth" }]; },
    async modify() { stop("oauth_refresh_and_credential_mutation_forbidden"); },
    async delete() { stop("logout_forbidden"); },
  };
  const runtime = await ModelRuntime.create({ credentials, modelsPath: files[1],
    modelsStore: new InMemoryModelsStore(), refreshOnCreate: false, allowModelNetwork: false });
  const model = runtime.getModel(PROVIDER, MODEL);
  if (!model) stop("requested_model_not_in_local_sdk_catalog");
  const levels = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
  const reasoning = levels.find((level) => model.thinkingLevelMap?.[level] !== null) ?? "off";
  report.thinking = reasoning;
  if (!execute) { report.status = "preflight_ready_no_dispatch"; }
  else {
    const controller = new AbortController();
    timer = setTimeout(() => { abortReason = "total_deadline"; controller.abort(); }, DEADLINE_MS);
    let text = "", status;
    const stream = runtime.streamSimple(model, { messages: [{ role: "user", content: "Reply only with OK.", timestamp: Date.now() }] }, {
      maxTokens: 32, reasoning, transport: "sse", cacheRetention: "none", maxRetries: 0,
      timeoutMs: DEADLINE_MS, signal: controller.signal,
      onPayload(payload) {
        if (payload.tools?.length || payload.store !== false || payload.model !== MODEL) stop("unexpected_context_or_tools");
        report.sentThinking = payload.reasoning?.effort ?? "none";
        report.serverOutputTokenCapSupported = typeof payload.max_output_tokens === "number";
      },
      async fetch(input, options) {
        assertFresh();
        if (report.requests !== 0) stop("second_provider_request_forbidden");
        report.requests++;
        return globalThis.fetch(input, options);
      },
      onResponse(response) { status = response.status; },
    });
    for await (const event of stream) {
      if (event.type === "text_delta") {
        text += event.delta;
        if (Buffer.byteLength(text, "utf8") > TEXT_LIMIT_BYTES) { abortReason = "text_limit"; controller.abort(); }
      }
      if (event.type.startsWith("toolcall")) { abortReason = "unexpected_tool_call"; controller.abort(); }
    }
    const result = await stream.result();
    report.httpStatus = status;
    report.stopReason = result.stopReason;
    report.abortBoundary = abortReason ?? null;
    report.usage = numbers(result.usage, ["input", "output", "cacheRead", "cacheWrite", "totalTokens", "reasoning"]);
    report.sdkCost = numbers(result.usage?.cost, ["input", "output", "cacheRead", "cacheWrite", "total"]);
    const output = result.content.filter((block) => block.type === "text").map((block) => block.text).join("").trim();
    report.result = output === "OK" ? "OK" : null;
    report.status = output === "OK" && result.stopReason === "stop" && !abortReason ? "pass" : "failed";
    if (report.status !== "pass") report.errorClass = abortReason ?? (status === 401 || status === 403 ? "human_auth_or_access_required"
      : status === 429 ? "quota_or_rate_limit" : "provider_error_or_non_OK_result");
  }
} catch (error) {
  report.status = "blocked";
  report.errorClass = error instanceof GuardStop ? error.code : "sdk_or_local_configuration_error";
} finally {
  if (timer) clearTimeout(timer);
  report.persistentFilesUnchanged = files.length > 0 && before.length === files.length
    ? files.every((path, index) => digest(path) === before[index]) : null;
  if (report.persistentFilesUnchanged === false) { report.status = "failed"; report.errorClass = "persistent_files_changed"; }
  console.log(JSON.stringify(report));
  process.exitCode = report.status === "pass" || report.status === "preflight_ready_no_dispatch" ? 0 : 2;
}
