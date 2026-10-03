const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { install } = require('../install-cli.cjs');
const binary = process.platform === 'win32' ? 'free-router.exe' : 'free-router';

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'free-router-install-test-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const project = path.join(root, 'project');
  const directory = path.join(root, 'bin');
  const files = [
    [`backend/target/release/${binary}`, 'new binary'],
    ['frontend/dist/index.html', '<html>installed</html>'],
    ...['server.mjs', 'tool-events.mjs', 'web-search.mjs', 'package.json', 'package-lock.json'].map(name => [`agent/${name}`, '{}']),
    ['agent/node_modules/@earendil-works/pi-coding-agent/index.js', 'SDK'],
    ['settings.local.json', 'secret that must not ship'],
  ];
  for (const [name, value] of files) {
    const file = path.join(project, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, value);
  }
  fs.mkdirSync(directory);
  return { root, project, directory };
}

test('installation contains runtime resources and survives source deletion', t => {
  const { project, directory } = fixture(t);
  const installed = install(project, directory);
  fs.rmSync(project, { recursive: true });
  assert.equal(fs.readFileSync(installed, 'utf8'), 'new binary');
  const resources = path.join(directory, 'free-router-resources');
  assert.equal(fs.readFileSync(path.join(resources, 'frontend/dist/index.html'), 'utf8'), '<html>installed</html>');
  assert.ok(fs.existsSync(path.join(resources, 'agent/node_modules/@earendil-works/pi-coding-agent/index.js')));
  assert.ok(!fs.existsSync(path.join(resources, 'settings.local.json')));
  assert.deepEqual(fs.readdirSync(directory).sort(), [binary, 'free-router-resources'].sort());
});

test('failed resource copy preserves the old binary and resources', t => {
  const { project, directory } = fixture(t);
  install(project, directory);
  fs.writeFileSync(path.join(directory, binary), 'old binary');
  fs.unlinkSync(path.join(project, 'agent/web-search.mjs'));
  assert.throws(() => install(project, directory));
  assert.equal(fs.readFileSync(path.join(directory, binary), 'utf8'), 'old binary');
  assert.ok(fs.existsSync(path.join(directory, 'free-router-resources/agent/web-search.mjs')));
  assert.ok(fs.readdirSync(directory).every(name => !name.startsWith('.free-router-install-')));
});

test('installation replaces an old binary symlink without overwriting its target', { skip: process.platform === 'win32' }, t => {
  const { root, project, directory } = fixture(t);
  const original = path.join(root, 'original');
  fs.writeFileSync(original, 'original binary');
  fs.symlinkSync(original, path.join(directory, binary));
  const installed = install(project, directory);
  assert.equal(fs.readFileSync(original, 'utf8'), 'original binary');
  assert.equal(fs.readFileSync(installed, 'utf8'), 'new binary');
  assert.ok(!fs.lstatSync(installed).isSymbolicLink());
  assert.equal(fs.statSync(installed).mode & 0o777, 0o755);
});


test('failed binary replacement rolls resources back to the previous version', t => {
  const { project, directory } = fixture(t);
  install(project, directory);
  const resources = path.join(directory, 'free-router-resources');
  fs.writeFileSync(path.join(resources, 'frontend/dist/index.html'), 'old UI');
  fs.unlinkSync(path.join(directory, binary));
  fs.mkdirSync(path.join(directory, binary)); // Force rename to fail after resources are swapped.
  assert.throws(() => install(project, directory));
  assert.equal(fs.readFileSync(path.join(resources, 'frontend/dist/index.html'), 'utf8'), 'old UI');
  assert.ok(fs.statSync(path.join(directory, binary)).isDirectory());
});


test('relative npm bin symlinks continue working after source deletion', { skip: process.platform === 'win32' }, t => {
  const { project, directory } = fixture(t);
  const npmBin = path.join(project, 'agent/node_modules/.bin');
  fs.mkdirSync(npmBin);
  fs.symlinkSync('../@earendil-works/pi-coding-agent/index.js', path.join(npmBin, 'pi'));
  install(project, directory);
  fs.rmSync(project, { recursive: true });
  const installedBin = path.join(directory, 'free-router-resources/agent/node_modules/.bin/pi');
  assert.equal(fs.readFileSync(installedBin, 'utf8'), 'SDK');
  assert.equal(fs.readlinkSync(installedBin), '../@earendil-works/pi-coding-agent/index.js');
});
