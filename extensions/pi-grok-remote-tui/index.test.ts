import { afterAll, beforeAll, expect, mock, test } from "bun:test";
import { appendFileSync, existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { metaPath } from "./transport.ts";

type SettingItem = {
  id: string;
  label: string;
  description?: string;
  currentValue: string;
  values?: string[];
};

/** Minimal SettingsList stand-in: space/enter cycles, esc cancels. */
class MockSettingsList {
  private items: SettingItem[];
  private index = 0;
  private onChange: (id: string, newValue: string) => void;
  private onCancel: () => void;

  constructor(
    items: SettingItem[],
    _maxVisible: number,
    _theme: unknown,
    onChange: (id: string, newValue: string) => void,
    onCancel: () => void,
  ) {
    this.items = items;
    this.onChange = onChange;
    this.onCancel = onCancel;
  }

  invalidate() {}
  render() {
    return this.items.map((item) => `${item.label}=${item.currentValue}`);
  }

  handleInput(data: string) {
    if (data === "\x1b[A") {
      this.index = this.index === 0 ? this.items.length - 1 : this.index - 1;
      return;
    }
    if (data === "\x1b[B") {
      this.index = this.index === this.items.length - 1 ? 0 : this.index + 1;
      return;
    }
    if (data === " " || data === "\r") {
      const item = this.items[this.index]!;
      const values = item.values ?? ["on", "off"];
      const next = values[(values.indexOf(item.currentValue) + 1) % values.length]!;
      item.currentValue = next;
      this.onChange(item.id, next);
      return;
    }
    if (data === "\x1b") {
      this.onCancel();
    }
  }
}

mock.module("@earendil-works/pi-tui", () => ({
  CURSOR_MARKER: "\x1b_pi:c\x07",
  isKeyRelease: (data: string) => /:3(?:u|~|[ABCDHF])/.test(data),
  KeybindingsManager: class {
    matches() {
      return false;
    }
  },
  TUI_KEYBINDINGS: {},
  setKeybindings: () => {},
  SettingsList: MockSettingsList,
}));

const {
  default: registerRemoteTui,
  createDemoSelector,
  applyDemoCapabilities,
} = await import("./index.ts");
const { dispatchComponentInput, installCustomPatch, projectComponentLine } = await import("./host.ts");
const testDirectory = mkdtempSync(join(tmpdir(), "remote-tui-host-tests-"));
const previousMetaPath = process.env.PI_GROK_REMOTE_TUI_META;
beforeAll(() => { process.env.PI_GROK_REMOTE_TUI_META = join(testDirectory, "active.json"); });
afterAll(async () => {
  // Close the final test's host and watcher before releasing the isolated path.
  const ui = { custom: async (..._args: any[]): Promise<any> => undefined, setWidget() {} };
  installCustomPatch(ui);
  await ui.custom((_tui: unknown, _theme: unknown, _kb: unknown, done: (result: unknown) => void) => {
    done(undefined);
    return { render: () => [], invalidate() {} };
  });
  if (previousMetaPath === undefined) delete process.env.PI_GROK_REMOTE_TUI_META;
  else process.env.PI_GROK_REMOTE_TUI_META = previousMetaPath;
  rmSync(testDirectory, { recursive: true });
});

test("focused custom child gets letters and Escape without global shortcut interception", async () => {
  const inputs: string[] = [];
  let rootFocus = false;
  let childFocus = false;
  const globals = globalThis as typeof globalThis & { __piGrokShortcutIntercept?: (data: string) => boolean };
  const previous = globals.__piGrokShortcutIntercept;
  let intercepted = 0;
  globals.__piGrokShortcutIntercept = () => { intercepted++; return true; };
  const ui = { custom: async (..._args: any[]): Promise<any> => undefined, setWidget() {} };
  installCustomPatch(ui);
  let finish!: (result: string) => void;
  let showOverlay!: (component: any) => { hide(): void };
  try {
    const result = ui.custom((tui: any, _theme: unknown, _kb: unknown, done: typeof finish) => {
      finish = done;
      showOverlay = tui.showOverlay;
      tui.setFocus({
        invalidate() {}, render: () => [],
        get focused() { return childFocus; },
        set focused(value: boolean) { childFocus = value; },
        handleInput(data: string) { inputs.push(data); if (data === "s") done("saved"); },
      });
      return {
        invalidate() {}, render: () => ["frame"],
        get focused() { return rootFocus; },
        set focused(value: boolean) { rootFocus = value; },
        handleInput() { throw new Error("root must not steal child focus"); },
      };
    });
    await new Promise((resolve) => setImmediate(resolve));
    expect(childFocus).toBe(true);
    expect(rootFocus).toBe(false);
    const overlay = showOverlay({ invalidate() {}, render: () => ["nested"] });
    expect(childFocus).toBe(false);
    overlay.hide();
    expect(childFocus).toBe(true);
    const { id, keysPath } = JSON.parse(readFileSync(metaPath(), "utf8"));
    appendFileSync(keysPath, ["a", "\x1b", "s"].map((data) => JSON.stringify({ id, op: "input", data }) + "\n").join(""));
    expect(await result).toBe("saved");
    expect(inputs).toEqual(["a", "\x1b", "s"]);
    expect(intercepted).toBe(0);
    expect(childFocus).toBe(false);
  } finally {
    finish?.("cancelled");
    globals.__piGrokShortcutIntercept = previous;
  }
});

test("async custom factories claim input before rendering and publish matching close", async () => {
  const lifecycle: { op: string; id: string }[] = [];
  const ui = {
    custom: async (..._args: any[]): Promise<any> => undefined,
    setWidget: (key: string, lines?: string[]) => {
      if (key === "__pi_grok_remote_tui_session__" && lines?.[0]) {
        lifecycle.push(JSON.parse(lines[0]));
      }
    },
  };
  installCustomPatch(ui);
  let finish!: (result: string) => void;
  const result = ui.custom((_tui: unknown, _theme: unknown, _kb: unknown, done: typeof finish) => {
    finish = done;
    return { render: () => ["frame"], invalidate() {} };
  });
  expect(lifecycle).toHaveLength(1);
  expect(lifecycle[0]?.op).toBe("open");
  await new Promise((resolve) => setImmediate(resolve));
  finish("saved");
  expect(await result).toBe("saved");
  expect(lifecycle.at(-1)).toEqual({ op: "close", id: lifecycle[0]!.id });
});

test("remote host filters key release unless component opts in", () => {
  const regularInputs: string[] = [];
  const releaseAwareInputs: string[] = [];
  const press = "\x1b[B";
  const release = "\x1b[1;1:3B";

  dispatchComponentInput(
    {
      invalidate() {},
      render: () => [],
      handleInput: (data) => regularInputs.push(data),
    },
    press,
  );
  dispatchComponentInput(
    {
      invalidate() {},
      render: () => [],
      handleInput: (data) => regularInputs.push(data),
    },
    release,
  );
  dispatchComponentInput(
    {
      wantsKeyRelease: true,
      invalidate() {},
      render: () => [],
      handleInput: (data) => releaseAwareInputs.push(data),
    },
    release,
  );

  expect(regularInputs).toEqual([press]);
  expect(releaseAwareInputs).toEqual([release]);
});

test("Pi session shutdown closes a custom host and disposes its component exactly once", async () => {
  const previous = process.env.PI_GROK_REMOTE_TUI;
  process.env.PI_GROK_REMOTE_TUI = "1";
  const handlers = new Map<string, (...args: any[]) => void>();
  const pi = {
    on: (event: string, handler: (...args: any[]) => void) => { handlers.set(event, handler); },
    registerCommand() {},
  };
  let disposed = 0;
  const ui = { custom: async (..._args: any[]): Promise<any> => undefined, setWidget() {} };
  try {
    registerRemoteTui(pi as never);
    handlers.get("session_start")?.({}, { ui });
    const result = ui.custom(() => ({ render: () => ["frame"], invalidate() {}, dispose() { disposed++; } }));
    await new Promise((resolve) => setImmediate(resolve));
    handlers.get("session_shutdown")?.();
    expect(await result).toBeUndefined();
    handlers.get("session_shutdown")?.();
    expect(disposed).toBe(1);
    expect(existsSync(metaPath())).toBe(false);
  } finally {
    handlers.get("session_shutdown")?.();
    if (previous === undefined) delete process.env.PI_GROK_REMOTE_TUI;
    else process.env.PI_GROK_REMOTE_TUI = previous;
  }
});

test("Pi shallow UI wrapper retains the host marker and custom input ownership", async () => {
  const ui = { custom: async (..._args: any[]): Promise<any> => undefined, setWidget() {} };
  installCustomPatch(ui);
  const original = ui.custom;
  const wrapped = { ...ui, custom: (...args: unknown[]) => original(...args) };
  const wrappedCustom = wrapped.custom;
  installCustomPatch(wrapped as never);
  expect(wrapped.custom).toBe(wrappedCustom);
  expect((wrapped as Record<string, unknown>).__piGrokRemoteTuiHost).toBe(true);
  expect(await wrapped.custom((_tui: unknown, _theme: unknown, _keys: unknown, done: (result: string) => void) => {
    done("closed");
    return { render: () => [], invalidate() {} };
  })).toBe("closed");
});

test("id-scoped resize updates dimensions, invalidates focused content and recomputes overlay layout", async () => {
  const widths: number[] = [];
  const layouts: Record<string, unknown>[] = [];
  let terminal: { columns: number; rows: number } | undefined;
  let invalidated = 0;
  let childInvalidated = 0;
  let disposed = 0;
  const ui = {
    custom: async (..._args: any[]): Promise<any> => undefined,
    setWidget(key: string, lines?: string[]) {
      if (key === "__pi_grok_remote_tui_layout__" && lines?.[0]) layouts.push(JSON.parse(lines[0]));
    },
  };
  installCustomPatch(ui);
  const result = ui.custom((tui: any, _theme: unknown, _keys: unknown, done: (result: string) => void) => {
    terminal = tui.terminal;
    tui.setFocus({render:()=>[],invalidate(){childInvalidated++;},handleInput(data: string){if(data==="x") done("picked");}});
    return {
      render(width: number) { widths.push(width); return ["frame"]; },
      invalidate() { invalidated++; }, dispose() { disposed++; },
      handleInput() { throw new Error("resize stole child focus"); },
    };
  }, { overlay: true, overlayOptions: { width: "50%", maxHeight: "60%" } });
  await new Promise((resolve) => setImmediate(resolve));
  const { id, keysPath } = JSON.parse(readFileSync(metaPath(), "utf8"));
  appendFileSync(keysPath, [
    {id:"old",op:"resize",columns:200,rows:80},
    {id,op:"resize",columns:120,rows:40},
  ].map(value => JSON.stringify(value)+"\n").join(""));
  await new Promise((resolve) => setTimeout(resolve, 75));
  expect(terminal).toEqual({columns:120,rows:40});
  expect(widths.at(-1)).toBe(60);
  expect(layouts.at(-1)).toMatchObject({overlay:true,width:60,maxHeight:"60%"});
  expect(invalidated).toBe(1);
  expect(childInvalidated).toBe(1);
  appendFileSync(keysPath, JSON.stringify({id,op:"input",data:"x"})+"\n");
  expect(await result).toBe("picked");
  expect(disposed).toBe(1);
});

test("a component failing resize invalidation releases focus and disposes the host", async () => {
  let disposed = 0;
  const ui = {custom:async (..._args: any[]): Promise<any> => undefined,setWidget(){}};
  installCustomPatch(ui);
  const result = ui.custom(() => ({render:()=>["frame"],invalidate(){throw new Error("resize failed");},dispose(){disposed++;}}));
  const rejected = result.catch((error: Error) => error.message);
  await new Promise((resolve) => setImmediate(resolve));
  const {id,keysPath} = JSON.parse(readFileSync(metaPath(), "utf8"));
  appendFileSync(keysPath, JSON.stringify({id,op:"resize",columns:100,rows:30})+"\n");
  expect(await rejected).toBe("resize failed");
  expect(disposed).toBe(1);
  expect(existsSync(metaPath())).toBe(false);
});

test("custom host is NOT installed under native Pi (no PI_GROK)", async () => {
  const previousGrok = process.env.PI_GROK;
  const previousFlag = process.env.PI_GROK_REMOTE_TUI;
  delete process.env.PI_GROK;
  delete process.env.PI_GROK_REMOTE_TUI;

  let sessionStart:
    | ((event: unknown, ctx: { ui: { custom: (...args: unknown[]) => unknown; setWidget: () => void } }) => void)
    | undefined;
  const pi = {
    on: (_event: string, handler: typeof sessionStart) => {
      sessionStart = handler;
    },
    registerCommand: () => {},
  };
  const originalCustom = async () => "native";
  const ui = {
    custom: originalCustom,
    setWidget: () => {},
  };

  try {
    registerRemoteTui(pi as never);
    sessionStart?.({}, { ui });
    expect(ui.custom).toBe(originalCustom);
    expect(await ui.custom()).toBe("native");
  } finally {
    if (previousGrok === undefined) delete process.env.PI_GROK;
    else process.env.PI_GROK = previousGrok;
    if (previousFlag === undefined) delete process.env.PI_GROK_REMOTE_TUI;
    else process.env.PI_GROK_REMOTE_TUI = previousFlag;
  }
});

test("custom host exposes terminal dimensions to component factories", async () => {
  const previous = process.env.PI_GROK_REMOTE_TUI;
  process.env.PI_GROK_REMOTE_TUI = "1";

  let sessionStart:
    | ((event: unknown, ctx: { ui: { custom: (...args: unknown[]) => unknown; setWidget: () => void } }) => void)
    | undefined;
  const pi = {
    on: (_event: string, handler: typeof sessionStart) => {
      sessionStart = handler;
    },
    registerCommand: () => {},
  };
  const ui = {
    custom: async () => undefined,
    setWidget: () => {},
  };

  try {
    registerRemoteTui(pi as never);
    sessionStart?.({}, { ui });

    const result = await ui.custom((tui: { terminal: { columns: number; rows: number } }, _theme, _kb, done) => {
      expect(tui.terminal.columns).toBeGreaterThan(0);
      expect(tui.terminal.rows).toBeGreaterThan(0);
      done("ok");
      return { invalidate() {}, render: () => [], handleInput() {} };
    });

    expect(result).toBe("ok");
  } finally {
    if (previous === undefined) delete process.env.PI_GROK_REMOTE_TUI;
    else process.env.PI_GROK_REMOTE_TUI = previous;
  }
});

test("custom host mirrors Pi overlay width and position metadata", async () => {
  const previous = process.env.PI_GROK_REMOTE_TUI;
  const previousWidth = process.env.PI_GROK_REMOTE_TUI_WIDTH;
  process.env.PI_GROK_REMOTE_TUI = "1";
  process.env.PI_GROK_REMOTE_TUI_WIDTH = "80";

  let sessionStart:
    | ((event: unknown, ctx: { ui: { custom: (...args: unknown[]) => unknown; setWidget: (key: string, lines?: string[]) => void } }) => void)
    | undefined;
  const pi = {
    on: (_event: string, handler: typeof sessionStart) => {
      sessionStart = handler;
    },
    registerCommand: () => {},
  };
  let layout: Record<string, unknown> | undefined;
  const ui = {
    custom: async () => undefined,
    setWidget: (key: string, lines?: string[]) => {
      if (key === "__pi_grok_remote_tui_layout__" && lines?.[0]) {
        layout = JSON.parse(lines[0]) as Record<string, unknown>;
      }
    },
  };

  try {
    registerRemoteTui(pi as never);
    sessionStart?.({}, { ui });
    void ui.custom(
      (_tui, _theme, _kb, _done) => ({ invalidate() {}, render: () => ["frame"], handleInput() {} }),
      {
        overlay: true,
        overlayOptions: {
          width: "50%",
          maxHeight: "60%",
          anchor: "top-left",
          offsetX: 2,
          offsetY: 3,
        },
      },
    );
    await new Promise((resolve) => setImmediate(resolve));
    expect(layout).toMatchObject({
      overlay: true,
      width: 40,
      maxHeight: "60%",
      anchor: "top-left",
      offsetX: 2,
      offsetY: 3,
    });
  } finally {
    if (previous === undefined) delete process.env.PI_GROK_REMOTE_TUI;
    else process.env.PI_GROK_REMOTE_TUI = previous;
    if (previousWidth === undefined) delete process.env.PI_GROK_REMOTE_TUI_WIDTH;
    else process.env.PI_GROK_REMOTE_TUI_WIDTH = previousWidth;
  }
});

test("custom host keeps Pi custom inline by default", async () => {
  const previous = process.env.PI_GROK_REMOTE_TUI;
  const previousWidth = process.env.PI_GROK_REMOTE_TUI_WIDTH;
  process.env.PI_GROK_REMOTE_TUI = "1";
  process.env.PI_GROK_REMOTE_TUI_WIDTH = "120";

  let sessionStart:
    | ((event: unknown, ctx: { ui: { custom: (...args: unknown[]) => unknown; setWidget: (key: string, lines?: string[]) => void } }) => void)
    | undefined;
  const pi = {
    on: (_event: string, handler: typeof sessionStart) => {
      sessionStart = handler;
    },
    registerCommand: () => {},
  };
  let layout: Record<string, unknown> | undefined;
  const ui = {
    custom: async () => undefined,
    setWidget: (key: string, lines?: string[]) => {
      if (key === "__pi_grok_remote_tui_layout__" && lines?.[0]) {
        layout = JSON.parse(lines[0]) as Record<string, unknown>;
      }
    },
  };

  try {
    registerRemoteTui(pi as never);
    sessionStart?.({}, { ui });
    void ui.custom((_tui, _theme, _kb, _done) => ({
      width: 37,
      invalidate() {},
      render: () => ["frame"],
      handleInput() {},
    }));
    await new Promise((resolve) => setImmediate(resolve));
    expect(layout).toMatchObject({ overlay: false, width: 120 });
  } finally {
    if (previous === undefined) delete process.env.PI_GROK_REMOTE_TUI;
    else process.env.PI_GROK_REMOTE_TUI = previous;
    if (previousWidth === undefined) delete process.env.PI_GROK_REMOTE_TUI_WIDTH;
    else process.env.PI_GROK_REMOTE_TUI_WIDTH = previousWidth;
  }
});

test("new and legacy cursor cells keep one native highlight without Pi control markers", () => {
  for (const [frame, expected] of [
    ["\x1b_pi:c\x07\x1b_pi:fc\x07 \x1b_pi:/fc\x07", "\x1b[7m \x1b[27m"],
    ["a\x1b_pi:c\x07\x1b_pi:fc\x07字\x1b_pi:/fc\x07", "a\x1b[7m字\x1b[27m"],
    ["\x1b_pi:fc\x07x\x1b_pi:/fc\x07after", "\x1b[7mx\x1b[27mafter"],
    ["before\x1b_pi:c\x07\x1b[7m \x1b[27mafter", "before\x1b[7m \x1b[27mafter"],
  ]) {
    expect(projectComponentLine(frame!)).toBe(expected!);
  }
});

test("custom host removes Pi hardware cursor markers from projected frames", async () => {
  const previous = process.env.PI_GROK_REMOTE_TUI;
  process.env.PI_GROK_REMOTE_TUI = "1";

  let sessionStart:
    | ((event: unknown, ctx: { ui: { custom: (...args: unknown[]) => unknown; setWidget: (key: string, lines?: string[]) => void } }) => void)
    | undefined;
  const pi = {
    on: (_event: string, handler: typeof sessionStart) => {
      sessionStart = handler;
    },
    registerCommand: () => {},
  };
  let frame: string[] | undefined;
  const ui = {
    custom: async () => undefined,
    setWidget: (_key: string, lines?: string[]) => {
      frame = lines;
    },
  };

  try {
    registerRemoteTui(pi as never);
    sessionStart?.({}, { ui });
    void ui.custom((_tui, _theme, _kb, _done) => ({
      invalidate() {},
      render: () => ["before\x1b_pi:c\x07\x1b_pi:fc\x07 \x1b_pi:/fc\x07after"],
      handleInput() {},
    }));
    await new Promise((resolve) => setImmediate(resolve));
    await new Promise((resolve) => setImmediate(resolve));

    expect(frame).toBeDefined();
    expect(frame?.join("\n")).not.toContain("pi:c");
    expect(frame?.join("\n")).not.toContain("pi:fc");
    expect(frame).toEqual(["before\x1b[7m \x1b[27mafter"]);
  } finally {
    if (previous === undefined) delete process.env.PI_GROK_REMOTE_TUI;
    else process.env.PI_GROK_REMOTE_TUI = previous;
  }
});

test("demo SettingsList toggles and applies selected surfaces", () => {
  const applied: string[][] = [];
  let closed: string | undefined;
  const theme = {
    fg: (_c: string, text: string) => text,
    bold: (text: string) => text,
  };
  const demo = createDemoSelector(
    { requestRender: () => {} },
    theme,
    (result) => {
      closed = result;
    },
    (keys) => {
      applied.push([...keys]);
    },
  );

  // Space on first item (header) → on
  demo.handleInput?.(" ");
  // Move down and enable footer
  demo.handleInput?.("\x1b[B");
  demo.handleInput?.(" ");
  expect(applied).toEqual([["header"], ["header", "footer"]]);

  const rendered = demo.render(80).join("\n");
  expect(rendered).toContain("Remote TUI capability lab");
  expect(rendered).toContain("Header widget=on");
  expect(rendered).toContain("Footer widget=on");

  // Esc closes with selected keys
  demo.handleInput?.("\x1b");
  expect(closed).toBe("header,footer");
});

test("applyDemoCapabilities projects header/footer/status/title/editor", () => {
  const widgets = new Map<string, { lines?: string[]; placement?: string }>();
  let status: { key?: string; text?: string } = {};
  let title: string | undefined;
  let editorText: string | undefined;

  applyDemoCapabilities(
    {
      setWidget: (key, lines, options) => {
        widgets.set(key, { lines, placement: options?.placement });
      },
      setStatus: (key, text) => {
        status = { key, text };
      },
      setTitle: (value) => {
        title = value;
      },
      setEditorText: (value) => {
        editorText = value;
      },
    },
    ["header", "footer", "status", "title", "editor"],
  );

  expect(widgets.get("remote_tui_demo_header")?.placement).toBe("aboveEditor");
  expect(widgets.get("remote_tui_demo_header")?.lines?.join("\n")).toContain("Remote TUI demo header");
  expect(widgets.get("remote_tui_demo_footer")?.placement).toBe("belowEditor");
  expect(widgets.get("remote_tui_demo_footer")?.lines?.join("\n")).toContain("Footer · 5 selected");
  expect(widgets.get("remote_tui_demo_footer")?.lines?.join("\n")).not.toContain("Esc");
  expect(status).toEqual({
    key: "remote-tui-demo",
    text: "Remote TUI demo: Header widget, Footer widget, Status bar, Window title, Prompt editor",
  });
  expect(title).toBe("Remote TUI capability lab");
  expect(editorText).toContain("Remote TUI demo applied");
});

test("showOverlay restores previous root component on hide", async () => {
  const previous = process.env.PI_GROK_REMOTE_TUI;
  process.env.PI_GROK_REMOTE_TUI = "1";

  let sessionStart:
    | ((event: unknown, ctx: { ui: { custom: (...args: unknown[]) => unknown; setWidget: (key: string, lines?: string[]) => void } }) => void)
    | undefined;
  const pi = {
    on: (_event: string, handler: typeof sessionStart) => {
      sessionStart = handler;
    },
    registerCommand: () => {},
  };
  const ui = {
    custom: async () => undefined,
    setWidget: () => {},
  };

  try {
    registerRemoteTui(pi as never);
    sessionStart?.({}, { ui });

    let tuiRef: {
      showOverlay: (component: {
        invalidate(): void;
        render(width: number): string[];
        handleInput?(data: string): void;
      }) => { hide: () => void };
    } | null = null;

    void ui.custom((tui, _theme, _kb, _done) => {
      tuiRef = tui as typeof tuiRef;
      return {
        invalidate() {},
        render: () => ["root"],
        handleInput() {},
      };
    });
    await new Promise((resolve) => setImmediate(resolve));
    await new Promise((resolve) => setImmediate(resolve));

    const handle = tuiRef!.showOverlay({
      invalidate() {},
      render: () => ["overlay"],
      handleInput() {},
    });
    handle.hide();
    expect(tuiRef).toBeTruthy();
  } finally {
    if (previous === undefined) delete process.env.PI_GROK_REMOTE_TUI;
    else process.env.PI_GROK_REMOTE_TUI = previous;
  }
});
