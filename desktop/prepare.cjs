const { cp, mkdir, rm, chmod, access } = require('node:fs/promises');
const { join } = require('node:path');
async function prepare() {
  require('electron'); // Ensure the pinned runtime is installed before packaging its local dist.
  const root = join(__dirname, '..');
  const runtime = join(__dirname, 'runtime');
  const binaryName = process.platform === 'win32' ? 'free-router.exe' : 'free-router';
  await access(join(root, 'agent/node_modules/@earendil-works/pi-coding-agent'));
  await rm(runtime, { recursive: true, force: true });
  await mkdir(runtime, { recursive: true });
  await cp(join(root, 'backend/target/release', binaryName), join(runtime, binaryName));
  await chmod(join(runtime, binaryName), 0o755);
  await cp(join(root, 'frontend/dist'), join(runtime, 'frontend/dist'), { recursive: true });
  const esbuildPlatform = `${process.platform}-${process.arch}`;
  for (const name of ['server.mjs', 'tool-events.mjs', 'web-search.mjs', 'package.json', 'package-lock.json', 'node_modules']) {
    await cp(join(root, 'agent', name), join(runtime, 'agent', name), { recursive: true, filter: source => {
      const platform = source.split(`${require('node:path').sep}@esbuild${require('node:path').sep}`)[1]?.split(require('node:path').sep)[0];
      return !platform || platform === esbuildPlatform;
    } });
  }
  console.log(`Desktop runtime prepared (${process.platform}/${process.arch})`);
}
prepare().catch(error => { console.error(error.message); process.exitCode = 1; });
