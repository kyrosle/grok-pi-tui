import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";

function frontend(browserLanguage = "en") {
	const web = resolve(import.meta.dir, "../web");
	let source = readFileSync(resolve(web, "app.js"), "utf8");
	for (const [marker, file] of [["UI_CONFIG", "ui-config.json"], ["I18N", "i18n.json"], ["MODELS", "models.js"], ["RESOURCES", "resources.js"], ["HOST", "host.js"], ["SETTINGS", "settings.js"]]) {
		source = source.replace("__PI_GROK_WEB_CONFIG_" + marker + "__", () => readFileSync(resolve(web, file!), "utf8"));
	}
	source = source.replace("void init();", `globalThis.api = {
		setState(value) { state = value; lang = detectLang(); },
		setDraft(value) { view.hostDraft = value; lang = detectLang(); },
		detectLang, languagePreference, hostLabel, hostDescription, hostOptionLabel
	};`);
	const context = vm.createContext({ navigator: { language: browserLanguage }, location: { hash: "", search: "" }, URLSearchParams, localStorage: { getItem: () => "en" } });
	new vm.Script(source).runInContext(context);
	return context.api;
}

test("explicit native language wins; auto uses the host OS then browser locale", () => {
	const api = frontend("zh-TW");
	expect(api.languagePreference()).toBe("auto");
	expect(api.detectLang()).toBe("zh");
	api.setState({ host: { ui: { language: "en" }, systemLanguage: "zh-CN" } });
	expect(api.detectLang()).toBe("en");
	api.setState({ host: { ui: { language: "auto" }, systemLanguage: "en" } });
	expect(api.detectLang()).toBe("en");
	api.setState({ host: { ui: { language: "zh-CN" }, systemLanguage: "en" } });
	expect(api.detectLang()).toBe("zh");
	api.setDraft({ language: "en" });
	expect(api.detectLang()).toBe("en");
	api.setDraft({});
	expect(api.detectLang()).toBe("zh");
});

test("host options display semantic translations while retaining canonical runtime values", () => {
	const api = frontend();
	const entry = { key: "pi_eval", label: "Code execution mode", options: ["v1", "v2"], localized: {
		en: { label: "Code execution mode", description: "Select execution behavior.", options: { v1: "Persistent REPL", v2: "Tool-enabled execution" } },
		"zh-CN": { label: "代码执行模式", description: "选择代码的执行方式。", options: { v1: "持久交互式", v2: "工具协作" } },
	} };
	api.setState({ host: { ui: { language: "zh-CN" } } });
	expect(api.hostLabel(entry)).toBe("代码执行模式");
	expect(api.hostDescription(entry)).toBe("选择代码的执行方式。");
	expect(entry.options.map(value => api.hostOptionLabel(entry, value))).toEqual(["持久交互式", "工具协作"]);
	expect(entry.options).toEqual(["v1", "v2"]);
	api.setState({ host: { ui: { language: "en" } } });
	expect(api.hostOptionLabel(entry, "v2")).toBe("Tool-enabled execution");
});
