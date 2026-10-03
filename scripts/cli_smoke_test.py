"""CLI integration: offline transactions, live auth, lock recovery, installed paths."""
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'backend/target/debug/free-router'


def unused_port():
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


with tempfile.TemporaryDirectory(prefix='free-router-cli-test-') as directory:
    directory = Path(directory)
    settings = directory / 'config/settings.json'
    port = unused_port()
    env = dict(os.environ, SETTINGS_FILE=str(settings), HOST='127.0.0.1', PORT=str(port),
               FREE_ROUTER_DESKTOP='1', GATEWAY_API_KEY='admin-test',
               OPENCODE_API_KEY='env-key', OPENROUTER_API_KEY='', COMMANDCODE_API_KEY='',
               EXA_API_KEY='', DEFAULT_PROVIDER='opencode', TOKIO_WORKER_THREADS='2')

    def cli(*args, override=None, ok=True, binary=BINARY):
        result = subprocess.run([str(binary), *args], env=dict(env, **(override or {})), cwd=directory,
                                capture_output=True, text=True, timeout=20)
        assert (result.returncode == 0) == ok, (args, result.returncode, result.stderr)
        return result

    # Every update includes inherited env keys and is serialized across processes.
    with ThreadPoolExecutor(max_workers=10) as pool:
        list(pool.map(lambda index: cli('keys', 'add', 'opencode', f'parallel-{index}'), range(10)))
    saved = json.loads(settings.read_text())
    assert set(saved['opencode']) == {'env-key', *(f'parallel-{index}' for index in range(10))}
    assert settings.stat().st_mode & 0o777 == 0o600
    assert not list(settings.parent.glob('.*.tmp'))
    listed = json.loads(cli('keys', 'list', 'opencode', '--json').stdout)
    assert listed[0]['key_count'] == 11 and 'parallel-' not in json.dumps(listed)
    cli('keys', 'remove', 'opencode', listed[0]['keys'][0]['id'][:16])
    assert len(json.loads(settings.read_text())['opencode']) == 10
    cli('provider', 'openrouter')
    assert cli('provider').stdout.strip() == 'openrouter'
    cli('keys', 'clear', 'opencode')
    assert json.loads(cli('status', '--json').stdout)['providers'][0]['key_count'] == 0
    cli('keys', 'add', 'unknown', 'secret', ok=False)
    key = cli('gateway-key', 'generate').stdout.strip()
    assert key.startswith('fr_') and cli('gateway-key', 'show').stdout.strip() == key
    assert (settings.parent / 'gateway-key.local.txt').stat().st_mode & 0o777 == 0o600

    def start():
        proc = subprocess.Popen([str(BINARY), 'serve'], env=env, cwd=directory,
                                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        for _ in range(100):
            try:
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/api/status', timeout=.2) as response:
                    json.loads(response.read())
                return proc
            except OSError:
                if proc.poll() is not None:
                    raise AssertionError(proc.stderr.read().decode())
                time.sleep(.05)
        proc.kill()
        proc.wait()
        raise AssertionError('gateway did not start')

    proc = start()
    try:
        before = settings.read_bytes()
        before_key = (settings.parent / 'gateway-key.local.txt').read_bytes()
        for args in [('keys', 'add', 'opencode', 'wrong-auth-key'), ('provider', 'commandcode'), ('gateway-key', 'generate'), ('gateway-key', 'show')]:
            cli(*args, override={'GATEWAY_API_KEY': 'wrong'}, ok=False)
        assert settings.read_bytes() == before
        assert (settings.parent / 'gateway-key.local.txt').read_bytes() == before_key
        cli('keys', 'add', 'opencode', 'live-key')
        assert next(p for p in json.loads(cli('status', '--json').stdout)['providers'] if p['id'] == 'opencode')['key_count'] == 1
        # Show reads the live service even with a different local settings path.
        alternate = {'SETTINGS_FILE': str(directory / 'other/settings.json')}
        assert cli('gateway-key', 'show', override=alternate).stdout.strip() == key
        rotated = cli('gateway-key', 'generate').stdout.strip()
        assert rotated != key and cli('gateway-key', 'show').stdout.strip() == rotated
        # Wrong port must not permit an offline write while this config is in use.
        cli('provider', 'commandcode', override={'PORT': str(unused_port())}, ok=False)
        assert json.loads(settings.read_text())['default_provider'] == 'openrouter'
        second = subprocess.run([str(BINARY), 'serve'], env=dict(env, PORT=str(unused_port())),
                                capture_output=True, text=True, timeout=5)
        assert second.returncode != 0 and '配置正在使用中' in second.stderr
    finally:
        proc.kill()  # OS lock must also recover after an ungraceful exit.
        proc.wait(timeout=5)
        proc.stderr.close()
    cli('provider', 'commandcode')
    assert cli('provider').stdout.strip() == 'commandcode'

    class Unrelated(BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"service":"Other"}')
        def log_message(self, *args):
            pass

    server = ThreadingHTTPServer(('127.0.0.1', 0), Unrelated)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        before = settings.read_bytes()
        cli('provider', 'opencode', override={'PORT': str(server.server_port)}, ok=False)
        assert settings.read_bytes() == before
    finally:
        server.shutdown()
        server.server_close()

    # A copied binary no longer resolves resources/config against its build checkout.
    installed = directory / 'bin/free-router'
    installed.parent.mkdir()
    shutil.copy2(BINARY, installed)
    clean = dict(os.environ, FREE_ROUTER_DESKTOP='1')
    for name in ('SETTINGS_FILE', 'FREE_ROUTER_RESOURCE_ROOT'):
        clean.pop(name, None)
    result = subprocess.run([str(installed), 'config'], env=clean, cwd=directory,
                            capture_output=True, text=True, check=True)
    assert str(ROOT) not in result.stdout
    assert str(installed.parent) in result.stdout
    resources = installed.parent / 'free-router-resources'
    (resources / 'frontend/dist').mkdir(parents=True)
    (resources / 'frontend/dist/index.html').write_text('installed UI')
    result = cli('config', binary=installed)
    assert str(resources) in result.stdout
    assert str(settings) in result.stdout
    # Switching cwd must not load an unrelated project's .env.
    (directory / '.env').write_text('FREE_ROUTER_RESOURCE_ROOT=/unrelated/project\n')
    dotenv_env = dict(env)
    dotenv_env.pop('FREE_ROUTER_DESKTOP', None)
    dotenv_env.pop('FREE_ROUTER_RESOURCE_ROOT', None)
    result = subprocess.run([str(installed), 'config'], env=dotenv_env, cwd=directory,
                            capture_output=True, text=True, check=True)
    assert str(resources) in result.stdout and '/unrelated/project' not in result.stdout
    # Actual installed serve uses its copied UI, independent of the source root.
    installed_proc = subprocess.Popen([str(installed), 'serve'], env=env, cwd=directory,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            try:
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/', timeout=.2) as response:
                    assert response.read() == b'installed UI'
                break
            except OSError:
                if installed_proc.poll() is not None:
                    raise AssertionError(installed_proc.stderr.read().decode())
                time.sleep(.05)
        else:
            raise AssertionError('installed gateway did not start')
    finally:
        installed_proc.terminate()
        installed_proc.wait(timeout=5)
        installed_proc.stderr.close()

print('PASS: concurrent CLI updates, env inheritance, offline/live commands, auth rejection, process locks and installed resource paths')
