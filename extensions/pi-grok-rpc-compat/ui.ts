import type { ExtensionUIContext, WorkingIndicatorOptions } from "@earendil-works/pi-coding-agent";

export const WORKING_STATUS_KEY = "__pi_grok_working__";
const UI_STATE = Symbol.for("pi-grok.rpc-ui-state");
const REMOTE_HOST_MARK = "__piGrokRemoteTuiHost";

/** Official Pi 1.0 RPC contract; mappings keep the same native Pager surfaces. */
export const UI_CAPABILITIES = [
  ["standard", "select / confirm / input / editor", "native QuestionView; timeout/cancel return to Pi"],
  ["standard", "notify / setStatus / setTitle", "native scrollback, toast, status and terminal title"],
  ["standard", "setWidget(string[])", "native widget; aboveEditor/belowEditor and clear by key"],
  ["standard", "setEditorText", "replace native PromptWidget text"],
  ["mapped", "setWorkingMessage / setWorkingVisible", "streaming status override; Pi running state and loader stay authoritative"],
  ["mapped", "setWorkingIndicator", "one static frame or hide/reset the status override; animated frames unsupported"],
  ["degraded", "pasteToEditor", "Pi RPC replaces editor text; no native paste/collapse handling"],
  ["experimental", "custom", "Remote TUI factory host; verified focus, keys, resize/layout, cancel and dispose; arbitrary third-party TUI APIs unsupported"],
  ["unsupported", "getEditorText / getEditorComponent", "RPC has no synchronous native editor readback"],
  ["unsupported", "setWidget(factory) / setHeader / setFooter", "persistent component factories require a terminal host"],
  ["unsupported", "onTerminalInput / setEditorComponent / addAutocompleteProvider", "native raw input/editor replacement is not exposed by RPC"],
  ["unsupported", "setHiddenThinkingLabel / setToolsExpanded / getToolsExpanded", "native block presentation is not exposed by RPC"],
  ["unsupported", "getAllThemes / getTheme / setTheme", "Pi RPC does not expose theme discovery/switching"],
] as const;

type UiState = {
  active: boolean;
  visible: boolean;
  message?: string;
  frame?: string;
  indicatorHidden: boolean;
  diagnostics: Map<string, string>;
  status: ExtensionUIContext["setStatus"];
  notify: ExtensionUIContext["notify"];
};
type BridgedUi = ExtensionUIContext & { [UI_STATE]?: UiState };

export function hasRpcUiBridge(ui: unknown): boolean {
  return !!ui && typeof ui === "object" && !!(ui as BridgedUi)[UI_STATE];
}

export function hasRemoteTuiHost(ui: unknown): boolean {
  return !!ui && typeof ui === "object"
    && (!!(ui as unknown as Record<string, unknown>)[REMOTE_HOST_MARK]
      || !!((ui as ExtensionUIContext).custom as unknown as Record<string, unknown>)?.[REMOTE_HOST_MARK]);
}

function publishWorking(state: UiState): void {
  const text = state.frame
    ? `${state.frame} ${state.message ?? "Working…"}`
    : state.message;
  state.status(WORKING_STATUS_KEY, state.active && state.visible && !state.indicatorHidden ? text : undefined);
}

/** Patch RPC no-ops only. The symbol survives Pi's shallow UI-context wrapper. */
export function installRpcUiBridge(value: unknown): void {
  if (!value || typeof value !== "object") return;
  const ui = value as BridgedUi;
  if (ui[UI_STATE] || typeof ui.setStatus !== "function" || typeof ui.notify !== "function") return;
  const state: UiState = {
    active: false, visible: true, indicatorHidden: false, diagnostics: new Map(),
    status: ui.setStatus.bind(ui), notify: ui.notify.bind(ui),
  };
  ui[UI_STATE] = state;
  const diagnose = (method: string, detail: string) => {
    if (state.diagnostics.has(method)) return;
    state.diagnostics.set(method, detail);
    state.notify(`Pi extension UI: ${method}: ${detail}. See /pi-ui-capabilities.`, "warning");
  };
  ui.setWorkingMessage = (message?: string) => { state.message = message; publishWorking(state); };
  ui.setWorkingVisible = (visible: boolean) => {
    state.visible = visible;
    if (!visible) diagnose("setWorkingVisible", "hides the mapped status override; the native Pi activity indicator remains visible");
    publishWorking(state);
  };
  ui.setWorkingIndicator = (options?: WorkingIndicatorOptions) => {
    if (options?.frames && options.frames.length > 1) {
      diagnose("setWorkingIndicator", "animated frames are unsupported; use a single static frame");
      return;
    }
    state.frame = options?.frames?.[0];
    state.indicatorHidden = options?.frames?.length === 0;
    publishWorking(state);
  };

  const originalWidget = ui.setWidget.bind(ui);
  ui.setWidget = ((key: string, content: unknown, options?: unknown) => {
    if (typeof content === "function") {
      diagnose("setWidget(factory)", "component factories are unsupported; use string[] or experimental custom()");
      return;
    }
    originalWidget(key, content as string[] | undefined, options as Parameters<ExtensionUIContext["setWidget"]>[2]);
  }) as ExtensionUIContext["setWidget"];

  // Keep official RPC return values, including empty editor readback. Do not
  // manufacture current editor text from the last extension write.
  const unsupported = [
    "onTerminalInput", "getEditorText", "getEditorComponent", "setEditorComponent",
    "addAutocompleteProvider", "setFooter", "setHeader", "setHiddenThinkingLabel",
    "setToolsExpanded", "getToolsExpanded", "getAllThemes", "getTheme", "setTheme",
  ] as const;
  for (const method of unsupported) {
    const original = ui[method];
    if (typeof original !== "function") continue;
    (ui as unknown as Record<string, unknown>)[method] = (...args: unknown[]) => {
      diagnose(method, "unsupported by Pi RPC; the official RPC fallback is preserved");
      return (original as (...args: unknown[]) => unknown).apply(ui, args);
    };
  }
  const paste = ui.pasteToEditor?.bind(ui);
  if (paste) ui.pasteToEditor = (text: string) => {
    diagnose("pasteToEditor", "Pi RPC replaces editor text without native paste handling");
    paste(text);
  };
  const custom = ui.custom?.bind(ui);
  if (custom && !hasRemoteTuiHost(ui)) ui.custom = ((...args: unknown[]) => {
    diagnose("custom", "Remote TUI host is inactive; official RPC returns undefined");
    return (custom as (...args: unknown[]) => unknown)(...args);
  }) as ExtensionUIContext["custom"];
}

export function setWorkingActive(value: unknown, active: boolean): void {
  const state = (value as BridgedUi | undefined)?.[UI_STATE];
  if (!state) return;
  state.active = active;
  publishWorking(state);
}

export function resetRpcUiBridge(value: unknown): void {
  const state = (value as BridgedUi | undefined)?.[UI_STATE];
  if (!state) return;
  state.active = false;
  state.visible = true;
  state.indicatorHidden = false;
  state.message = undefined;
  state.frame = undefined;
  state.diagnostics.clear();
  publishWorking(state);
}

export function uiCapabilityReport(ui: unknown, mode: string): string {
  const remote = hasRemoteTuiHost(ui) ? "active" : "inactive";
  const seen = (ui as BridgedUi | undefined)?.[UI_STATE]?.diagnostics;
  return [
    `Pi extension UI capabilities (Pi 1.0 RPC; extension mode=${mode}; Remote TUI=${remote})`,
    ...UI_CAPABILITIES.map(([level, method, detail]) => `${level}: ${method} — ${detail}`),
    "The experimental mode facade does not guarantee third-party terminal components.",
    ...(seen?.size ? ["Observed calls:", ...Array.from(seen, ([method, detail]) => `${method}: ${detail}`)] : []),
  ].join("\n");
}
