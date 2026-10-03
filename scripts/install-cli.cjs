// Install a standalone CLI plus its frontend and Agent resources.
const { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, renameSync, rmSync } = require('node:fs');
const { execFileSync } = require('node:child_process');
const { homedir } = require('node:os');
const { delimiter, join, resolve, sep } = require('node:path');

function install(project, directory) {
  const binary = process.platform === 'win32' ? 'free-router.exe' : 'free-router';
  const source = join(project, 'backend', 'target', 'release', binary);
  for (const path of [source, join(project, 'frontend/dist/index.html'), join(project, 'agent/node_modules/@earendil-works/pi-coding-agent')]) {
    if (!existsSync(path)) throw new Error(`缺少安装资源：${path}；请运行 npm run cli:install`);
  }
  mkdirSync(directory, { recursive: true });
  const stage = mkdtempSync(join(directory, '.free-router-install-'));
  const destination = join(directory, binary);
  const resources = join(directory, 'free-router-resources');
  const stagedResources = join(stage, 'resources');
  const backup = join(stage, 'previous-resources');
  let replacedResources = false;
  let hadResources = false;
  try {
    // Prepare every file before replacing an existing installation.
    cpSync(source, join(stage, binary));
    if (process.platform !== 'win32') chmodSync(join(stage, binary), 0o755);
    cpSync(join(project, 'frontend/dist'), join(stagedResources, 'frontend/dist'), { recursive: true });
    for (const name of ['server.mjs', 'tool-events.mjs', 'web-search.mjs', 'package.json', 'package-lock.json', 'node_modules']) {
      cpSync(join(project, 'agent', name), join(stagedResources, 'agent', name), {
        recursive: true,
        verbatimSymlinks: true,
        filter: path => {
          const platform = path.split(`${sep}@esbuild${sep}`)[1]?.split(sep)[0];
          return !platform || platform === `${process.platform}-${process.arch}`;
        },
      });
    }
    hadResources = existsSync(resources);
    if (hadResources) renameSync(resources, backup);
    try {
      renameSync(stagedResources, resources);
      replacedResources = true;
      // Atomic replacement also replaces an old symlink without writing through it.
      renameSync(join(stage, binary), destination);
    } catch (error) {
      if (replacedResources) rmSync(resources, { recursive: true, force: true });
      if (hadResources) renameSync(backup, resources);
      throw error;
    }
    return destination;
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
}

if (require.main === module) {
  try {
    const project = resolve(__dirname, '..');
    const directory = resolve(process.env.FREE_ROUTER_BIN_DIR || join(homedir(), '.local', 'bin'));
    const destination = install(project, directory);
    console.log(`已安装 ${destination}（包含前端与 Agent 资源）`);
    console.log(execFileSync(destination, ['--version'], { encoding: 'utf8' }).trim());
    const entries = (process.env.PATH || '').split(delimiter).filter(Boolean);
    if (!entries.some(entry => resolve(entry) === directory)) {
      console.warn(`提示：${directory} 不在 PATH 中，请加入 PATH 后再使用 free-router。`);
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}

module.exports = { install };
