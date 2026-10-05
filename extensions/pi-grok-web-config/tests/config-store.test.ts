
import { test, expect, mock, afterAll } from "bun:test";
import { mkdtempSync, writeFileSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import vm from "node:vm";
const dir=mkdtempSync(join(tmpdir(),"pi-web-config-tests-"));
const previousHome=process.env.GROK_HOME, previousCatalog=process.env.PI_GROK_WEB_CONFIG_CATALOG;
process.env.GROK_HOME=dir;
const hostCatalogPath=join(dir,"host-catalog.json");
writeFileSync(hostCatalogPath,JSON.stringify([{source:"native/Pi settings registry",systemLanguage:"zh-CN",manifest:{settings:[{key:"pi_bash",kind:"bool",default:true},{key:"language",kind:"string",default:"auto",options:["auto","en","zh-CN"],localized:{en:{label:"Settings language"},"zh-CN":{label:"设置语言"}}}]}}]));
process.env.PI_GROK_WEB_CONFIG_CATALOG=hostCatalogPath;
mock.module("@earendil-works/pi-coding-agent",()=>({
 getAgentDir:()=>dir,
 DefaultResourceLoader:class { async reload(){} getSkills(){return {skills:[]}} getPrompts(){return {prompts:[]}} getThemes(){return {themes:[]}} }
}));
const store=await import("../config-store.ts");
afterAll(()=>{if(previousHome===undefined)delete process.env.GROK_HOME;else process.env.GROK_HOME=previousHome;if(previousCatalog===undefined)delete process.env.PI_GROK_WEB_CONFIG_CATALOG;else process.env.PI_GROK_WEB_CONFIG_CATALOG=previousCatalog;});
test("assembled frontend compiles and embedded translations have parity",()=>{
 const web=resolve(import.meta.dir,"../web");
 const dict=JSON.parse(readFileSync(join(web,"i18n.json"),"utf8"));
 expect(Object.keys(dict.en).sort()).toEqual(Object.keys(dict.zh).sort());
 let app=readFileSync(join(web,"app.js"),"utf8");
 for(const [marker,file] of [["UI_CONFIG","ui-config.json"],["I18N","i18n.json"],["MODELS","models.js"],["RESOURCES","resources.js"],["HOST","host.js"],["SETTINGS","settings.js"]]) app=app.replace("__PI_GROK_WEB_CONFIG_"+marker+"__",()=>readFileSync(join(web,file!),"utf8"));
 expect(()=>new vm.Script(app)).not.toThrow();
 const html=readFileSync(join(web,"index.html"),"utf8");
 for(const match of html.matchAll(/data-i18n(?:-placeholder|-aria)?="([^"]+)"/g)) {expect(dict.en[match[1]!]).toBeString();expect(dict.zh[match[1]!]).toBeString();}
 const config=JSON.parse(readFileSync(join(web,"ui-config.json"),"utf8"));
 for(const field of config.settings.groups.flatMap(group=>group.fields)) {
  expect(dict.zh["settings_field_"+field.key.replaceAll(".","_")]).toBeString();
  expect(dict.zh["settings_desc_"+field.key.replaceAll(".","_")]).toBeString();
 }
 expect(Object.keys(dict.en).some(key=>key.startsWith("host_label_")||key.startsWith("host_desc_"))).toBe(false);
});
test("model validator rejects duplicate IDs, invalid numbers and headers",()=>{
 const doc=(models:any[])=>({providers:{example:{models}}});
 expect(store.validateModelsDoc(doc([{id:"same"},{id:"same"}]))).toContain("duplicate");
 expect(store.validateModelsDoc(doc([{id:"m",contextWindow:-1}]))).toContain("positive integer");
 expect(store.validateModelsDoc(doc([{id:"m",cost:{input:-0.1}}]))).toContain("non-negative");
 expect(store.validateModelsDoc({providers:{example:{headers:{"X-Test":4}}}})).toContain("string values");
 expect(store.validateModelsDoc(doc([{id:"m",customFuture:true,cost:{input:0.25}}]))).toBeUndefined();
});
test("JSONC parser preserves URL and comment-like string contents",()=>{
 expect(JSON.parse(store.stripJsonComments('{"url":"https://example.test","value":"/*keep*/"} // comment'))).toEqual({url:"https://example.test",value:"/*keep*/"});
});
test("state preserves unknown model fields and reports invalid document shapes",async()=>{
 writeFileSync(join(dir,"models.json"),JSON.stringify({future:{keep:true},providers:{example:{models:[{id:"m"}]}}}));
 writeFileSync(join(dir,"settings.json"),"{}");
 let state=await store.collectState(undefined);
 expect((state.models as any).future).toEqual({keep:true});
 writeFileSync(join(dir,"settings.json"),"[]");
 expect((await store.collectState(undefined)).settingsError).toContain("JSON object");
 writeFileSync(join(dir,"models.json"),'{"providers":[]}');
 expect((await store.collectState(undefined)).modelsError).toContain("providers");
});
test("TOML inline comments do not turn booleans into strings",()=>{
 writeFileSync(join(dir,"config.toml"),'[ui]\npi_bash = true # user preference\nlabel = "value#kept" # comment\n[voice]\nlanguage = "en"\n');
 const state=store.collectHostState();
 expect(state.ui.pi_bash).toBe(true);expect(state.ui.label).toBe("value#kept");
 store.saveHostUi(join(dir,"config.toml"),{pi_bash:false});
 const result=readFileSync(join(dir,"config.toml"),"utf8");
 expect(result).toContain('label = "value#kept" # comment');expect(result).toContain('[voice]\nlanguage = "en"');
 expect(()=>store.saveHostUi(join(dir,"config.toml"),{bad:NaN})).toThrow();
});
test("host writes reject unsupported keys before touching existing configuration",()=>{
 const path=join(dir,"config.toml");
 const original='[ui]\npi_bash = true\nvoice_keybind_enabled = true\ncoding_data_sharing = "opt-out"\nfuture_setting = "keep"\n';
 writeFileSync(path,original);
 for(const key of ["voice_keybind_enabled","coding_data_sharing","permission_mode","future_setting"]){
  expect(()=>store.saveHostUi(path,{pi_bash:false,[key]:false})).toThrow("unavailable for Pi");
  expect(readFileSync(path,"utf8")).toBe(original);
 }
 store.saveHostUi(path,{pi_bash:false});
 expect(readFileSync(path,"utf8")).toContain('future_setting = "keep"');
 expect(readFileSync(path,"utf8")).toContain('voice_keybind_enabled = true');
});
test("host catalog retains shared translations, canonical options and system language",()=>{
 const host=store.collectHostState();
 expect(host.systemLanguage).toBe("zh-CN");
 const language=host.catalog.find(entry=>entry.key==="language")!;
 expect(language.localized?.["zh-CN"]?.label).toBe("设置语言");
 expect(language.options).toEqual(["auto","en","zh-CN"]);
});
test("language preference round-trips in the existing UI table without changing unrelated values",()=>{
 const path=join(dir,"config.toml");
 writeFileSync(path,'[ui]\npi_bash = false\nfuture_setting = "keep"\n[voice]\nlanguage = "en"\n');
 for(const language of ["auto","zh-CN","en"]) {
  store.saveHostUi(path,{language});
  expect(store.collectHostState().ui.language).toBe(language);
  expect(readFileSync(path,"utf8")).toContain('future_setting = "keep"');
  expect(readFileSync(path,"utf8")).toContain('[voice]\nlanguage = "en"');
 }
 const before=readFileSync(path,"utf8");
 expect(()=>store.saveHostUi(path,{language:"zh"})).toThrow("language must be");
 expect(readFileSync(path,"utf8")).toBe(before);
});
test("conditional settings save rejects an external package edit and preserves unknown fields",async()=>{
 const path=join(dir,"settings.json");
 writeFileSync(path,JSON.stringify({customFuture:{keep:true},packages:[]}));
 const before=await store.collectState(undefined);
 writeFileSync(path,JSON.stringify({customFuture:{keep:true},packages:["npm:@fixture/tools"]}));
 expect(()=>store.saveSettingsDoc(path,{...before.settings,defaultThinkingLevel:"high"},before.settingsVersion)).toThrow("changed outside this draft");
 expect(JSON.parse(readFileSync(path,"utf8")).packages).toEqual(["npm:@fixture/tools"]);
 const current=await store.collectState(undefined);
 store.saveSettingsDoc(path,{...current.settings,defaultThinkingLevel:"high"},current.settingsVersion);
 const saved=JSON.parse(readFileSync(path,"utf8"));
 expect(saved.customFuture).toEqual({keep:true});
 expect(saved.packages).toEqual(["npm:@fixture/tools"]);
 expect(saved.defaultThinkingLevel).toBe("high");
 expect((await store.collectState(undefined)).settingsVersion).not.toBe(current.settingsVersion);
});
