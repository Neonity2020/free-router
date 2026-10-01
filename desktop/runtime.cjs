const { createServer } = require('node:net');
const { mkdir, copyFile, chmod } = require('node:fs/promises');
const { join } = require('node:path');
const { constants } = require('node:fs');

async function findPort(start = 8787) {
  for (let port = start; port < start + 100; port++) {
    const available = await new Promise(resolve => {
      const server = createServer();
      server.once('error', () => resolve(false));
      server.listen(port, '127.0.0.1', () => server.close(() => resolve(true)));
    });
    if (available) return port;
  }
  throw new Error('无法找到可用的本地网关端口');
}
async function importDevelopmentSettings(source, destination) {
  await mkdir(destination, { recursive: true, mode: 0o700 });
  for (const name of ['settings.local.json', 'gateway-key.local.txt', 'update-settings.local.json']) {
    try {
      await copyFile(join(source, name), join(destination, name), constants.COPYFILE_EXCL);
      await chmod(join(destination, name), 0o600);
    } catch (error) { if (!['ENOENT', 'EEXIST'].includes(error.code)) throw error; }
  }
}
function trustedFrame(event, window, origin) {
  return event.sender === window?.webContents && event.senderFrame === window.webContents.mainFrame &&
    new URL(event.senderFrame.url).origin === origin;
}
module.exports = { findPort, importDevelopmentSettings, trustedFrame };
