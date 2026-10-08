import { appendFile } from 'node:fs/promises';
import { join } from 'node:path';
import { createModels } from '@earendil-works/pi-ai/models';
import { fauxProvider, fauxAssistantMessage, fauxToolCall } from '@earendil-works/pi-ai/providers/faux';
import { Type } from '@earendil-works/pi-ai';
import { awaitWithContext } from '@earendil-works/chord/context';
import { defineExtension, defineTool } from '@earendil-works/pi-durable';
import { context, openCore, selectStore } from '../core.mjs';
const options = JSON.parse(process.argv[2]);
setInterval(() => {}, 1000); // The parent deliberately SIGKILLs this owned fixture.
const faux = fauxProvider({ models: [{ id: 'test' }] }); const models = createModels(); models.setProvider(faux.provider);
const effect = defineTool({ name: 'effect', description: 'crash fixture', parameters: Type.Object({}), replay: 'unsafe', async execute(_, api, ctx) { await appendFile(join(options.home, 'effects.txt'), 'effect\n'); return awaitWithContext(new Promise(() => {}), ctx); } });
faux.setResponses([fauxAssistantMessage(fauxToolCall('effect', {}), { stopReason: 'toolUse' })]);
const core = await openCore(await selectStore(options), options, { models, extensions: [defineExtension({ name: 'effect', tools: [effect] })] });
await core.conversation.submit({ type: 'input', content: 'crash boundary', requestId: 'crash-1' }, context);
core.resume();
