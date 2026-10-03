import { test, expect } from "bun:test";
import { fixture } from "./fixture.ts";

test("settings HTTP save requires a server revision and rejects a package change after preflight", async () => {
	const f = await fixture();
	const token = new URL(f.server.url).searchParams.get("token")!;
	const request = (method: string, doc?: unknown, version?: string) => fetch(new URL("/api/" + (method === "GET" ? "state" : "settings"), f.server.url), {
		method,
		headers: { "x-pi-token": token, "content-type": "application/json", ...(version ? { "if-match": `"${version}"` } : {}) },
		body: doc === undefined ? undefined : JSON.stringify(doc),
	});
	try {
		const before = await (await request("GET")).json();
		const draft = { ...before.settings, defaultThinkingLevel: "high" };
		expect((await request("PUT", draft)).status).toBe(428);
		expect(f.writes).toBe(0);
		// An official/native package change between GET and PUT must not be
		// overwritten by the Web editor's otherwise valid stale document.
		f.state.settings.packages = ["npm:@fixture/package@1.0.0"];
		expect((await request("PUT", draft, before.settingsVersion)).status).toBe(409);
		expect(f.writes).toBe(0);
		expect(f.state.settings.packages).toEqual(["npm:@fixture/package@1.0.0"]);
		const current = await (await request("GET")).json();
		expect((await request("PUT", { ...current.settings, defaultThinkingLevel: "high" }, current.settingsVersion)).status).toBe(200);
		expect(f.writes).toBe(1);
		expect(f.state.settings.customFuture).toEqual({ keep: "yes" });
		expect(f.state.settings.packages).toEqual(["npm:@fixture/package@1.0.0"]);
	} finally {
		await f.server.close();
	}
});
