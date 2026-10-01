"""Exercise the real gateway against isolated local upstream servers."""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[1]
seen = []
class Upstream(BaseHTTPRequestHandler):
    def do_POST(self):
        data = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        seen.append((self.server.provider, data, self.headers.get('Authorization')))
        if self.server.provider == 'opencode':
            self.send_response(429)
            self.end_headers()
            self.wfile.write(b'{"error":{"message":"rate limited"}}')
            return
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream' if data.get('stream') else 'application/json')
        self.end_headers()
        self.wfile.write(b'data: {"choices":[{"delta":{"content":"hello"}}]}\n\ndata: [DONE]\n\n' if data.get('stream') else b'{"choices":[{"message":{"content":"hello"}}]}')
    def log_message(self, *args): pass

def request(path, data=None, token='test-local', settings_header=True):
    req = urllib.request.Request(f'http://127.0.0.1:{port}{path}', data=json.dumps(data).encode() if data is not None else None, headers={'Content-Type':'application/json', 'Authorization':f'Bearer {token}'})
    if settings_header: req.add_header('X-Gateway-Settings', '1')
    try:
        with urllib.request.urlopen(req, timeout=5) as res: return res.status, res.read(), res.headers
    except urllib.error.HTTPError as e: return e.code, e.read(), e.headers

servers = []
for provider in ['opencode', 'openrouter']:
    server = ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
    server.provider = provider
    threading.Thread(target=server.serve_forever, daemon=True).start()
    servers.append(server)
with socket.socket() as s:
    s.bind(('127.0.0.1', 0))
    port = s.getsockname()[1]
settings_dir = tempfile.TemporaryDirectory()
settings_file = Path(settings_dir.name) / "settings.json"
env = dict(os.environ, SETTINGS_FILE=str(settings_file), PORT=str(port), HOST='127.0.0.1', GATEWAY_API_KEY='test-local', DEFAULT_PROVIDER='opencode', OPENCODE_API_KEY='mock-zen', OPENROUTER_API_KEY='mock-router', OPENCODE_BASE_URL=f'http://127.0.0.1:{servers[0].server_port}/v1', OPENROUTER_BASE_URL=f'http://127.0.0.1:{servers[1].server_port}/v1')
proc = subprocess.Popen([str(ROOT / 'backend/target/debug/free-router')], cwd=ROOT, env=env, stdout=subprocess.DEVNULL)
try:
    for _ in range(50):
        try:
            request('/api/status')
            break
        except urllib.error.URLError: time.sleep(.1)
    assert request('/api/gateway-key', {}, token='wrong')[0] == 401
    assert request('/api/gateway-key', {}, settings_header=False)[0] == 403
    assert json.loads(request('/api/gateway-key')[1])['key'] == ''
    code, data, headers = request('/api/gateway-key', {})
    gateway_key = json.loads(data)['key']
    assert code == 200 and gateway_key.startswith('fr_') and len(gateway_key) == 67
    assert headers['Cache-Control'] == 'no-store'
    assert request('/v1/models', token=gateway_key)[0] == 200
    assert request('/api/settings', {}, token=gateway_key)[0] == 401
    assert gateway_key not in request('/api/status')[1].decode()
    key_file = Path(settings_dir.name) / 'gateway-key.local.txt'
    assert key_file.stat().st_mode & 0o777 == 0o600
    rotated_key = json.loads(request('/api/gateway-key', {})[1])['key']
    assert rotated_key != gateway_key
    assert request('/v1/models', token=gateway_key)[0] == 401
    assert request('/v1/models', token=rotated_key)[0] == 200
    assert request('/api/updates', token='wrong')[0] == 401
    update_status = json.loads(request('/api/updates')[1])
    assert update_status['current_version'] == '0.1.0'
    assert request('/api/updates/check', {})[0] == 409
    assert request('/api/updates/settings', {'repository':'../repo','auto_download':True})[0] == 400
    assert request('/api/updates/settings', {'repository':'','auto_download':False}, settings_header=False)[0] == 403
    assert request('/api/updates/settings', {'repository':'','auto_download':False})[0] == 200
    assert json.loads(request('/api/updates')[1])['config']['auto_download'] is False
    assert request('/v1/models', token='wrong')[0] == 401
    assert len(json.loads(request('/v1/models')[1])['data']) == 3
    assert request('/v1/chat/completions', {'model':'unknown','messages':[{'role':'user','content':'hello'}]})[0] == 400
    body = {'model':'space-bunny','messages':[{'role':'user','content':'hello'}], 'temperature':.2}
    code, data, headers = request('/v1/chat/completions', body)
    assert code == 200 and json.loads(data)['choices'][0]['message']['content'] == 'hello'
    assert headers['x-gateway-provider'] == 'openrouter'
    assert seen[-2][1]['model'] == 'space-bunny-free' and seen[-1][1]['model'] == 'stealth/space-bunny-alpha'
    assert seen[-2][2] == 'Bearer mock-zen' and seen[-1][2] == 'Bearer mock-router'
    assert seen[-1][1]['temperature'] == .2
    before = len(seen)
    assert request('/v1/chat/completions', dict(body, model='opencode/space-bunny'))[0] == 429
    assert len(seen) == before + 1
    code, data, headers = request('/v1/chat/completions', dict(body, model='openrouter/space-bunny', stream=True))
    assert code == 200 and b'data: [DONE]\n\n' in data and headers['Content-Type'] == 'text/event-stream'
    status = json.loads(request('/api/status')[1])
    assert status['fallbacks'] == 1 and status['requests'] == 3
    assert 'mock-zen' not in str(status)
    assert request('/api/settings', {'openrouter':'replacement'}, token='wrong')[0] == 401
    assert request('/api/settings', {'openrouter':'replacement'}, settings_header=False)[0] == 403
    assert request('/api/settings', {'openrouter':''})[0] == 400
    assert request('/api/settings', {'openrouter':'replacement', 'default_provider':'openrouter'})[0] == 200
    assert settings_file.stat().st_mode & 0o777 == 0o600
    assert json.loads(settings_file.read_text())['openrouter'] == 'replacement'
    request('/v1/chat/completions', body)
    assert seen[-1][2] == 'Bearer replacement'
    assert json.loads(request('/api/status')[1])['default_provider'] == 'openrouter'
    assert 'replacement' not in request('/api/status')[1].decode()
    proc.terminate(); proc.wait(timeout=5)
    proc = subprocess.Popen([str(ROOT / 'backend/target/debug/free-router')], cwd=ROOT, env=env, stdout=subprocess.DEVNULL)
    for _ in range(50):
        try:
            request('/api/status'); break
        except urllib.error.URLError: time.sleep(.1)
    assert json.loads(request('/api/gateway-key')[1])['key'] == rotated_key
    assert json.loads(request('/api/updates')[1])['config']['auto_download'] is False
    assert request('/v1/models', token=rotated_key)[0] == 200
    request('/v1/chat/completions', body, token=rotated_key)
    assert seen[-1][2] == 'Bearer replacement'
    assert request('/api/settings', {'openrouter':None,'opencode':None})[0] == 200
    assert request('/v1/chat/completions', body)[0] == 503
    assert not any(p['configured'] for p in json.loads(request('/api/status')[1])['providers'])
    proc.terminate(); proc.wait(timeout=5)
    env['GATEWAY_API_KEY'] = ''
    proc = subprocess.Popen([str(ROOT / 'backend/target/debug/free-router')], cwd=ROOT, env=env, stdout=subprocess.DEVNULL)
    for _ in range(50):
        try:
            request('/api/status'); break
        except urllib.error.URLError: time.sleep(.1)
    assert request('/v1/models', token='local')[0] == 401
    assert request('/v1/models', token='')[0] == 401
    assert request('/v1/models', token=rotated_key)[0] == 200
    assert json.loads(request('/api/status')[1])['management_auth_required'] is False
    assert json.loads(request('/api/gateway-key')[1])['key'] == rotated_key
    assert json.loads(request('/api/updates')[1])['config']['auto_download'] is False
    print('PASS: gateway key generation/rotation/reload, admin isolation, authentication without environment key, settings save/reload/clear, immediate effect, restricted file permissions, settings authentication,  authentication, model list, validation, fallback, model mapping, key isolation, direct routing, SSE and statistics')
finally:
    proc.terminate()
    proc.wait(timeout=5)
    for server in servers: server.shutdown()
    settings_dir.cleanup()
