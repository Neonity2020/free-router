const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('freeRouterDesktop', Object.freeze({
  platform: process.platform,
  chooseDirectory: cwd => ipcRenderer.invoke('desktop:choose-directory', cwd),
}));
