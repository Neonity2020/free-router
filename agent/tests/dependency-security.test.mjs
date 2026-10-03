import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';

test('Pi minimatch resolves the installed security-fixed brace-expansion', async () => {
  const root = new URL('../node_modules/@earendil-works/pi-coding-agent/node_modules/', import.meta.url);
  const metadata = JSON.parse(await readFile(new URL('brace-expansion/package.json', root), 'utf8'));
  assert.equal(metadata.version, '5.0.12');
  const require = createRequire(new URL('minimatch/package.json', root));
  assert.match(require.resolve('brace-expansion'), /pi-coding-agent.*brace-expansion/);
  assert.equal(require('brace-expansion').expand('{a,b}').join(','), 'a,b');
});
