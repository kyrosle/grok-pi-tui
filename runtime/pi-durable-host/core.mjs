// Pi owns execution and storage. This module only composes its public SDKs.
import { mkdir, readFile, realpath, readdir, writeFile, stat, chmod } from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import { join } from 'node:path';
import { homedir } from 'node:os';
import lockfile from 'proper-lockfile';
import { EnvHttpProxyAgent, getGlobalDispatcher, setGlobalDispatcher, fetch as sdkFetch } from 'undici';
import { BACKGROUND_CONTEXT as context } from '@earendil-works/chord/context';
import { ModelRuntime, SettingsManager, loadProjectContextFiles, loadSkills, formatSkillsForPrompt } from '@earendil-works/pi-coding-agent';
import { Harness, createRegistry, defineExtension, defineDoc, section, watchEvents, ROOT_CONVERSATION_ID } from '@earendil-works/pi-durable';
import { NodeExecutionEnv } from '@earendil-works/pi-durable/env/node';
import { CodingTools } from '@earendil-works/pi-durable/tools';
import { openNodeSqliteStorage } from '@earendil-works/pi-durable/storage/sqlite/node';
import { Subagent } from './subagent.mjs';

export const SDK_VERSION = '1.1.0';
// These two published SDKs share the SQLite schema. New timing fields are
// optional on old records; the official adapter owns reading them. Preserve
// the store's original manifest and reject unknown versions.
const STORE_SDK_VERSIONS = new Set(['1.0.4', SDK_VERSION]);
export { context, ROOT_CONVERSATION_ID };
const ViewSelection = defineDoc({ kind: 'grok-pi.view-selection', version: 1, scope: 'session', initial: () => ({ conversationId: ROOT_CONVERSATION_ID }) });

export function parseLocator(locator) {
  const m = /^durable:([0-9a-f-]{36}):(\d+)$/.exec(locator ?? '');
  if (!m || !Number.isSafeInteger(Number(m[2]))) throw new Error('Expected a Durable session locator; Pi JSONL sessions use --no-durable.');
  return { storeId: m[1], conversationId: Number(m[2]) };
}

export async function projectRoot(home, cwd) {
  const canonical = await realpath(cwd);
  return { cwd: canonical, root: join(home, 'durable', createHash('sha256').update(canonical).digest('hex').slice(0, 20)) };
}

export async function listStores(root) {
  const rows = [];
  for (const item of await readdir(root, { withFileTypes: true }).catch(() => [])) {
    if (!item.isDirectory() || !/^[0-9a-f-]{36}$/.test(item.name)) continue;
    const directory = join(root, item.name);
    try {
      const info = JSON.parse(await readFile(join(directory, 'session.json'), 'utf8'));
      const times = await stat(join(directory, 'session.sqlite-wal')).catch(() => stat(join(directory, 'session.sqlite')));
      if (info.storeId === item.name && STORE_SDK_VERSIONS.has(info.sdkVersion)) rows.push({ ...info, updatedAt: times.mtime.toISOString(), directory });
    } catch { /* Incomplete/foreign stores are never silently opened. */ }
  }
  return rows.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

export async function selectStore(options) {
  const { cwd, root } = await projectRoot(options.home, options.cwd);
  let id = options.session ? parseLocator(options.session).storeId : options.storeId;
  if (id && !/^[0-9a-f-]{36}$/.test(id)) throw new Error('Invalid Durable store ID');
  if (!id && options.continue) id = (await listStores(root))[0]?.storeId;
  const created = !id;
  id ??= randomUUID();
  const directory = join(root, id);
  if (created) {
    await mkdir(directory, { recursive: true, mode: 0o700 });
    await writeFile(join(directory, 'session.json'), JSON.stringify({ storeId: id, cwd, sdkVersion: SDK_VERSION, createdAt: new Date().toISOString(), name: options.name ?? 'Pi Durable' }), { flag: 'wx', mode: 0o600 });
  } else {
    const info = JSON.parse(await readFile(join(directory, 'session.json'), 'utf8'));
    if (info.cwd !== cwd || info.storeId !== id || !STORE_SDK_VERSIONS.has(info.sdkVersion)) throw new Error('Durable store identity or unsupported SDK version; no migration was performed.');
  }
  return { root, directory, cwd, created, storeId: id, conversationId: options.session ? parseLocator(options.session).conversationId : undefined };
}

export function createSettings(settings) {
  return {
    get stream() { return { ...settings.getProviderRetrySettings(), timeoutMs: settings.getProviderRetrySettings().timeoutMs ?? (settings.getHttpIdleTimeoutMs() || 2147483647) }; },
    get compaction() { return settings.getCompactionSettings(); },
    get retry() { return settings.getRetrySettings(); },
    get steeringMode() { return settings.getSteeringMode(); },
    get followUpMode() { return settings.getFollowUpMode(); },
  };
}

export async function openCore(location, options = {}, overrides = {}) {
  const release = await lockfile.lock(location.directory, { realpath: false, stale: 10000, retries: { retries: 12, minTimeout: 1000, maxTimeout: 1000 } });
  const agentDir = options.agentDir ?? process.env.PI_CODING_AGENT_DIR ?? join(homedir(), '.pi', 'agent');
  const settings = SettingsManager.create(location.cwd, agentDir);
  const timeout = settings.getHttpIdleTimeoutMs();
  const proxy = settings.getGlobalSettings().httpProxy;
  const dispatcher = new EnvHttpProxyAgent({ ...(proxy ? { httpProxy: proxy, httpsProxy: proxy } : {}), bodyTimeout: timeout, headersTimeout: timeout });
  const previousDispatcher = getGlobalDispatcher();
  setGlobalDispatcher(dispatcher);
  globalThis.fetch = sdkFetch;
  const envs = new Map();
  let harness;
  try {
    const models = overrides.models ?? await ModelRuntime.create({ allowModelNetwork: false });
    const registry = createRegistry();
    registry.install(CodingTools);
    registry.install(Subagent);
    for (const extension of overrides.extensions ?? []) registry.install(extension);
    // Loading context is data-only; ordinary ExtensionAPI factories are never executed.
    const contextFiles = options.noContextFiles ? [] : loadProjectContextFiles({ cwd: options.approve ? location.cwd : agentDir, agentDir }).filter(file => options.approve || file.path.startsWith(`${agentDir}/`));
    const skills = options.noSkills ? [] : loadSkills({ cwd: options.approve ? location.cwd : agentDir, agentDir, skillPaths: [], includeDefaults: true }).skills;
    registry.install(defineExtension({ name: 'grok-pi.prompt', sections: [section('preamble', () => [
      options.systemPrompt ?? 'You are a coding assistant. Use the available tools to inspect and change the project. Verify your work. Interrupted tools may have partially executed; never assume they succeeded or retry side effects without checking.',
      `Working directory: ${location.cwd}`,
      ...contextFiles.map(file => `${file.path}\n${file.content}`),
      formatSkillsForPrompt(skills), ...(options.appendSystemPrompts ?? []),
    ].filter(Boolean).join('\n\n'), { tag: false })] }));
    const env = ({ cwd = location.cwd }) => {
      if (!envs.has(cwd)) envs.set(cwd, new NodeExecutionEnv({ cwd }));
      return envs.get(cwd);
    };
    harness = await Harness.open(await openNodeSqliteStorage(join(location.directory, 'session.sqlite')), { models, registry, settings: createSettings(settings), env, onReport: error => console.error(String(error)) }, context);
    const available = models.getAvailableSnapshot?.() ?? models.getModels();
    const provider = options.provider ?? (options.model?.includes('/') ? undefined : settings.getDefaultProvider());
    const pattern = options.model ?? settings.getDefaultModel();
    const chosen = available.find(m => (!provider || m.provider === provider) && (!pattern || m.id === pattern || `${m.provider}/${m.id}` === pattern));
    const existingRoot = await harness.commit(tx => tx.conversation(ROOT_CONVERSATION_ID), context);
    if ((!existingRoot || options.model || options.provider) && !chosen) throw new Error('No configured chat model found. Configure credentials with Pi /login, then start Durable again.');
    const root = await harness.root(context, { agent: {
      cwd: location.cwd,
      ...(chosen ? { model: { provider: chosen.provider, modelId: chosen.id } } : {}),
      thinkingLevel: options.thinking ?? settings.getDefaultThinkingLevel() ?? 'off',
    } });
    const saved = options.continue && location.conversationId == null ? await harness.snapshot(ViewSelection, context) : undefined;
    let conversation = location.conversationId == null && !saved ? root : await harness.conversation(location.conversationId ?? saved.conversationId, context);
    if (!conversation) throw new Error('Durable conversation does not exist.');
    const excluded = new Set((options.excludeTools ?? '').split(',').map(name => name.trim()).filter(Boolean));
    const tools = options.noTools ? [] : typeof options.tools === 'string' ? options.tools.split(',').map(name => name.trim()).filter(Boolean).map(name => {
      const tool = registry.snapshot().tools().find(item => item.tool.name === name)?.tool;
      if (!tool) throw new Error(`Unsupported Durable tool: ${name}`);
      return tool;
    }).filter(tool => !excluded.has(tool.name)) : excluded.size ? registry.snapshot().tools().map(item => item.tool).filter(tool => !excluded.has(tool.name)) : undefined;
    // root() only applies agent options when creating a root. CLI/F2 tool
    // restrictions must also apply when reopening an existing conversation.
    if (options.model || options.provider || options.thinking || tools !== undefined) await conversation.configure({ ...((options.model || options.provider) && chosen ? { model: { provider: chosen.provider, modelId: chosen.id } } : {}), ...(options.thinking ? { thinkingLevel: options.thinking } : {}), ...(tools !== undefined ? { tools } : {}) }, context);
    for (const name of ['session.sqlite', 'session.sqlite-wal', 'session.sqlite-shm']) await chmod(join(location.directory, name), 0o600).catch(error => { if (error.code !== 'ENOENT') throw error; });
    const graph = await harness.taskGraph(context);
    const recovery = [];
    for (const node of Object.values(graph.value.tasks)) {
      if (node.kind !== 'pi.tool' || node.state.phase !== 'execute') continue;
      const task = await harness.getTask(node.id, context);
      const entry = await harness.commit(tx => tx.entry(task.input.assistant), context);
      const call = entry?.model?.[0]?.content?.find(block => block.type === 'toolCall' && block.id === task.input.callId);
      const policy = registry.snapshot().tools().find(item => item.tool.name === call?.name)?.tool.replay;
      if (task?.state.checkpoint?.replay !== 'safe' || policy !== 'safe') recovery.push({ taskId: node.id, conversationId: node.conversationId, tool: call?.name, message: 'Interrupted tool may have partially executed. Inspect its effects before continuing.' });
    }
    graph.dispose();
    let resumed = false;
    const core = {
      location, harness, models, registry, settings, recovery,
      get conversation() { return conversation; },
      get locator() { return `durable:${location.storeId}:${conversation.id}`; },
      async attach(id) {
        const next = await harness.conversation(Number(id), context);
        if (!next) throw new Error('Unknown Durable conversation.');
        await harness.commit(async tx => { (await tx.doc(ViewSelection)).conversationId = next.id; }, context);
        conversation = next;
      },
      resume(confirmed = false) {
        if (recovery.length && !confirmed) throw new Error('Recovery decision required for interrupted unsafe tools.');
        harness.resume(); resumed = true; recovery.length = 0;
      },
      get resumed() { return resumed; },
      async snapshot() { const events = await watchEvents(harness, conversation.id, context); const snapshot = events.snapshot; await events.stop(); return snapshot; },
      async close() { await harness.close(context); for (const value of envs.values()) await value.cleanup(context); await dispatcher.close(); await release(); },
    };
    return core;
  } catch (error) {
    // A failed target must not leave the still-active core using a closed
    // process-global HTTP dispatcher.
    if (getGlobalDispatcher() === dispatcher) setGlobalDispatcher(previousDispatcher);
    await harness?.close(context);
    for (const value of envs.values()) await value.cleanup(context);
    await dispatcher.close(); await release(); throw error;
  }
}
