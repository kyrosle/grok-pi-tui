import { expect, test } from "bun:test";
import {
  WORKING_STATUS_KEY, hasRpcUiBridge, installRpcUiBridge, resetRpcUiBridge,
  setWorkingActive, uiCapabilityReport,
} from "./ui.ts";

function fixture() {
  const statuses = new Map<string, string | undefined>();
  const notices: { text: string; type: string }[] = [];
  const widgets: unknown[][] = [];
  const editorWrites: string[] = [];
  const ui = {
    notify(text: string, type = "info") { notices.push({ text, type }); },
    setStatus(key: string, value?: string) { statuses.set(key, value); },
    setWidget(...args: unknown[]) { widgets.push(args); },
    setEditorText(text: string) { editorWrites.push(text); },
    pasteToEditor(text: string) { this.setEditorText(text); },
    getEditorText() { return ""; },
    getEditorComponent() { return undefined; },
    setEditorComponent() {}, setHeader() {}, setFooter() {}, setHiddenThinkingLabel() {},
    onTerminalInput() { return () => {}; },
    addAutocompleteProvider() {}, setToolsExpanded() {}, getToolsExpanded() { return false; },
    getAllThemes() { return []; }, getTheme() { return undefined; },
    setTheme() { return { success: false, error: "Theme switching not supported in RPC mode" }; },
    setWorkingMessage(_message?: string) {}, setWorkingVisible(_visible: boolean) {},
    setWorkingIndicator(_options?: { frames?: string[] }) {},
    async custom(..._args: unknown[]) { return undefined; },
  };
  return { ui, statuses, notices, widgets, editorWrites };
}

test("working override follows actual agent lifecycle and visibility without owning activity", () => {
  const { ui, statuses } = fixture();
  installRpcUiBridge(ui);
  ui.setWorkingMessage("Checking sources");
  expect(statuses.get(WORKING_STATUS_KEY)).toBeUndefined();
  setWorkingActive(ui, true);
  expect(statuses.get(WORKING_STATUS_KEY)).toBe("Checking sources");
  ui.setWorkingIndicator({ frames: ["●"] });
  expect(statuses.get(WORKING_STATUS_KEY)).toBe("● Checking sources");
  ui.setWorkingVisible(false);
  expect(statuses.get(WORKING_STATUS_KEY)).toBeUndefined();
  ui.setWorkingVisible(true);
  expect(statuses.get(WORKING_STATUS_KEY)).toBe("● Checking sources");
  ui.setWorkingIndicator({ frames: [] });
  expect(statuses.get(WORKING_STATUS_KEY)).toBeUndefined();
  ui.setWorkingIndicator();
  expect(statuses.get(WORKING_STATUS_KEY)).toBe("Checking sources");
  setWorkingActive(ui, false);
  expect(statuses.get(WORKING_STATUS_KEY)).toBeUndefined();
  setWorkingActive(ui, true);
  ui.setWorkingMessage();
  expect(statuses.get(WORKING_STATUS_KEY)).toBeUndefined();
});

test("animated indicator diagnoses once and keeps the working override", () => {
  const { ui, statuses, notices } = fixture();
  installRpcUiBridge(ui);
  setWorkingActive(ui, true);
  ui.setWorkingMessage("Inspecting");
  ui.setWorkingIndicator({ frames: ["a", "b"] });
  ui.setWorkingIndicator({ frames: ["c", "d"] });
  expect(statuses.get(WORKING_STATUS_KEY)).toBe("Inspecting");
  expect(notices).toHaveLength(1);
  expect(notices[0]?.text).toContain("animated frames are unsupported");
});

test("UI clones share lifecycle state and reset does not clear unrelated surfaces", () => {
  const { ui, statuses } = fixture();
  installRpcUiBridge(ui);
  const wrapped = { ...ui };
  expect(hasRpcUiBridge(wrapped)).toBe(true);
  installRpcUiBridge(wrapped);
  ui.setStatus("another-extension", "keep");
  wrapped.setWorkingMessage("Running");
  setWorkingActive(wrapped, true);
  expect(statuses.get(WORKING_STATUS_KEY)).toBe("Running");
  resetRpcUiBridge(wrapped);
  expect(statuses.get(WORKING_STATUS_KEY)).toBeUndefined();
  expect(statuses.get("another-extension")).toBe("keep");
});

test("standard widget placement and editor replacement stay source faithful; readback is not fabricated", () => {
  const { ui, widgets, editorWrites, notices } = fixture();
  installRpcUiBridge(ui);
  const options = { placement: "belowEditor" };
  ui.setWidget("footer", ["line"], options);
  ui.setWidget("footer", undefined, options);
  expect(widgets).toEqual([["footer", ["line"], options], ["footer", undefined, options]]);
  ui.setEditorText("first");
  ui.pasteToEditor("replacement");
  expect(editorWrites).toEqual(["first", "replacement"]);
  expect(ui.getEditorText()).toBe("");
  expect(ui.getEditorText()).toBe("");
  expect(ui.getEditorComponent()).toBeUndefined();
  expect(notices.filter((notice) => notice.text.includes("getEditorText:"))).toHaveLength(1);
});

test("unsupported factories/raw input retain RPC defaults and produce an inspectable support report", async () => {
  const { ui, notices, widgets } = fixture();
  installRpcUiBridge(ui);
  let factoryRuns = 0;
  const factory = () => { factoryRuns++; return {}; };
  ui.setWidget("factory", factory);
  ui.setHeader();
  ui.setFooter();
  ui.setEditorComponent();
  const unsubscribe = ui.onTerminalInput();
  unsubscribe();
  expect(await ui.custom(factory)).toBeUndefined();
  expect(factoryRuns).toBe(0);
  expect(widgets).toEqual([]);
  expect(ui.getToolsExpanded()).toBe(false);
  expect(ui.getAllThemes()).toEqual([]);
  expect(ui.setTheme().success).toBe(false);
  const report = uiCapabilityReport(ui, "rpc");
  expect(report).toContain("standard: select / confirm / input / editor");
  expect(report).toContain("Remote TUI=inactive");
  expect(report).toContain("Observed calls:");
  expect(report).toContain("getEditorText / getEditorComponent");
  expect(notices.every((notice) => notice.type === "warning")).toBe(true);
});
