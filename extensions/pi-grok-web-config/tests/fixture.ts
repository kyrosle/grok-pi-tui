
import { startWebConfigServer } from "../server.ts";
import { readdirSync,readFileSync,writeFileSync,mkdirSync } from "node:fs";
import { join,resolve } from "node:path";
import { createHash } from "node:crypto";
const root = resolve(import.meta.dir,"../../..");
const translations = JSON.parse(readFileSync(join(root,"crates/codegen/xai-grok-pager/src/settings/translations.json"),"utf8"));
const localized = (entry:any) => ({...entry,localized:{en:{label:entry.label,description:entry.description},"zh-CN":{label:translations[entry.label] || entry.label,description:translations[entry.description] || entry.description}}});
const catalog = readdirSync(join(root,"extensions")).flatMap(name=>{
 try { const manifest=JSON.parse(readFileSync(join(root,"extensions",name,"grok-pi.json"),"utf8"));return (manifest.settings||[]).map(entry=>localized({...entry,...entry.f2,source:"extensions/"+name+"/grok-pi.json"})); } catch { return []; }
});
catalog.push({key:"language",label:"Settings language",description:"Choose the language for settings.",default:"auto",kind:"string",options:["auto","en","zh-CN"],section:"display",source:"native/Pi settings registry",localized:{en:{label:"Settings language",description:"Choose the language for settings.",options:{auto:"System default",en:"English","zh-CN":"简体中文"}},"zh-CN":{label:"设置语言",description:"选择设置界面的语言。",options:{auto:"跟随系统",en:"English","zh-CN":"简体中文"}}}});
const state:any = {
 agentDir:"/demo/pi/agent",cwd:"/demo/workspace/grok-pi",paths:{models:"/demo/pi/agent/models.json",settings:"/demo/pi/agent/settings.json"},
 models:{customTopLevel:{preserve:true},providers:{
  openai:{name:"OpenAI",api:"openai-responses",baseUrl:"https://api.openai.com/v1",apiKey:"OPENAI_API_KEY",customOption:"preserve",models:[{id:"gpt-5",name:"GPT-5",contextWindow:400000,maxTokens:128000,reasoning:true,input:["text","image"],cost:{input:1.25,output:10,cacheRead:0.125,cacheWrite:0},compat:{supportsStore:false}}]},
  anthropic:{name:"Anthropic",api:"anthropic-messages",baseUrl:"https://api.anthropic.com",models:[{id:"claude-sonnet",name:"Claude Sonnet",contextWindow:200000,maxTokens:64000}]},
  local:{name:"Local development",api:"openai-completions",baseUrl:"http://localhost:11434/v1",models:[]}
 }},
 settings:{defaultProvider:"openai",defaultModel:"gpt-5",compaction:{enabled:true,reserveTokens:16384},customFuture:{keep:"yes"},extensions:["/demo/extensions/review.ts"],skills:["/demo/skills/local"]},
 current:{provider:"openai",modelId:"gpt-5"},defaults:{provider:"openai",modelId:"gpt-5"},
 providerAuth:{openai:{configured:true,source:"env",label:"environment"},anthropic:{configured:false}},
 resources:{extensions:[{path:"/demo/extensions/review.ts",name:"Review helper",source:"settings"},{path:"/demo/cli/bridge.ts",name:"Session bridge",source:"cli"}],skills:[{name:"Review code",path:"/demo/skills/review/SKILL.md",description:"Review changes before sharing.",source:"discovered"}],prompts:[{name:"Explain changes",path:"/demo/prompts/explain.md",description:"Explain the reasoning behind a code change.",source:"discovered"}],themes:[{name:"Graphite",path:"/demo/themes/graphite.json",source:"discovered"}]},
 host:{grokHome:"/demo/grok-pi",configPath:"/demo/grok-pi/config.toml",ui:{language:"en",pi_subagents:true,terminal_custom:"value"},systemLanguage:"zh-CN",uiTables:{keybindings:{"Ctrl+K":"search"}},catalog}
};
const settingsVersion=()=>createHash("sha256").update(JSON.stringify(state.settings)).digest("hex");
export async function fixture() {
 let writes=0;
 const server = await startWebConfigServer({host:"127.0.0.1",port:0,uiHtmlPath:join(root,"extensions/pi-grok-web-config/web/index.html"),
	loadState:async()=>({...structuredClone(state),settingsVersion:settingsVersion()}),
 saveModels:async doc=>{ state.models=doc;writes++; },
	 saveSettings:async(doc,version)=>{if(version!==settingsVersion())throw Object.assign(new Error("settings changed"),{statusCode:409});state.settings=doc;state.defaults={provider:state.settings.defaultProvider,modelId:state.settings.defaultModel};writes++;},
 saveHostUi:async updates=>{Object.assign(state.host.ui,updates);writes++;},
 useModel:async(provider,modelId)=>{state.current={provider,modelId};},
 reload:async()=>{},
 });
 return {server,state,get writes(){return writes;}};
}
if(import.meta.main) {
 const {server}=await fixture();
 mkdirSync(join(root,".impeccable/review"),{recursive:true});
 writeFileSync(join(root,".impeccable/review/server.json"),JSON.stringify({url:server.url}));
 console.log("Fixture ready on port "+server.port+" (synthetic data only)");
}
