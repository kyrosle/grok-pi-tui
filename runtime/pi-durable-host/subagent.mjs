// Pi's documented owned-conversation pattern, adapted from experimental/durable/subagent.ts.
// Copyright (c) 2025 Mario Zechner. MIT; see licenses/pi-MIT.txt.
import { Type } from '@earendil-works/pi-ai';
import { AssistantEntry, configure, defineExtension, defineTool } from '@earendil-works/pi-durable';

export const Subagent = defineExtension({ name: 'subagent', tools: [defineTool({
  name: 'subagent', description: 'Delegate a self-contained task to a foreground child conversation and return its answer.',
  parameters: Type.Object({ task: Type.String() }), replay: 'safe',
  async execute(args, api, context) {
    const child = await api.commit(async tx => {
      const existing = (await tx.scanConversations({ ownerTaskId: api.taskId }, 1)).items[0];
      if (existing) return existing.id;
      const created = await tx.createConversation({ ownership: { kind: 'task', taskId: api.taskId } });
      await configure(tx, created.id, { extensions: { remove: [Subagent] } });
      return created.id;
    }, context);
    await api.details({ conversationId: child }, context);
    const conversation = await api.conversation(child, context);
    const settled = await (await conversation.submit({ type: 'input', content: args.task, requestId: `subagent:${api.taskId}` }, context)).wait(context);
    if (settled.status !== 'done' || settled.type !== 'input') throw new Error(`Subagent ${child}: ${settled.status}`);
    const answer = await api.commit(tx => tx.entry(AssistantEntry, settled.answer), context);
    return { content: [{ type: 'text', text: answer?.model?.[0]?.content?.filter(block => block.type === 'text').map(block => block.text).join('') ?? '' }], details: { conversationId: child } };
  },
})] });
