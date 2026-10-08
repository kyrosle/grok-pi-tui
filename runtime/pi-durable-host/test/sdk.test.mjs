import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createModels } from '@earendil-works/pi-ai/models';
import { fauxProvider, fauxAssistantMessage, fauxToolCall } from '@earendil-works/pi-ai/providers/faux';
import { Type } from '@earendil-works/pi-ai';
import { awaitWithContext, withCancel } from '@earendil-works/chord/context';
import { defineExtension, defineTask, defineTool, ToolTask, AssistantEntry } from '@earendil-works/pi-durable';
import { context, SDK_VERSION, openCore, selectStore, listStores, parseLocator } from '../core.mjs';
import { createController } from '../host.mjs';
import { serveOwner } from '../owner.mjs';
import net from 'node:net';
import { createInterface } from 'node:readline';
import { getGlobalDispatcher } from 'undici';
import { spawn } from 'node:child_process';

async function fixture() {
  const home = await mkdtemp(join(tmpdir(), 'grok-pi-durable-test-'));
  const cwd = join(home, 'project'); await mkdir(cwd);
  const agentDir = join(home, 'pi'); await mkdir(agentDir);
  const faux = fauxProvider({ models: [{ id: 'test' }] });
  const models = createModels(); models.setProvider(faux.provider);
  const location = await selectStore({ home, cwd });
  return { home, cwd, agentDir, faux, models, location };
}

test('known store manifests remain unchanged and unknown SDK versions are refused', async () => {
  const f = await fixture();
  const path = join(f.location.directory, 'session.json');
  const info = JSON.parse(await readFile(path, 'utf8'));
  for (const sdkVersion of ['1.0.4', SDK_VERSION]) {
    const bytes = JSON.stringify({ ...info, sdkVersion }); await writeFile(path, bytes);
    assert.equal((await selectStore({ ...f, session: `durable:${info.storeId}:1` })).storeId, info.storeId);
    assert.equal(await readFile(path, 'utf8'), bytes);
  }
  await writeFile(path, JSON.stringify({ ...info, sdkVersion: '99.0.0' }));
  await assert.rejects(selectStore({ ...f, session: `durable:${info.storeId}:1` }), /unsupported SDK version/);
  assert.deepEqual(await listStores(f.location.root), []);
});

test('SDK 1.1 records execution times and keeps scan cursors in their declared order', async () => {
  const f = await fixture(); await writeFile(join(f.cwd, 'timed.txt'), 'timed data');
  f.faux.setResponses([
    fauxAssistantMessage(fauxToolCall('read', { path: 'timed.txt' }, { id: 'timed-read' }), { stopReason: 'toolUse' }),
    fauxAssistantMessage('TIMED_REPLY'),
  ]);
  const controller = await createController(f, { models: f.models });
  try {
    const admitted = await controller.request('submit', { text: 'read timed.txt', requestId: 'timing-1' });
    assert.equal((await controller.request('wait', { id: admitted.id })).status, 'done');
    const { core } = controller;
    const snapshot = await core.snapshot();
    const tool = snapshot.entries.find(entry => entry.kind === 'pi.tool-result');
    assert.ok(Number.isFinite(tool.model[0].durationMs) && tool.model[0].durationMs >= 0);
    const record = await core.harness.getTask(tool.byTaskId, context);
    assert.ok(record.startedAt <= record.endedAt);
    const first = await core.conversation.entries({ order: 'ascending' }, 1, undefined, context);
    const last = await core.conversation.entries({ order: 'descending' }, 1, undefined, context);
    assert.ok(first.items[0].id < last.items[0].id);
    assert.ok(first.next);
    const next = await core.conversation.entries({}, 1, first.next, context);
    assert.ok(next.items[0].id > first.items[0].id);
    await assert.rejects(core.conversation.entries({ order: 'descending' }, 1, first.next, context), /ascending.*descending/);
    assert.ok(await core.conversation.context(context, { at: first.items[0].id }));
    assert.ok((await controller.request('sessions')).some(row => row.id === core.locator));
  } finally { await controller.close(); }
});

test('published public SDK admits once, snapshots and reopens the same SQLite', async () => {
  const f = await fixture(); f.faux.setResponses([fauxAssistantMessage('persisted answer')]);
  const core = await openCore(f.location, f, { models: f.models });
  const draft = { type: 'input', content: 'hello', requestId: 'lost-ack' };
  const first = await core.conversation.submit(draft, context);
  const second = await core.conversation.submit(draft, context);
  assert.equal(first.id, second.id);
  assert.equal((await first.wait(context)).status, 'done');
  const locator = core.locator; await core.close();
  const again = await openCore(await selectStore({ ...f, session: locator }), f, { models: f.models });
  const snapshot = await again.snapshot();
  assert.equal(snapshot.entries.filter(e => e.kind === 'pi.user').length, 1);
  assert.ok(JSON.stringify(snapshot.entries).includes('persisted answer'));
  assert.equal((await again.conversation.submit(draft, context)).id, first.id);
  assert.equal((await (await again.harness.submission(first.id, context)).status(context)).status, 'done');
  await again.close();
  assert.equal(parseLocator(locator).storeId, f.location.storeId);
  assert.throws(() => parseLocator('/tmp/classic.jsonl'));
});

test('explicit socket owner continues work after detach and allows reacquisition', { timeout: 10000 }, async () => {
  const f = await fixture(); f.faux.setResponses([async () => { await new Promise(resolve => setTimeout(resolve, 200)); return fauxAssistantMessage('background durable result'); }]);
  const owner = await serveOwner({ ...f, backgroundOwner: true }, { models: f.models });
  const connect = async () => {
    const socket = net.createConnection(owner.path); await new Promise((resolve, reject) => { socket.once('connect', resolve); socket.once('error', reject); });
    const replies = new Map(); let seq = 0;
    createInterface({ input: socket }).on('line', line => { const value = JSON.parse(line); if (value.kind === 'response') { replies.get(value.id)?.(value); replies.delete(value.id); } });
    return { socket, request: async (method, params = {}) => { const id = String(++seq); const promise = new Promise(resolve => replies.set(id, resolve)); socket.write(`${JSON.stringify({ id, method, params })}\n`); const value = await promise; if (value.error) throw new Error(value.error); return value.result; } };
  };
  let client = await connect(); await client.request('watch');
  const submission = await client.request('submit', { text: 'background work', requestId: 'background-1' });
  assert.equal((await client.request('status', { id: submission.id })).status, 'placed');
  await client.request('close'); client.socket.destroy();
  await new Promise(resolve => setTimeout(resolve, 50));
  client = await connect();
  assert.equal((await client.request('wait', { id: submission.id })).status, 'done');
  assert.equal((await client.request('status', { id: submission.id })).status, 'done');
  await client.request('close'); client.socket.destroy(); await owner.stop();
});

test('controller routes public operations and watches only committed state', { timeout: 10000 }, async () => {
  const f = await fixture(); f.faux.setResponses([fauxAssistantMessage('controller answer')]);
  const frames = [];
  const controller = await createController(f, { models: f.models }, frame => frames.push(frame));
  assert.equal((await controller.request('hello')).protocol, 'grok-pi-durable/1');
  await controller.request('watch');
  const result = await controller.request('submit', { text: 'controller', requestId: 'controller-1' });
  assert.equal((await controller.request('wait', { id: result.id })).status, 'done');
  assert.equal((await controller.request('status', { id: result.id })).status, 'done');
  assert.ok(frames.some(frame => frame.event.type === 'snapshot'));
  assert.ok(frames.some(frame => frame.event.type === 'task_graph'));
  const old = (await controller.request('hello')).session;
  const fresh = (await controller.request('new')).session;
  assert.notEqual(old, fresh);
  assert.equal((await controller.request('conversations')).length, 2);
  assert.ok((await controller.request('sessions')).length);
  await controller.request('attach', { session: old });
  assert.equal((await controller.request('hello')).session, old);
  await assert.rejects(controller.request('codemode'), /Unsupported/);
  await controller.close();
});

test('failed attaches keep the watched core; background owners stay bound to one store', async () => {
  const f = await fixture(); f.faux.setResponses([fauxAssistantMessage('after rejected attach')]);
  const frames = [];
  const controller = await createController(f, { models: f.models }, frame => frames.push(frame));
  try {
    await controller.request('watch');
    const locator = controller.core.locator;
    await assert.rejects(controller.request('attach', { session: `durable:${controller.core.location.storeId}:99999` }), /Unknown/);
    await assert.rejects(controller.request('attach', { session: 'durable:00000000-0000-0000-0000-000000000000:0' }), /ENOENT/);
    const dispatcher = getGlobalDispatcher();
    const invalidTarget = await selectStore(f);
    await assert.rejects(controller.request('attach', { session: `durable:${invalidTarget.storeId}:99999` }), /does not exist/);
    assert.equal(getGlobalDispatcher(), dispatcher, 'failed target must preserve active-core HTTP transport');
    assert.equal(controller.core.locator, locator);
    const admitted = await controller.request('submit', { text: 'still watched', requestId: 'after-attach' });
    await controller.request('wait', { id: admitted.id });
    assert.ok(frames.some(frame => JSON.stringify(frame).includes('after rejected attach')));
  } finally { await controller.close(); }
  const target = await selectStore(f);
  const owner = await serveOwner({ ...f, backgroundOwner: true }, { models: f.models });
  try {
    await assert.rejects(owner.controller.request('attach', { session: `durable:${target.storeId}:0` }), /bound to one store/);
    assert.notEqual(owner.controller.core.location.storeId, target.storeId);
  } finally { await owner.stop(); }
});

test('tool exclusions and no-tools apply when reopening persisted conversations', async () => {
  const f = await fixture();
  let core = await openCore(f.location, f, { models: f.models });
  const locator = core.locator; await core.close();
  core = await openCore(await selectStore({ ...f, session: locator }), { ...f, excludeTools: 'bash' }, { models: f.models });
  assert.ok(!(await core.snapshot()).agent.tools.includes('bash'));
  assert.ok((await core.snapshot()).agent.tools.includes('subagent'));
  await core.close();
  core = await openCore(await selectStore({ ...f, session: locator }), { ...f, noTools: true }, { models: f.models });
  assert.deepEqual((await core.snapshot()).agent.tools, []);
  await core.close();
});

test('abort recovery marks all conversations before resuming the store', { timeout: 10000 }, async () => {
  const f = await fixture(); let reached; let calls = 0;
  const started = new Promise(resolve => { reached = resolve; });
  const tool = defineTool({ name: 'effect', parameters: Type.Object({}), description: 'effect', replay: 'unsafe', execute: async (_, api, ctx) => { calls++; reached(); return awaitWithContext(new Promise(() => {}), ctx); } });
  const overrides = { models: f.models, extensions: [defineExtension({ name: 'effect', tools: [tool] })] };
  const core = await openCore(f.location, f, overrides);
  const assistant = await core.conversation.commit(tx => tx.appendEntry(AssistantEntry, core.conversation.id, { model: [fauxAssistantMessage(fauxToolCall('effect', {}, { id: 'abort-effect' }))] }), context);
  const id = await core.conversation.commit(tx => tx.createTask(ToolTask, { assistant: assistant.id, callId: 'abort-effect' }, { ownership: { kind: 'conversation' } }), context);
  core.resume(); await started;
  const other = await core.harness.createConversation({ ownership: { kind: 'ownerless' } }, context);
  const session = `durable:${core.location.storeId}:${other.id}`;
  await core.close();
  const controller = await createController({ ...f, session }, overrides);
  try {
    assert.equal(controller.core.recovery.length, 1);
    await controller.request('recover', { action: 'abort' });
    assert.equal((await controller.core.harness.getTask(id, context)).state.outcome.status, 'aborted');
    assert.equal(controller.core.recovery.length, 0);
    assert.equal(calls, 1);
  } finally { await controller.close(); }
});

test('checkpoint survives close/reopen without replaying committed ticks', async () => {
  const f = await fixture(); let signal;
  const atTwo = new Promise(resolve => { signal = resolve; }); const ticks = [];
  const ticker = defineTask({ name: 'test.ticker', version: 1, initial: () => ({ phase: 'tick', n: 1 }), phases: {
    async tick(task, runtime, ctx) {
      const n = task.state.checkpoint.n;
      if (await runtime.memo(`tick-${n}`, ctx) === undefined) { await runtime.memo(`tick-${n}`, true, ctx); ticks.push(n); }
      await runtime.commit(() => n === 4 ? { status: 'terminal', outcome: { status: 'completed', result: n } } : { status: 'running', checkpoint: { phase: 'tick', n: n + 1 } }, ctx);
      if (n === 2) signal();
      await runtime.sleep(Date.now() + 100, ctx);
    },
  }, abort: async (_, runtime, ctx) => runtime.commit(() => ({ status: 'terminal', outcome: { status: 'aborted' } }), ctx) });
  const extension = defineExtension({ name: 'test', tasks: [ticker] });
  let core = await openCore(f.location, f, { models: f.models, extensions: [extension] });
  const id = await core.conversation.commit(tx => tx.createTask(ticker, {}, { ownership: { kind: 'conversation' } }), context);
  core.resume(); await atTwo; const locator = core.locator; await core.close();
  core = await openCore(await selectStore({ ...f, session: locator }), f, { models: f.models, extensions: [extension] });
  assert.equal((await core.harness.waitForTask(id, context)).state.outcome.result, 4);
  assert.deepEqual(ticks, [1, 2, 3, 4]); await core.close();
});

test('cancelling a waiter preserves work; SDK-owned child returns its answer', { timeout: 10000 }, async () => {
  const f = await fixture();
  f.faux.setResponses([async () => { await new Promise(resolve => setTimeout(resolve, 100)); return fauxAssistantMessage('still runs'); }]);
  const core = await openCore(f.location, f, { models: f.models });
  const submission = await core.conversation.submit({ type: 'input', content: 'wait cancellation', requestId: 'wait-1' }, context);
  const observer = withCancel(context); const waiting = submission.wait(observer.context); observer.cancel();
  await assert.rejects(waiting); assert.equal((await submission.wait(context)).status, 'done');
  f.faux.setResponses([fauxAssistantMessage(fauxToolCall('subagent', { task: 'child task' }), { stopReason: 'toolUse' }), fauxAssistantMessage('CHILD_ANSWER'), fauxAssistantMessage('PARENT_ANSWER')]);
  const delegated = await core.conversation.submit({ type: 'input', content: 'delegate', requestId: 'child-1' }, context);
  assert.equal((await delegated.wait(context)).status, 'done');
  const children = (await core.harness.commit(tx => tx.scanConversations({}, 10), context)).items.filter(c => c.owner);
  assert.equal(children.length, 1); assert.ok(children[0].owner.taskId);
  const child = await core.harness.conversation(children[0].id, context);
  const state = await child.viewState(context); assert.match(JSON.stringify(state.value.entries), /CHILD_ANSWER/); state.dispose(); await core.close();
});

test('unsafe recovered tool is gated; official SDK records interrupted without rerunning', { timeout: 10000 }, async () => {
  const f = await fixture(); let reached; let calls = 0;
  const started = new Promise(resolve => { reached = resolve; });
  const tool = defineTool({ name: 'effect', parameters: Type.Object({}), description: 'test effect', replay: 'unsafe', execute: async (_, api, ctx) => { calls++; reached(); return awaitWithContext(new Promise(() => {}), ctx); } });
  const ext = defineExtension({ name: 'effect', tools: [tool] });
  let core = await openCore(f.location, f, { models: f.models, extensions: [ext] });
  const assistant = await core.conversation.commit(tx => tx.appendEntry(AssistantEntry, core.conversation.id, { model: [fauxAssistantMessage(fauxToolCall('effect', {}, { id: 'effect-1' }))] }), context);
  const taskId = await core.conversation.commit(tx => tx.createTask(ToolTask, { assistant: assistant.id, callId: 'effect-1' }, { ownership: { kind: 'conversation' } }), context);
  core.resume(); await started; const locator = core.locator; await core.close();
  core = await openCore(await selectStore({ ...f, session: locator }), f, { models: f.models, extensions: [ext] });
  assert.equal(core.recovery.length, 1); assert.throws(() => core.resume(), /Recovery decision/);
  core.resume(true); await core.harness.waitForTask(taskId, context);
  assert.equal(calls, 1);
  assert.match(JSON.stringify((await core.snapshot()).entries), /interrupted/);
  await core.close();
});

for (const policy of ['safe', 'unsafe']) test(`stored safe tool only replays when current policy is ${policy}`, { timeout: 10000 }, async () => {
  const f = await fixture(); let reached; const started = new Promise(resolve => { reached = resolve; }); let calls = 0;
  const tool = replay => defineTool({ name: 'replay', description: 'replay boundary', parameters: Type.Object({}), replay, async execute(_, api, ctx) {
    calls++; if (calls === 1) { reached(); return awaitWithContext(new Promise(() => {}), ctx); }
    return { content: [{ type: 'text', text: 'safe resumed' }] };
  } });
  const extension = replay => defineExtension({ name: 'replay', tools: [tool(replay)] });
  let core = await openCore(f.location, f, { models: f.models, extensions: [extension('safe')] });
  const assistant = await core.conversation.commit(tx => tx.appendEntry(AssistantEntry, core.conversation.id, { model: [fauxAssistantMessage(fauxToolCall('replay', {}, { id: 'replay-1' }))] }), context);
  const id = await core.conversation.commit(tx => tx.createTask(ToolTask, { assistant: assistant.id, callId: 'replay-1' }, { ownership: { kind: 'conversation' } }), context);
  core.resume(); await started; const locator = core.locator; await core.close();
  core = await openCore(await selectStore({ ...f, session: locator }), f, { models: f.models, extensions: [extension(policy)] });
  assert.equal(core.recovery.length, policy === 'safe' ? 0 : 1);
  core.resume(true); await core.harness.waitForTask(id, context);
  assert.equal(calls, policy === 'safe' ? 2 : 1); await core.close();
});

test('SIGKILL after an external effect keeps intent and does not duplicate the unsafe effect', { timeout: 20000 }, async () => {
  const f = await fixture();
  const child = spawn(process.execPath, [new URL('./crash-fixture.mjs', import.meta.url).pathname, JSON.stringify({ home: f.home, cwd: f.cwd, agentDir: f.agentDir })], { stdio: 'ignore' });
  let marker;
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) { marker = await readFile(join(f.home, 'effects.txt'), 'utf8').catch(() => ''); if (marker) break; await new Promise(resolve => setTimeout(resolve, 25)); }
  assert.equal(marker, 'effect\n');
  const exited = new Promise(resolve => child.once('exit', (code, signal) => resolve({ code, signal }))); child.kill('SIGKILL');
  assert.equal((await exited).signal, 'SIGKILL');
  f.faux.setResponses([fauxAssistantMessage('recovered answer')]);
  const effect = defineTool({ name: 'effect', description: 'effect', parameters: Type.Object({}), replay: 'unsafe', async execute() { throw new Error('unsafe effect replayed'); } });
  const core = await openCore(await selectStore({ ...f, continue: true }), f, { models: f.models, extensions: [defineExtension({ name: 'effect', tools: [effect] })] });
  assert.equal(core.recovery.length, 1); core.resume(true);
  const record = await core.harness.commit(tx => tx.submissionByRequest(core.conversation.id, 'crash-1'), context);
  assert.equal((await (await core.harness.submission(record.id, context)).wait(context)).status, 'done');
  assert.equal(await readFile(join(f.home, 'effects.txt'), 'utf8'), 'effect\n'); await core.close();
});
