/** Actual Pi child fixture: root-only provider registration must flow through the SDK. */
import { AssistantMessageEventStream } from "@earendil-works/pi-ai";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  pi.registerProvider("pi-child-fixture", {
    baseUrl: "http://127.0.0.1:9", apiKey: "isolated-fixture", api: "openai-completions",
    models: [{ id: "local", name: "Local child fixture", reasoning: false, input: ["text"], contextWindow: 32768, maxTokens: 4096,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 } }],
    streamSimple(model, context) {
      const stream = new AssistantMessageEventStream();
      const user = [...context.messages].reverse().find(message => message.role === "user");
      const text = typeof user?.content === "string" ? user.content : JSON.stringify(user?.content);
      const child = text?.includes("NATIVE_CHILD_REQUEST");
      const waitingChild = text?.includes("NATIVE_CHILD_WAIT_REQUEST");
      if (waitingChild) {
        // A real running SDK child, held until the isolated parent process is killed.
        // The registered provider never opens a network connection.
        return stream;
      }
      let userIndex = context.messages.length - 1;
      while (userIndex >= 0 && context.messages[userIndex].role !== "user") userIndex--;
      const spawned = context.messages.slice(userIndex + 1).some(message => message.role === "toolResult" && message.toolName === "spawn_subagent");
      const background = text?.includes("NATIVE_PARENT_WAIT_REQUEST") ?? false;
      const content = child ? [{ type: "text" as const, text: "NATIVE_CHILD_BODY" }]
        : spawned ? [{ type: "text" as const, text: "NATIVE_PARENT_DONE" }]
        : [{ type: "toolCall" as const, id: "fixture-child-spawn", name: "spawn_subagent", arguments: {
          prompt: background ? "NATIVE_CHILD_WAIT_REQUEST" : "NATIVE_CHILD_REQUEST",
          description: background ? "Native orphan fixture" : "Native child fixture", subagent_type: "general-purpose", background,
        } }];
      const stopReason = child || spawned ? "stop" as const : "toolUse" as const;
      const message = { role: "assistant" as const, content, api: model.api, provider: model.provider, model: model.id,
        stopReason, timestamp: Date.now(), usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0,
          cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } } };
      queueMicrotask(() => {
        const block = message.content[0];
        if (block.type === "text") {
          const partial = { ...message, content: [{ type: "text" as const, text: "" }] };
          stream.push({ type: "start", partial });
          stream.push({ type: "text_start", contentIndex: 0, partial });
          partial.content[0].text = block.text;
          stream.push({ type: "text_delta", contentIndex: 0, delta: block.text, partial });
          stream.push({ type: "text_end", contentIndex: 0, content: block.text, partial });
        } else stream.push({ type: "start", partial: message });
        stream.push({ type: "done", reason: stopReason, message }); stream.end();
      });
      return stream;
    },
  });
}
