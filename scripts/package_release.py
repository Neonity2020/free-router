"""Build a complete, portable binary + frontend release archive."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tomllib

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--target')
parser.add_argument('--verify-version')
args = parser.parse_args()
version = tomllib.loads((root / 'backend/Cargo.toml').read_text())['package']['version']
if args.verify_version:
    frontend_version = json.loads((root / 'frontend/package.json').read_text())['version']
    if frontend_version != version:
        raise SystemExit('Frontend and backend versions must match')
    if args.verify_version != f'v{version}':
        raise SystemExit(f'Tag must match Cargo version v{version}')
    raise SystemExit(0)
if args.target not in ['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu']:
    raise SystemExit('Specify a supported --target')
binary = root / f'backend/target/{args.target}/release/free-router'
if not binary.is_file() or not (root / 'frontend/dist/index.html').is_file():
    raise SystemExit('Build release binary and frontend first')
if not (root / 'agent/node_modules/@earendil-works/pi-coding-agent').is_dir():
    raise SystemExit('Install Agent dependencies with npm ci --prefix agent first')
output = root / 'release'
output.mkdir(exist_ok=True)
archive = output / f'free-router-{args.target}.tar.gz'
esbuild_platform = {
    'aarch64-apple-darwin': 'darwin-arm64',
    'x86_64-apple-darwin': 'darwin-x64',
    'x86_64-unknown-linux-gnu': 'linux-x64',
}[args.target]
def agent_filter(info):
    # Pi's shrinkwrap includes esbuild binaries for every platform.
    if '/@esbuild/' in info.name:
        package = info.name.split('/@esbuild/', 1)[1].split('/', 1)[0]
        if package != esbuild_platform:
            return None
    return info
with tarfile.open(archive, 'w:gz') as tar:
    tar.add(binary, arcname='free-router', recursive=False)
    tar.add(root / 'frontend/dist', arcname='frontend/dist')
    tar.add(root / 'README.md', arcname='README.md')
    for name in ['server.mjs', 'tool-events.mjs', 'package.json', 'package-lock.json', 'node_modules']:
        tar.add(root / 'agent' / name, arcname=f'agent/{name}', filter=agent_filter)
    note = f'Free Router {version}\nRun ./free-router from this directory.\nPi Agent coding tools require Node.js 22.19+ on PATH (or set PI_NODE_BIN). Agent dependencies are included.\nWhen updating, stop the old process and copy settings.local.json, gateway-key.local.txt, update-settings.local.json and .env into this directory before starting.\n'.encode()
    info = tarfile.TarInfo('INSTALL.txt')
    info.size = len(note)
    tar.addfile(info, io.BytesIO(note))
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
if archive.stat().st_size > 150 * 1024 * 1024:
    raise SystemExit('Release archive exceeds the auto-updater 150 MiB size limit')
archive.with_suffix(archive.suffix + '.sha256').write_text(f'{digest}  {archive.name}\n')
print(archive)
