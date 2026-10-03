import { expect, test } from "bun:test";
import registerCompatibility, { installRunnerHooksOn } from "./index.ts";
import { hasRpcUiBridge } from "./ui.ts";

function withFlags(body: () => void) {
  const keys = ["PI_GROK", "PI_GROK_EXTENSION_TUI_COMPAT", "PI_GROK_REMOTE_TUI"];
  const saved = keys.map((key) => process.env[key]);
  const host = globalThis as typeof globalThis & { __piGrokEnsureRemoteTuiHost?: (ui: unknown) => void };
  const originalHost = host.__piGrokEnsureRemoteTuiHost;
  try {
    process.env.PI_GROK = "1";
    process.env.PI_GROK_EXTENSION_TUI_COMPAT = "1";
    process.env.PI_GROK_REMOTE_TUI = "1";
    body();
  } finally {
    keys.forEach((key, i) => {
      if (saved[i] === undefined) delete process.env[key];
      else process.env[key] = saved[i];
    });
    host.__piGrokEnsureRemoteTuiHost = originalHost;
  }
}

test("mode facade requires an installed custom host; native/print and disabled shim retain actual mode", () => {
  withFlags(() => {
    const modes: string[] = [];
    const prototype = {
      setUIContext(_ui: unknown, mode = "print") { modes.push(mode); },
      getRegisteredCommands() { return []; },
    };
    installRunnerHooksOn(prototype);
    installRunnerHooksOn(prototype);
    const ui = { notify() {}, setStatus() {}, setWidget() {}, async custom() {} };
    prototype.setUIContext(ui, "rpc");
    expect(modes.at(-1)).toBe("rpc");
    expect(hasRpcUiBridge(ui)).toBe(true);
    const host = globalThis as typeof globalThis & { __piGrokEnsureRemoteTuiHost?: (ui: unknown) => void };
    host.__piGrokEnsureRemoteTuiHost = (value) => {
      (value as Record<string, unknown>).__piGrokRemoteTuiHost = true;
    };
    prototype.setUIContext(ui, "rpc");
    expect(modes.at(-1)).toBe("tui");
    process.env.PI_GROK_REMOTE_TUI = "0";
    prototype.setUIContext(ui, "rpc");
    expect(modes.at(-1)).toBe("rpc");
    prototype.setUIContext(ui, "print");
    expect(modes.at(-1)).toBe("print");
    prototype.setUIContext(ui, "tui");
    expect(modes.at(-1)).toBe("tui");
    delete process.env.PI_GROK;
    const native = { notify() {}, setStatus() {}, setWidget() {}, async custom() {} };
    prototype.setUIContext(native, "rpc");
    expect(hasRpcUiBridge(native)).toBe(false);
    expect(modes.at(-1)).toBe("rpc");
  });
});

test("loading compatibility in native Pi leaves registrations and UI untouched", async () => {
  const previous = process.env.PI_GROK;
  delete process.env.PI_GROK;
  const calls: string[] = [];
  try {
    await registerCompatibility({ on() { calls.push("event"); }, registerCommand() { calls.push("command"); } } as never);
    expect(calls).toEqual([]);
  } finally {
    if (previous === undefined) delete process.env.PI_GROK;
    else process.env.PI_GROK = previous;
  }
});

test("changed private runner contract fails explicitly before installing partial hooks", () => {
  const prototype = { setUIContext() {} };
  const original = prototype.setUIContext;
  expect(() => installRunnerHooksOn(prototype as never)).toThrow("contract changed");
  expect(prototype.setUIContext).toBe(original);
});
