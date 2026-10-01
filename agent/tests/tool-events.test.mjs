import test from 'node:test';
import assert from 'node:assert/strict';
import { recordToolEvent } from '../tool-events.mjs';

test('replayed events update one call and late events cannot overwrite its final output', () => {
  const state = { items: [], toolItems: new Map() };
  const record = event => recordToolEvent(state, { toolCallId: 'a', toolName: 'bash', ...event },
    (s, item) => { s.items.push(item); return item; }, JSON.stringify, result => result.text);
  record({ type: 'tool_execution_start', args: { command: 'pwd' } });
  record({ type: 'tool_execution_update', partialResult: { text: 'part' } });
  record({ type: 'tool_execution_start', args: { command: 'pwd' } });
  assert.equal(state.items[0].text, '"part"');
  record({ type: 'tool_execution_update', partialResult: { text: 'part more' } });
  assert.equal(state.items[0].text, '"part more"');
  record({ type: 'tool_execution_end', result: { text: 'final' } });
  record({ type: 'tool_execution_start' });
  record({ type: 'tool_execution_update', partialResult: { text: 'late' } });
  record({ type: 'tool_execution_end', result: { text: 'late final' } });
  assert.equal(state.items.length, 1);
  assert.equal(state.items[0].text, '"final"');
  assert.equal(state.toolItems.size, 0);
  record({ type: 'tool_execution_end', toolCallId: 'b', isError: true, result: { text: 'failed' } });
  assert.equal(state.items.length, 2);
  assert.equal(state.items[1].status, 'error');
});
