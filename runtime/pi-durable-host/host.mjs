import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';
import { randomUUID } from 'node:crypto';
import { realpath } from 'node:fs/promises';
import { watchEvents } from '@earendil-works/pi-durable';
import { context, SDK_VERSION, ROOT_CONVERSATION_ID, openCore, selectStore, listStores, parseLocator } from './core.mjs';

import caps from './capabilities.json' with { type: 'json' };
export const CAPABILITIES = caps;

// A controller around SDK handles; there is no scheduler or authoritative queue here.
export async function createController(options, overrides = {}, emit = () => {}) {
  let core = await openCore(await selectStore(options), options, overrides);
  let stream, view, graph, subscriptions = [];
  const inputIds = new Map(); // UI correlation only; the SDK inbox remains authoritative.
  let epoch = randomUUID(), sequence = 0;
  let controls = Promise.resolve();
  let closed = false;
  const event = value => emit({ kind: 'event', epoch, sequence: ++sequence, session: core.locator, event: value });
  const viewEvent = value => ({ type: 'view_state', docs: { 'pi.agent': value.docs['pi.agent'], 'pi.usage': value.docs['pi.usage'], 'pi.inbox': value.docs['pi.inbox'] }, inbox: (value.docs['pi.inbox']?.items ?? []).map(item => ({ ...item, requestId: inputIds.get(item.id) })) });
  let latestView, latestGraph, flushPending = false;
  const flush = () => {
    if (flushPending) return;
    flushPending = true;
    setImmediate(async () => {
      try {
        while (latestView || latestGraph) {
          const nextView = latestView, nextGraph = latestGraph; latestView = latestGraph = undefined;
          if (nextView) await event(viewEvent(nextView));
          if (nextGraph) await event({ type: 'task_graph', graph: nextGraph });
        }
      } finally { flushPending = false; }
    });
  };
  const catalog = () => (core.models.getAvailableSnapshot?.() ?? core.models.getModels()).map(m => ({ provider: m.provider, id: m.id, name: m.name, contextWindow: m.contextWindow, maxTokens: m.maxTokens, reasoning: m.reasoning, api: m.api, input: m.input, cost: m.cost }));
  async function detach() {
    latestView = latestGraph = undefined;
    subscriptions.forEach(fn => fn()); subscriptions = [];
    await stream?.stop(); stream = undefined;
    view?.dispose(); graph?.dispose(); view = graph = undefined;
  }
  async function watch() {
    await detach(); epoch = randomUUID(); sequence = 0;
    stream = await watchEvents(core.harness, core.conversation.id, context);
    view = await core.conversation.viewState(context);
    graph = await core.harness.taskGraph(context);
    for (const record of (await core.harness.inspect(context)).submissions) if (record.requestId) inputIds.set(record.id, record.requestId);
    await event(stream.snapshot);
    await event(viewEvent(view.value));
    await event({ type: 'task_graph', graph: graph.value });
    stream.start(async events => { for (const value of events) {
      if (value.type === 'run_start') {
        const promptIds = [];
        for (const id of value.inputs) { const handle = await core.harness.submission(id, context); const record = await handle?.status(context); if (record?.requestId) { inputIds.set(id, record.requestId); promptIds.push(record.requestId); } }
        await event({ ...value, promptIds });
      } else await event(value);
    }
      // Transport barrier after the whole committed batch, not a storage cursor.
      for (const value of events) if (value.type === 'submission' && ['done', 'unanswered'].includes(value.record.status)) await event({ type: 'settled', id: value.record.id });
    });
    subscriptions.push(view.subscribe(value => { latestView = value; flush(); }));
    subscriptions.push(graph.subscribe(value => { latestGraph = value; flush(); }));
    if (!core.recovery.length) core.resume();
  }
  async function handle(id) {
    const submission = await core.harness.submission(Number(id), context);
    if (!submission) throw new Error('Unknown Durable submission.');
    return submission;
  }
  async function info() {
    return { ...CAPABILITIES, backgroundOwner: options.backgroundOwner === true, session: core.locator, location: core.location, models: catalog(), snapshot: await core.snapshot(), recovery: core.recovery, paused: !core.resumed };
  }
  return {
    get core() { return core; },
    async request(method, params = {}) {
      if (['new', 'attach', 'watch', 'detach', 'configure', 'recover', 'submit'].includes(method)) {
        const next = controls.then(() => this.dispatch(method, params)); controls = next.catch(() => {}); return next;
      }
      return this.dispatch(method, params);
    },
    async dispatch(method, params = {}) {
      switch (method) {
        case 'hello': return info();
        case 'watch': await watch(); return { attached: true };
        case 'snapshot': return core.snapshot();
        case 'submit': {
          if (!core.resumed) core.resume();
          if (typeof params.text !== 'string' || !params.text.trim() || typeof params.requestId !== 'string' || !params.requestId) throw new Error('text and stable requestId are required.');
          if (params.whenBusy && !['steer', 'followUp', 'reject'].includes(params.whenBusy)) throw new Error('Unsupported queue mode.');
          const submission = await core.conversation.submit({ type: 'input', content: params.text, requestId: params.requestId, whenBusy: params.whenBusy ?? 'steer' }, context);
          inputIds.set(submission.id, params.requestId);
          if (view) event(viewEvent(view.value));
          return { id: submission.id, ...(await submission.status(context)) };
        }
        case 'wait': if (!core.resumed) core.resume(); return (await handle(params.id)).wait(context);
        case 'status': return (await handle(params.id)).status(context);
        case 'withdraw': {
          const numeric = Number(params.id);
          const record = Number.isSafeInteger(numeric) ? undefined : await core.harness.commit(tx => tx.submissionByRequest(core.conversation.id, params.id), context);
          return { result: await core.harness.abortSubmission(record?.id ?? numeric, context, core.conversation.id) };
        }
        case 'recover': {
          if (params.action === 'continue') core.resume(true);
          else if (params.action === 'abort') {
            // Recovery is store-wide. Mark work before enabling scheduling;
            // aborting only the viewed conversation misses other owners.
            const inspection = await core.harness.inspect(context);
            for (const submission of inspection.submissions) await core.harness.abortSubmission(submission.id, context);
            for (const task of inspection.tasks) await core.harness.abortTask(task.record.id, context);
            core.resume(true);
            await Promise.all(inspection.tasks.map(task => core.harness.waitForTask(task.record.id, context)));
          }
          else throw new Error('Recovery action must be continue or abort.');
          return { resumed: true };
        }
        case 'abort': if (!core.resumed) core.resume(); await core.conversation.abort(context, { background: params.background === true }); return {};
        case 'compact': {
          if (!core.resumed) core.resume();
          const id = await core.conversation.compact(params.instructions, context);
          const result = await core.harness.waitForTask(id, context); return result.state;
        }
        case 'configure': {
          if (params.model && !catalog().some(m => m.provider === params.model.provider && m.id === params.model.modelId)) throw new Error('Model is not configured/available.');
          if (params.thinkingLevel && !['off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max'].includes(params.thinkingLevel)) throw new Error('Invalid thinking level.');
          await core.conversation.configure({ ...(params.model ? { model: params.model } : {}), ...(params.thinkingLevel ? { thinkingLevel: params.thinkingLevel } : {}) }, context); return info();
        }
        case 'task': return core.harness.getTask(Number(params.id), context);
        case 'tasks': { const value = await core.harness.taskGraph(context); const result = value.value; value.dispose(); return result; }
        case 'sessionInfo': {
          const snapshot = await core.snapshot();
          const messages = snapshot.entries.flatMap(entry => entry.model ?? []);
          const totals = { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 };
          let totalCost = 0;
          for (const usage of [...Object.values(snapshot.usage.models ?? {}), ...Object.values(snapshot.usage.tools ?? {})]) {
            for (const key of Object.keys(totals)) totals[key] += Number(usage[key] ?? 0);
            totalCost += Number(usage.cost?.total ?? 0);
          }
          totals.total = totals.input + totals.output + totals.cacheRead + totals.cacheWrite;
          const last = messages.findLast(message => message.role === 'assistant' && message.usage);
          const used = last ? ['input', 'output', 'cacheRead', 'cacheWrite'].reduce((sum, key) => sum + Number(last.usage[key] ?? 0), 0) : 0;
          return { messages: { messages }, stats: { tokens: totals, cost: totalCost, userMessages: messages.filter(m => m.role === 'user').length, assistantMessages: messages.filter(m => m.role === 'assistant').length, toolCalls: messages.flatMap(m => Array.isArray(m.content) ? m.content : []).filter(b => b.type === 'toolCall').length, toolResults: messages.filter(m => m.role === 'toolResult').length, totalMessages: messages.length, contextUsage: { tokens: used }, sessionFile: `${core.location.directory}/session.sqlite` }, session: core.locator, cwd: core.location.cwd };
        }
        case 'conversations': return (await core.harness.commit(tx => tx.scanConversations({}, 256), context)).items;
        case 'sessions': {
          const rows = (await listStores(core.location.root)).map(row => ({ id: `durable:${row.storeId}:${ROOT_CONVERSATION_ID}`, name: row.name, summary: row.name, cwd: row.cwd, createdAt: row.createdAt, updatedAt: row.updatedAt, sessionPath: row.directory }));
          const conversations = await this.request('conversations');
          for (const conversation of conversations) if (conversation.id !== ROOT_CONVERSATION_ID) rows.push({ id: `durable:${core.location.storeId}:${conversation.id}`, name: `Durable conversation #${conversation.id}`, summary: conversation.owner ? `Child of task #${conversation.owner.taskId}` : 'Durable conversation', cwd: core.location.cwd, sessionPath: core.location.directory, updatedAt: new Date().toISOString() });
          return rows;
        }
        case 'attach': {
          const identity = parseLocator(params.session);
          if (identity.storeId === core.location.storeId) await core.attach(identity.conversationId);
          else {
            if (options.backgroundOwner) throw new Error('A background Durable owner is bound to one store. Restart with --session to attach another owner.');
            if (Object.keys((await this.request('tasks')).tasks).length) throw new Error('Pause running Durable work before switching stores.');
            // Admit the target before detaching/closing the current core.
            const next = await openCore(await selectStore({ ...options, session: params.session }), options, overrides);
            await detach();
            const previous = core; core = next;
            await previous.close();
          }
          await detach(); return info();
        }
        case 'new': {
          // A fresh conversation is an official Harness operation, not a reset of old storage.
          const agent = (await core.snapshot()).agent;
          const selected = agent.tools?.map(name => core.registry.snapshot().tools().find(item => item.tool.name === name)?.tool).filter(Boolean);
          const conversation = await core.harness.createConversation({ ownership: { kind: 'ownerless' }, agent: { cwd: core.location.cwd, model: agent.model, thinkingLevel: agent.thinkingLevel, ...(selected ? { tools: selected } : {}) } }, context);
          await core.attach(conversation.id); await detach(); return info();
        }
        case 'detach': await detach(); return {};
        default: throw new Error(`Unsupported Durable operation: ${method}`);
      }
    },
    async close() { if (closed) return; closed = true; await controls; await detach(); await core.close(); },
  };
}

export async function startStdio(options, overrides = {}) {
  const write = value => new Promise(resolve => { process.stdout.write(`${JSON.stringify(value)}\n`, resolve); });
  const controller = await createController(options, overrides, write);
  const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
  let closing = false;
  async function close() { if (closing) return; closing = true; input.close(); await controller.close(); }
  input.on('line', line => {
    if (line.length > 16 * 1024 * 1024) { console.error('Durable request too large'); close(); return; }
    let request;
    try { request = JSON.parse(line); if (typeof request.id !== 'string' || typeof request.method !== 'string') throw new Error('Invalid request envelope'); }
    catch (error) { write({ kind: 'response', id: request?.id ?? '', error: String(error) }); return; }
    (async () => {
      try {
        if (request.method === 'close') { await close(); write({ kind: 'response', id: request.id, result: {} }); process.stdin.pause(); return; }
        const result = await controller.request(request.method, request.params);
        write({ kind: 'response', id: request.id, result: result ?? null });
      } catch (error) { write({ kind: 'response', id: request.id, error: String(error) }); }
    })();
  });
  input.on('close', () => { close().catch(error => console.error(String(error))); });
  process.once('SIGTERM', () => close().finally(() => process.exit(0)));
  process.once('SIGINT', () => close().finally(() => process.exit(0)));
}

export async function launch(options, overrides = {}) {
  if (options.locate) {
    const location = await selectStore(options);
    const { socketPath } = await import('./owner.mjs');
    console.log(JSON.stringify({ ...location, socket: await socketPath(location.directory) }));
  } else if (options.backgroundOwner) {
    await (await import('./owner.mjs')).serveOwner(options, overrides);
  } else await startStdio(options, overrides);
}

if (process.argv[1] && import.meta.url === pathToFileURL(await realpath(process.argv[1])).href) {
  const options = JSON.parse(process.argv[2] ?? '{}');
  if (Number(process.versions.node.split('.')[0]) < 22 || (process.versions.node.startsWith('22.') && Number(process.versions.node.split('.')[1]) < 19)) throw new Error('Node >=22.19.0 is required');
  await launch(options).catch(error => { console.error(String(error)); process.stdout.write(`${JSON.stringify({kind:'fatal',message:String(error)})}\n`); process.exitCode = 1; });
}
