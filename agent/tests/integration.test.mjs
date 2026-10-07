import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

test('Web API executes the official Pi coding tools through the Rust gateway', async (t) => {
  const root = resolve(import.meta.dirname, '../..');
  // Fail fast with an actionable message: spawning a missing binary otherwise
  // surfaces as an opaque native crash instead of a test assertion.
  const binary = resolve(root, 'backend/target/debug/free-router');
  assert.ok(existsSync(binary), `Gateway binary not found: ${binary}\nBuild it first: cargo build --manifest-path backend/Cargo.toml`);
  const directory = await mkdtemp(resolve(tmpdir(), 'free-router-agent-'));
  const requests = [];
  const mock = createServer(async (req, res) => {
    let raw = '';
    for await (const chunk of req) raw += chunk;
    if (req.url === '/search') {
      assert.equal(req.headers['x-api-key'], 'mock-exa-key');
      const search = JSON.parse(raw);
      assert.equal(search.query, 'Pi Agent documentation');
      assert.equal(search.type, 'auto');
      res.writeHead(200, { 'content-type': 'application/json' });
      return res.end(JSON.stringify({ results: [{ title: 'Pi Docs', url: 'https://pi.dev/docs', highlights: ['SDK documentation'] }] }));
    }
    const body = JSON.parse(raw); requests.push(body);
    assert.equal(req.headers.authorization, 'Bearer mock-upstream');
    // Command Code still maps its Space Bunny alias; the OpenRouter route now
    // forwards the replacement model id verbatim.
    assert.ok(['stealth/space-bunny-alpha', 'openrouter/free'].includes(body.model), body.model);
    const cancel = body.messages.some(m => m.role === 'user' && m.content === 'cancel');
    const step = body.messages.filter(m => m.role === 'tool').length;
    const actions = [
      ['write', { path: 'hello.js', content: "console.log('before');\n" }],
      ['edit', { path: 'hello.js', oldText: 'before', newText: 'after' }],
      ['bash', { command: 'node hello.js' }],
      ['read', { path: 'hello.js' }],
      ['web_search', { query: 'Pi Agent documentation', num_results: 1 }],
    ];
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    const send = (delta, finish_reason = null) => res.write(`data: ${JSON.stringify({
      id: 'mock', object: 'chat.completion.chunk', model: body.model,
      choices: [{ index: 0, delta, finish_reason }],
    })}\n\n`);
    if (cancel || step < actions.length) {
      const [name, args] = cancel ? ['bash', { command: 'sleep 10' }] : actions[step];
      send({ role: 'assistant', tool_calls: [{ index: 0, id: `tool-${step}`, type: 'function', function: { name, arguments: JSON.stringify(args) } }] });
      send({}, 'tool_calls');
    } else {
      send({ role: 'assistant', content: '代码已修改并验证。' }); send({}, 'stop');
    }
    res.end('data: [DONE]\n\n');
  });
  mock.listen(0, '127.0.0.1'); await once(mock, 'listening');
  const finder = createServer(); finder.listen(0, '127.0.0.1'); await once(finder, 'listening');
  const port = finder.address().port; await new Promise(r => finder.close(r));
  const proc = spawn(binary, {
    cwd: root, stdio: ['ignore', 'pipe', 'pipe'], env: {
      ...process.env, HOST: '127.0.0.1', PORT: String(port), PI_AGENT_WORKSPACE: directory,
      SETTINGS_FILE: resolve(directory, 'settings.json'), GATEWAY_API_KEY: 'mock-management',
      DEFAULT_PROVIDER: 'openrouter', OPENROUTER_API_KEY: 'mock-upstream', OPENCODE_API_KEY: '',
      COMMANDCODE_API_KEY: 'mock-upstream', COMMANDCODE_BASE_URL: `http://127.0.0.1:${mock.address().port}/provider/v1`,
      OPENROUTER_BASE_URL: `http://127.0.0.1:${mock.address().port}/v1`,
      EXA_BASE_URL: `http://127.0.0.1:${mock.address().port}`, EXA_API_KEY: '',
    },
  });
  let errors = ''; proc.stderr.on('data', chunk => errors += chunk);
  t.after(async () => {
    proc.kill('SIGINT');
    if (proc.exitCode === null) await once(proc, 'exit');
    mock.closeAllConnections(); await new Promise(r => mock.close(r));
    await rm(directory, { recursive: true, force: true });
  });
  async function api(path, method = 'GET', body, key = 'mock-management', header = true) {
    const response = await fetch(`http://127.0.0.1:${port}/api/agent/${path}`, {
      method, headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${key}`,
        ...(header ? { 'X-Gateway-Settings': '1' } : {}) },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    return { status: response.status, data: await response.json() };
  }
  for (let i = 0; i < 100; i++) {
    try { await fetch(`http://127.0.0.1:${port}/api/status`); break; }
    catch { await new Promise(r => setTimeout(r, 50)); }
  }
  assert.equal((await api('status', 'GET', undefined, 'wrong')).status, 401);
  assert.equal((await api('status', 'GET', undefined, 'mock-management', false)).status, 403);
  assert.equal((await api('pick-directory', 'POST', {}, 'wrong')).status, 401);
  assert.equal((await api('pick-directory', 'POST', {}, 'mock-management', false)).status, 403);
  assert.equal((await api('pick-directory')).status, 405);
  const settings = await fetch(`http://127.0.0.1:${port}/api/settings`, {
    method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Gateway-Settings': '1', Authorization: 'Bearer mock-management' },
    body: JSON.stringify({ exa: 'mock-exa-key' }),
  });
  assert.equal(settings.status, 200);
  const publicStatus = await (await fetch(`http://127.0.0.1:${port}/api/status`)).json();
  assert.equal(publicStatus.exa_configured, true);
  assert.ok(!JSON.stringify(publicStatus).includes('mock-exa-key'));
  assert.equal(JSON.parse(await readFile(resolve(directory, 'settings.json'), 'utf8')).exa, 'mock-exa-key');
  const status = await api('status'); assert.equal(status.status, 200, JSON.stringify(status.data) + errors);
  assert.equal(status.data.default_cwd, directory);
  assert.equal((await api('sessions', 'POST', { cwd: 'relative', model: 'space-bunny' })).status, 400);
  const created = await api('sessions', 'POST', { cwd: directory, model: 'commandcode/space-bunny' });
  assert.equal(created.status, 201, JSON.stringify(created.data) + errors);
  const id = created.data.id;
  assert.deepEqual(created.data.tools.sort(), ['bash', 'edit', 'find', 'grep', 'ls', 'read', 'web_search', 'write']);
  assert.equal((await api(`sessions/${id}/prompt`, 'POST', { prompt: 'write and test code' })).status, 202);
  assert.equal((await api(`sessions/${id}/prompt`, 'POST', { prompt: 'overlap' })).status, 409);
  let final;
  for (let i = 0; i < 400; i++) {
    final = (await api(`sessions/${id}`)).data;
    if (!final.busy) break;
    await new Promise(r => setTimeout(r, 50));
  }
  assert.equal(final.busy, false, JSON.stringify(final) + errors);
  assert.equal(final.items.find(i => i.type === 'notice' && i.error), undefined, JSON.stringify(final) + errors);
  assert.equal(await readFile(resolve(directory, 'hello.js'), 'utf8'), "console.log('after');\n");
  assert.deepEqual(final.items.filter(i => i.type === 'tool').map(i => [i.name, i.status]), [
    ['write', 'done'], ['edit', 'done'], ['bash', 'done'], ['read', 'done'],
    ['web_search', 'done'],
  ]);
  assert.match(final.items.find(i => i.name === 'bash').text, /after/);
  assert.match(final.items.find(i => i.name === 'web_search').text, /https:\/\/pi.dev\/docs/);
  assert.ok(!JSON.stringify(final).includes('mock-exa-key'));
  assert.equal(final.items.at(-1).text, '代码已修改并验证。');
  assert.ok(requests.at(-1).messages.some(m => m.role === 'tool' && String(m.content).includes('after')));
  async function saveExa(value) {
    return fetch(`http://127.0.0.1:${port}/api/settings`, {
      method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Gateway-Settings': '1', Authorization: 'Bearer mock-management' },
      body: JSON.stringify(value),
    });
  }
  assert.equal((await saveExa({ exa: '' })).status, 400);
  assert.equal((await saveExa({ default_provider: 'openrouter' })).status, 200);
  assert.equal(JSON.parse(await readFile(resolve(directory, 'settings.json'), 'utf8')).exa, 'mock-exa-key');
  assert.equal((await saveExa({ exa: null })).status, 200);
  assert.equal((await (await fetch(`http://127.0.0.1:${port}/api/status`)).json()).exa_configured, false);
  const cleared = JSON.parse(await readFile(resolve(directory, 'settings.json'), 'utf8'));
  assert.equal(cleared.exa, '');
  assert.deepEqual(cleared.openrouter, ['mock-upstream']);
  const second = (await api('sessions', 'POST', { cwd: directory, model: 'openrouter/free' })).data;
  await api(`sessions/${second.id}/prompt`, 'POST', { prompt: 'cancel' });
  for (let i = 0; i < 100; i++) {
    const snapshot = (await api(`sessions/${second.id}`)).data;
    if (snapshot.items.some(i => i.type === 'tool' && i.status === 'running')) break;
    await new Promise(r => setTimeout(r, 50));
  }
  const started = Date.now();
  const stopped = await api(`sessions/${second.id}/abort`, 'POST', {});
  assert.equal(stopped.status, 200);
  assert.ok(Date.now() - started < 3000, 'abort must stop a running bash command');
  assert.equal((await api('sessions')).data.sessions.length, 2);
  assert.equal((await api(`sessions/${id}`, 'DELETE')).status, 200);
  assert.equal((await api(`sessions/${id}`)).status, 404);
  assert.equal(await readFile(resolve(directory, 'hello.js'), 'utf8'), "console.log('after');\n");
});
