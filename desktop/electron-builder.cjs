module.exports = {
  appId: 'io.github.neonity2020.free-router', productName: 'Free Router',
  directories: { output: 'desktop/dist' },
  files: ['desktop/*.cjs', 'package.json', '!desktop/prepare.cjs', '!desktop/electron-builder.cjs'],
  extraResources: [{ from: 'desktop/runtime', to: 'runtime' }],
  asar: true,
  electronDist: 'node_modules/electron/dist',
  mac: { target: ['dmg', 'zip'], category: 'public.app-category.developer-tools', identity: null },
  linux: { target: ['AppImage'], category: 'Development' },
  win: { target: ['nsis'] },
  artifactName: 'Free-Router-${version}-${os}-${arch}.${ext}',
};
