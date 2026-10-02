import { existsSync, writeFileSync } from "node:fs";
import { AssistantMessageEventStream, type AssistantMessage } from "@earendil-works/pi-ai";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

/** Synthetic provider; every request still passes through the real Pi child kernel. */
export default function (pi: ExtensionAPI) {
  pi.on("session_before_switch", () => ({ cancel: existsSync(process.env.PI_WORKFLOW_SWITCH_GUARD!) }));
  pi.registerProvider("workflow-scope", {
    api: "openai-completions", apiKey: "fixture", baseUrl: "http://127.0.0.1:9",
    models: [{ id: "local", name: "Workflow scope fixture", reasoning: false, input: ["text"], contextWindow: 32768,
      maxTokens: 4096, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }],
    streamSimple(model, context, options) {
      const stream = new AssistantMessageEventStream();
      const message: AssistantMessage = { role: "assistant", content: [{ type: "text", text: "fixture checkpoint" }],
        api: model.api, provider: model.provider, model: model.id, stopReason: "stop", timestamp: Date.now(),
        usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } };
      if (JSON.stringify(context.messages).includes("scope-wait")) {
        writeFileSync(process.env.PI_WORKFLOW_CHILD_STARTED!, "started");
        options?.signal?.addEventListener("abort", () => {
          writeFileSync(process.env.PI_WORKFLOW_CHILD_DRAINED!, "drained");
          stream.push({ type: "error", reason: "aborted", error: { ...message, stopReason: "aborted", errorMessage: "fixture cancelled" } });
          stream.end();
        }, { once: true });
      } else queueMicrotask(() => { stream.push({ type: "start", partial: message }); stream.push({ type: "done", reason: "stop", message }); stream.end(); });
      return stream;
    },
  });
}
