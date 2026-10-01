import test from 'node:test';
import assert from 'node:assert/strict';
import { buildTimeline, compactTools } from '../src/agentTimeline.ts';

const call = (id, extra = {}) => ({ id, type: 'tool', name: 'read', args: '{"path":"a"}', text: 'content', status: 'done', ...extra });
test('empty assistant rounds disappear and real messages preserve tool chronology', () => {
  const entries = buildTimeline([call('1'), { id: 'empty', type: 'assistant', text: '' }, call('2'),
    { id: 'reply', type: 'assistant', text: '解释' }, call('3'),
    { id: 'thinking', type: 'assistant', text: '', thinking: '推理' }]);
  assert.equal(entries.length, 3);
  assert.equal(entries[0].items.length, 2);
  assert.equal(entries[1].item.text, '解释');
  assert.equal(entries[2].items[1].thinking, '推理');
});
test('identical completed calls compact without hiding errors, running calls or changed arguments/output', () => {
  const items = [call('1'), call('2'), call('3', { status: 'error' }), call('4', { status: 'error' }),
    call('5', { args: '{"path":"b"}' }), call('6', { text: 'changed' }),
    call('7', { status: 'running' }), call('8', { status: 'running' })];
  const rows = compactTools(items);
  assert.equal(rows.length, 7);
  assert.equal(rows[0].count, 2);
  assert.equal(items.length, 8);
  assert.equal(rows.filter(row => row.item.status === 'error').length, 2);
});
