import test from 'node:test';
import assert from 'node:assert/strict';
import { createWebSearchTool } from '../web-search.mjs';

test('search validates configuration, reports safe errors and honors cancellation', async () => {
  const config = { key: '', baseUrl: 'https://api.exa.ai' };
  let requests = 0;
  const tool = createWebSearchTool(() => config, async (_url, options) => {
    requests++;
    assert.equal(options.headers['x-api-key'], 'secret');
    return new Response('secret internal upstream diagnostic', { status: 401 });
  });
  await assert.rejects(tool.execute('a', { query: 'test' }), /Settings/);
  assert.equal(requests, 0);
  config.key = 'secret';
  await assert.rejects(tool.execute('a', { query: 'test', num_results: 20 }), /参数无效/);
  await assert.rejects(tool.execute('a', { query: 'test' }), error => /401/.test(error.message) && !error.message.includes('secret'));
  const controller = new AbortController(); controller.abort();
  const cancelled = createWebSearchTool(() => config, async (_url, options) => { options.signal.throwIfAborted(); });
  await assert.rejects(cancelled.execute('a', { query: 'test' }, controller.signal), /搜索已停止/);
});

test('search returns bounded excerpts and source URLs, and picks up changed keys', async () => {
  const config = { key: 'one', baseUrl: 'https://api.exa.ai' };
  const keys = [];
  const tool = createWebSearchTool(() => config, async (url, options) => {
    assert.equal(url, 'https://api.exa.ai/search'); keys.push(options.headers['x-api-key']);
    return Response.json({ results: [{ title: 'Docs', url: 'https://example.com/docs', highlights: ['x'.repeat(3000)] }] });
  });
  const first = await tool.execute('a', { query: ' test ' });
  const result = JSON.parse(first.content[0].text);
  assert.equal(result.query, 'test');
  assert.equal(result.results[0].excerpt.length, 1500);
  assert.equal(result.results[0].url, 'https://example.com/docs');
  config.key = 'two'; await tool.execute('b', { query: 'test' });
  assert.deepEqual(keys, ['one', 'two']);
});
