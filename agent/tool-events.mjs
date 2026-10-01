// A call owns one transcript record; partial results are snapshots, not deltas.
export function recordToolEvent(state, event, addItem, limitText, toolText) {
  if (!['tool_execution_start', 'tool_execution_update', 'tool_execution_end'].includes(event.type)) return;
  let item = state.toolItems.get(event.toolCallId) || state.items.find(entry => entry.toolCallId === event.toolCallId);
  if (item && item.status !== 'running') return; // Ignore replay and late partial updates.
  if (!item) {
    item = addItem(state, { type: 'tool', toolCallId: event.toolCallId, name: event.toolName,
      args: event.args === undefined ? '' : limitText(event.args, 8000), text: '', status: 'running' });
    state.toolItems.set(event.toolCallId, item);
  }
  if (event.args !== undefined) item.args = limitText(event.args, 8000);
  if (event.toolName) item.name = event.toolName;
  if (event.type === 'tool_execution_update') item.text = limitText(toolText(event.partialResult));
  if (event.type === 'tool_execution_end') {
    item.text = limitText(toolText(event.result));
    item.status = event.isError ? 'error' : 'done';
    state.toolItems.delete(event.toolCallId);
  }
}
