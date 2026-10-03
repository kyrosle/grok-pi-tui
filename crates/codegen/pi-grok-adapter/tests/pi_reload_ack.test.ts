import { afterAll, expect, mock, test } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

mock.module("@earendil-works/pi-coding-agent", () => ({
  DefaultPackageManager: class {}, SettingsManager: {}, getAgentDir: () => "/unused-fixture",
}));
const directory = mkdtempSync(join(tmpdir(), "pi-reload-handler-test-"));
afterAll(() => rmSync(directory, { recursive: true }));
const source = readFileSync(new URL("../../xai-grok-pager-bin/src/bin/grok_pi/tree_bridge.rs", import.meta.url), "utf8");
const embedded = source.split('r#"')[1]!.split('"#;')[0]!;
const path = join(directory, "bridge.ts");
writeFileSync(path, embedded);
const { default: register } = await import(path);
const commands = new Map<string, { handler(args: string, ctx: unknown): Promise<void> }>();
register({ registerCommand: (name: string, command: any) => commands.set(name, command) });
const reload = commands.get("__pi_reload")!.handler;

test("reload writes success only after the public ctx.reload resolves", async () => {
  const responsePath = join(directory, "success.json");
  let finish!: () => void;
  const pending = reload(JSON.stringify({ responsePath }), { reload: () => new Promise<void>((resolve) => { finish = resolve; }) });
  expect(existsSync(responsePath)).toBe(false);
  finish();
  await pending;
  expect(JSON.parse(readFileSync(responsePath, "utf8"))).toEqual({ ok: true });
});

test("a caught public ctx.reload error creates a false business ACK rather than RPC handled success", async () => {
  const responsePath = join(directory, "failure.json");
  await reload(JSON.stringify({ responsePath }), { async reload() { throw new Error("fixture loader failed"); } });
  const result = JSON.parse(readFileSync(responsePath, "utf8"));
  expect(result.ok).toBe(false);
  expect(result.error).toContain("fixture loader failed");
});

test("legacy no-ACK invocation still throws a reload error for Pi to report", async () => {
  await expect(reload("", { async reload() { throw new Error("fixture rejected"); } })).rejects.toThrow("fixture rejected");
});
