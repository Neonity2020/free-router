import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { once } from 'node:events';
import { streamGatewayReply, gatewayPiSnippet } from '../src/piGateway.ts';

test('Pi SDK streams through the real Rust gateway', async (t) => {
  const seen = [];
  const upstream = createServer(async (req, res) => {
    let body = '';
    for await (const chunk of req) body += chunk;
    const data = JSON.parse(body);
    seen.push({ data, key: req.headers.authorization });
    if (data.model === 'space-bunny-free') {
      res.writeHead(429, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: { message: 'mock rate limit' } }));
      return;
    }
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    const send = (delta, finish_reason = null, usage) => res.write(`data: ${JSON.stringify({
      id: 'mock', object: 'chat.completion.chunk', model: data.model,
      choices: [{ index: 0, delta, finish_reason }], ...(usage ? { usage } : {}),
    })}\n\n`);
    send({ role: 'assistant', reasoning_content: '思考示例', content: '你好' });
    if (data.messages[0].content === 'cancel') {
      const timer = setTimeout(() => res.end('data: [DONE]\n\n'), 5000);
      res.on('close', () => clearTimeout(timer));
    } else {
      send({ content: '，世界' });
      send({}, data.messages[0].content === 'length' ? 'length' : 'stop', { prompt_tokens: 5, completion_tokens: 4, total_tokens: 9 });
      res.end('data: [DONE]\n\n');
    }
  });
  upstream.listen(0, '127.0.0.1');
  await once(upstream, 'listening');
  const finder = createServer();
  finder.listen(0, '127.0.0.1');
  await once(finder, 'listening');
  const port = finder.address().port;
  await new Promise(r => finder.close(r));
  const directory = await mkdtemp(resolve(tmpdir(), 'free-router-pi-'));
  const root = resolve(import.meta.dirname, '../..');
  const proc = spawn(resolve(root, 'backend/target/debug/free-router'), {
    cwd: root, stdio: 'ignore', env: {
      ...process.env, GATEWAY_KEY_COOLDOWN_SECS: '0', HOST: '127.0.0.1', PORT: String(port),
      SETTINGS_FILE: resolve(directory, 'settings.json'),
      GATEWAY_API_KEY: 'mock-gateway', DEFAULT_PROVIDER: 'opencode',
      OPENROUTER_API_KEY: 'mock-router', OPENCODE_API_KEY: 'mock-zen',
      OPENROUTER_BASE_URL: `http://127.0.0.1:${upstream.address().port}/v1`,
      OPENCODE_BASE_URL: `http://127.0.0.1:${upstream.address().port}/v1`,
      COMMANDCODE_API_KEY: 'mock-command', COMMANDCODE_BASE_URL: `http://127.0.0.1:${upstream.address().port}/provider/v1`,
    },
  });
  t.after(async () => {
    proc.kill();
    if (proc.exitCode === null) await once(proc, 'exit');
    upstream.closeAllConnections();
    await new Promise(r => upstream.close(r));
    await rm(directory, { recursive: true, force: true });
  });
  const baseUrl = `http://127.0.0.1:${port}/v1`;
  let ready = false;
  for (let i = 0; i < 50; i++) {
    try { await fetch(`http://127.0.0.1:${port}/api/status`); ready = true; break; }
    catch { await new Promise(r => setTimeout(r, 100)); }
  }
  assert.ok(ready, 'gateway must start');
  const probe = await fetch(`${baseUrl}/chat/completions`, {
    method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: 'Bearer mock-gateway' },
    body: JSON.stringify({ model: 'commandcode/space-bunny', messages: [{ role: 'user', content: 'hi' }], stream: true,
      max_completion_tokens: 1, thinking: { type: 'disabled' }, enable_thinking: false,
      reasoning_effort: 'none', reasoning: { effort: 'none' } }),
  });
  assert.equal(probe.status, 200);
  await probe.text();
  assert.equal(seen.at(-1).data.max_completion_tokens, 1);
  for (const hint of ['thinking', 'enable_thinking', 'reasoning_effort', 'reasoning']) {
    assert.ok(hint in seen.at(-1).data);
  }
  seen.length = 0;
  const call = (overrides = {}) => streamGatewayReply({
    baseUrl, modelId: 'space-bunny', apiKey: 'mock-gateway', prompt: 'hello',
    signal: new AbortController().signal, onText() {}, onThinking() {}, ...overrides,
  });
  const deltas = [];
  const thinking = [];
  const message = await call({ onText: text => deltas.push(text), onThinking: text => thinking.push(text) });
  assert.equal(message.stopReason, 'stop');
  assert.equal(deltas.at(-1), '你好，世界');
  assert.equal(message.usage.totalTokens, 9);
  assert.equal(thinking.at(-1), '思考示例');
  assert.deepEqual(seen.map(r => r.data.model), ['space-bunny-free', 'stealth/space-bunny-alpha']);
  assert.deepEqual(seen.map(r => r.key), ['Bearer mock-zen', 'Bearer mock-router']);
  assert.ok(seen.every(r => r.data.stream === true && r.data.max_tokens === 4096));
  await call({ modelId: 'openrouter/space-bunny' });
  await call({ modelId: 'commandcode/space-bunny' });
  assert.equal(seen.at(-1).key, 'Bearer mock-command');
  assert.equal(seen.at(-1).data.model, 'stealth/space-bunny-alpha');
  assert.equal((await call({ modelId: 'openrouter/space-bunny', prompt: 'length' })).stopReason, 'length');
  await assert.rejects(call({ modelId: 'opencode/space-bunny' }), /mock rate limit/);
  await assert.rejects(call({ apiKey: 'wrong-key' }), /401|[Uu]nauthorized|[Ii]nvalid/);
  await assert.rejects(call({ modelId: 'unknown' }), /未知/);
  const controller = new AbortController();
  const cancelled = await call({ modelId: 'openrouter/space-bunny', prompt: 'cancel',
    signal: controller.signal, onText: () => controller.abort() });
  assert.equal(cancelled.stopReason, 'aborted');
  const example = spawn(process.execPath, ['--input-type=module', '-e', gatewayPiSnippet(baseUrl, 'openrouter/space-bunny')], {
    cwd: resolve(root, 'frontend'), env: { ...process.env, GATEWAY_KEY_COOLDOWN_SECS: '0', GATEWAY_API_KEY: 'mock-gateway' },
  });
  let output = '', errors = '';
  example.stdout.on('data', chunk => output += chunk);
  example.stderr.on('data', chunk => errors += chunk);
  const [code] = await once(example, 'exit');
  assert.equal(code, 0, errors);
  assert.equal(output, '你好，世界');
});
