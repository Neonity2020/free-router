const test = require('node:test');
const assert = require('node:assert/strict');
const { createServer } = require('node:net');
const { once } = require('node:events');
const { mkdtemp, writeFile, readFile, rm, stat } = require('node:fs/promises');
const { join } = require('node:path');
const { tmpdir } = require('node:os');
const { findPort, importDevelopmentSettings, trustedFrame } = require('../runtime.cjs');

test('occupied gateway ports are skipped without reusing another service', async t => {
  const server = createServer(); server.listen(0, '127.0.0.1'); await once(server, 'listening');
  t.after(() => new Promise(resolve => server.close(resolve)));
  const port = server.address().port;
  assert.ok(await findPort(port) > port);
});
test('initial config import preserves existing desktop keys and restrictive permissions', async t => {
  const directory = await mkdtemp(join(tmpdir(), 'free-router-desktop-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const source = join(directory, 'source'), destination = join(directory, 'data');
  await require('node:fs/promises').mkdir(source);
  await writeFile(join(source, 'settings.local.json'), '{"exa":"test"}');
  await importDevelopmentSettings(source, destination);
  assert.equal(await readFile(join(destination, 'settings.local.json'), 'utf8'), '{"exa":"test"}');
  if (process.platform !== 'win32') assert.equal((await stat(join(destination, 'settings.local.json'))).mode & 0o777, 0o600);
  await writeFile(join(destination, 'settings.local.json'), 'keep');
  await importDevelopmentSettings(source, destination);
  assert.equal(await readFile(join(destination, 'settings.local.json'), 'utf8'), 'keep');
});
test('native IPC only trusts the app main frame at its own gateway origin', () => {
  const frame = { url: 'http://127.0.0.1:8788/#agent' };
  const contents = { mainFrame: frame }, window = { webContents: contents };
  assert.equal(trustedFrame({ sender: contents, senderFrame: frame }, window, 'http://127.0.0.1:8788'), true);
  assert.equal(trustedFrame({ sender: {}, senderFrame: frame }, window, 'http://127.0.0.1:8788'), false);
  assert.equal(trustedFrame({ sender: contents, senderFrame: { url: frame.url } }, window, 'http://127.0.0.1:8788'), false);
  frame.url = 'https://example.com';
  assert.equal(trustedFrame({ sender: contents, senderFrame: frame }, window, 'http://127.0.0.1:8788'), false);
});
