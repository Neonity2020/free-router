const { app, BrowserWindow, dialog, ipcMain, Menu, shell } = require('electron');
const { spawn } = require('node:child_process');
const { randomBytes } = require('node:crypto');
const { join } = require('node:path');
const { mkdir, stat } = require('node:fs/promises');
const { findPort, importDevelopmentSettings, trustedFrame } = require('./runtime.cjs');

let window, backend, origin, quitting = false, pickerOpen = false;
process.on('SIGINT', () => app.quit());
process.on('SIGTERM', () => app.quit());
app.setName('Free Router');
if (!app.requestSingleInstanceLock()) app.quit();
else {
  app.on('second-instance', () => { if (window?.isMinimized()) window.restore(); window?.show(); window?.focus(); });
  app.whenReady().then(start).catch(error => {
    dialog.showErrorBox('Free Router 启动失败', error.message); app.quit();
  });
}
async function start() {
  const project = join(__dirname, '..');
  const runtime = app.isPackaged ? join(process.resourcesPath, 'runtime') : project;
  const data = app.getPath('userData');
  await mkdir(data, { recursive: true, mode: 0o700 });
  if (!app.isPackaged) await importDevelopmentSettings(project, data);
  const port = await findPort();
  origin = `http://127.0.0.1:${port}`;
  const managementKey = randomBytes(32).toString('hex');
  const binary = app.isPackaged ? join(runtime, process.platform === 'win32' ? 'free-router.exe' : 'free-router')
    : join(project, 'backend/target/debug', process.platform === 'win32' ? 'free-router.exe' : 'free-router');
  backend = spawn(binary, [], { cwd: data, stdio: ['ignore', 'pipe', 'pipe'], env: {
    PATH: process.env.PATH || '', HOME: app.getPath('home'), TMPDIR: app.getPath('temp'),
    ...(process.env.SystemRoot ? { SystemRoot: process.env.SystemRoot } : {}),
    HOST: '127.0.0.1', PORT: String(port), GATEWAY_API_KEY: managementKey,
    SETTINGS_FILE: join(data, 'settings.local.json'), FREE_ROUTER_RESOURCE_ROOT: runtime,
    FREE_ROUTER_DESKTOP: '1', PI_NODE_BIN: process.execPath, PI_NODE_RUN_AS_NODE: '1',
    PI_AGENT_WORKSPACE: app.isPackaged ? app.getPath('home') : project,
  } });
  let failure;
  backend.on('error', error => { failure = error; });
  backend.stderr.on('data', () => {}); // Never emit credentials or upstream diagnostics into UI logs.
  backend.stdout.on('data', () => {});
  backend.on('exit', code => {
    failure = new Error(`本地网关已退出（${code}）`);
    if (window && !quitting) { dialog.showErrorBox('网关连接中断', '请退出并重新打开 Free Router。'); app.quit(); }
  });
  const deadline = Date.now() + 25000;
  while (Date.now() < deadline) {
    if (failure) throw failure;
    try {
      const response = await fetch(`${origin}/api/gateway-key`, { headers: { Authorization: `Bearer ${managementKey}`, 'X-Gateway-Settings': '1' }, signal: AbortSignal.timeout(800) });
      if (response.ok) break;
    } catch {}
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  if (failure || Date.now() >= deadline) throw failure || new Error('网关启动超时，请检查应用资源是否完整');
  ipcMain.handle('desktop:choose-directory', async (event, cwd) => {
    if (!trustedFrame(event, window, origin)) throw new Error('无效的目录选择请求');
    if (pickerOpen) return { cancelled: true, cwd: null };
    pickerOpen = true;
    try {
      let defaultPath = app.getPath('home');
      if (typeof cwd === 'string' && cwd.length < 4096 && require('node:path').isAbsolute(cwd)) {
        try { if ((await stat(cwd)).isDirectory()) defaultPath = cwd; } catch {}
      }
      const result = await dialog.showOpenDialog(window, { title: '选择 Pi Agent 工作目录', defaultPath, properties: ['openDirectory', 'createDirectory'] });
      return { cancelled: result.canceled, cwd: result.filePaths[0] || null };
    } finally { pickerOpen = false; }
  });
  Menu.setApplicationMenu(Menu.buildFromTemplate([
    ...(process.platform === 'darwin' ? [{ role: 'appMenu' }] : []),
    { label: '文件', submenu: [{ label: '退出 Free Router', click: () => app.quit() }] },
    { role: 'editMenu' }, { role: 'viewMenu' }, { role: 'windowMenu' },
  ]));
  openWindow(managementKey);
  app.on('activate', () => { if (!window) openWindow(managementKey); else window.show(); });
}
function openWindow(managementKey) {
  window = new BrowserWindow({ width: 1360, height: 920, minWidth: 900, minHeight: 650, show: false,
    title: 'Free Router', backgroundColor: '#f5f7f1',
    webPreferences: { preload: join(__dirname, 'preload.cjs'), contextIsolation: true, sandbox: true, nodeIntegration: false },
  });
  const session = window.webContents.session;
  session.setPermissionRequestHandler((contents, permission, callback, details) => callback(
    contents === window?.webContents && permission === 'clipboard-sanitized-write' && details.requestingUrl?.startsWith(`${origin}/`)
  ));
  session.setPermissionCheckHandler((contents, permission, requestingOrigin) =>
    contents === window?.webContents && permission === 'clipboard-sanitized-write' && requestingOrigin === origin);
  session.webRequest.onBeforeSendHeaders({ urls: [`${origin}/*`] }, (details, callback) => {
    const headers = details.requestHeaders;
    for (const name of Object.keys(headers)) if (name.toLowerCase() === 'authorization') delete headers[name];
    headers.Authorization = `Bearer ${managementKey}`;
    callback({ requestHeaders: headers });
  });
  const openExternal = url => { try { if (['https:', 'http:'].includes(new URL(url).protocol)) shell.openExternal(url); } catch {} };
  window.webContents.setWindowOpenHandler(({ url }) => { openExternal(url); return { action: 'deny' }; });
  window.webContents.on('will-navigate', (event, url) => {
    if (new URL(url).origin !== origin) { event.preventDefault(); openExternal(url); }
  });
  window.once('ready-to-show', () => window.show());
  window.on('closed', () => { window = null; });
  window.loadURL(origin);
}
app.on('window-all-closed', () => { if (process.platform !== 'darwin') app.quit(); });
app.on('before-quit', event => {
  if (quitting || !backend?.pid || backend.exitCode !== null || backend.signalCode !== null) return;
  event.preventDefault(); quitting = true;
  const timer = setTimeout(() => backend.kill('SIGKILL'), 8000);
  backend.once('exit', () => { clearTimeout(timer); app.quit(); });
  backend.kill('SIGINT');
});
