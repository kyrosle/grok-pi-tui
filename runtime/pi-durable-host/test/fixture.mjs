// No network or credentials: exercises the real Harness with its public faux provider.
import { createModels } from '@earendil-works/pi-ai/models';
import { fauxProvider, fauxAssistantMessage, fauxToolCall } from '@earendil-works/pi-ai/providers/faux';
import { launch } from '../host.mjs';
const faux = fauxProvider({ models: [{ id: 'test', name: 'Durable fixture', reasoning: true }], tokensPerSecond: 100 });
const models = createModels(); models.setProvider(faux.provider);
const reply = input => {
  const last = input.messages.findLast(message => message.role === 'user' || message.role === 'toolResult');
  if (last?.role === 'toolResult') return fauxAssistantMessage('DURABLE_TOOL_DONE');
  const content = typeof last?.content === 'string' ? last.content : (last?.content ?? []).filter(b => b.type === 'text').map(b => b.text).join('');
  if (content.includes('read fixture')) return fauxAssistantMessage(fauxToolCall('read', { path: 'fixture.txt' }, { id: 'read-fixture' }), { stopReason: 'toolUse' });
  return fauxAssistantMessage(`DURABLE_REPLY ${content}`);
};
faux.setResponses(Array.from({ length: 128 }, () => reply));
await launch(JSON.parse(process.argv[2]), { models });
