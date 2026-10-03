// Pi ships an npm-shrinkwrap that npm re-applies while unpacking, even when the
// application lock records the fixed release. Use the integrity-checked direct
// dependency as the runtime copy and fail installation if its version differs.
import { cp, readFile, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const source = new URL('../node_modules/brace-expansion/', import.meta.url);
const target = new URL('../node_modules/@earendil-works/pi-coding-agent/node_modules/brace-expansion/', import.meta.url);
// The script lives in scripts/, so node_modules is one level above it.
const metadata = JSON.parse(await readFile(new URL('package.json', source), 'utf8'));
if (metadata.version !== '5.0.12') throw new Error('Expected brace-expansion 5.0.12 security fix');
await rm(target, { recursive: true, force: true });
await cp(fileURLToPath(source), fileURLToPath(target), { recursive: true });
