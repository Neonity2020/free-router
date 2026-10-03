"""Exercise reasoning policy, shared Key cooldown and secret-free diagnostics."""
import http.client
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[1]
seen = []
class Upstream(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        key = self.headers['Authorization']
        seen.append((key, body))
        self.send_response(429 if key == 'Bearer rate-secret' else 200)
        self.send_header('Retry-After', '1')
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(b'{}')
    def log_message(self, *args): pass

server = ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
threading.Thread(target=server.serve_forever, daemon=True).start()
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    port = sock.getsockname()[1]
with tempfile.TemporaryDirectory() as directory:
    settings = Path(directory) / 'settings.json'
    settings.write_text(json.dumps({'opencode': ['rate-secret', 'healthy-secret']}))
    env = dict(os.environ, SETTINGS_FILE=str(settings), PORT=str(port), HOST='127.0.0.1', GATEWAY_API_KEY='gateway-secret',
               OPENCODE_API_KEY='', OPENROUTER_API_KEY='', COMMANDCODE_API_KEY='',
               GATEWAY_KEY_COOLDOWN_SECS='1', OPENCODE_BASE_URL=f'http://127.0.0.1:{server.server_port}/v1')
    with open(Path(directory) / 'log', 'w+') as log:
        proc = subprocess.Popen([str(ROOT / 'backend/target/debug/free-router')], cwd=ROOT, env=env, stdout=subprocess.DEVNULL, stderr=log)
        def request(path, body=None):
            conn = http.client.HTTPConnection('127.0.0.1', port, timeout=3)
            conn.request('POST' if body is not None else 'GET', path, json.dumps(body) if body is not None else None,
                         {'Content-Type': 'application/json', 'Authorization': 'Bearer gateway-secret', 'X-Gateway-Settings': '1'})
            res = conn.getresponse()
            data = res.read()
            headers = dict(res.getheaders())
            conn.close()
            return res.status, headers, data
        def chat(effort='max'):
            return request('/v1/chat/completions', {'model':'opencode/space-bunny', 'reasoning_effort':effort, 'messages':[{'role':'user','content':'private-prompt'}]})
        try:
            for _ in range(50):
                try:
                    request('/api/status')
                    break
                except OSError: time.sleep(.1)
            assert chat('none')[0] == 400
            assert not seen, seen
            _, _, data = request('/v1/models')
            declared = json.loads(data)['data'][2]['reasoning']
            assert declared['mandatory'] and declared['supported_efforts'] == ['low','medium','high','xhigh','max']
            code, headers, _ = chat()
            assert code == 200 and headers['x-gateway-request-id']
            assert len(seen) == 2 and seen[-1][1]['reasoning_effort'] == 'max', seen
            for _ in range(3): assert chat()[0] == 200
            assert sum(key == 'Bearer rate-secret' for key, _ in seen) == 1, seen
            # Keeping an existing credential must preserve its health state.
            assert request('/api/settings', {'opencode':['rate-secret']})[0] == 200
            before = len(seen)
            code, headers, _ = chat()
            assert code == 503 and int(headers['retry-after']) >= 1
            assert len(seen) == before
            time.sleep(1.1)
            assert chat()[0] == 429
            assert len(seen) == before + 1
            time.sleep(.1)
            log.flush()
            log.seek(0)
            diagnostic = log.read()
            for secret in ['rate-secret','healthy-secret','gateway-secret','private-prompt']:
                assert secret not in diagnostic, diagnostic
            records = [json.loads(line) for line in diagnostic.splitlines() if line.startswith('{')]
            assert any(record['status'] == 503 for record in records), records
            assert any(record['status'] == 400 for record in records), records
            assert all(record['outcome'] == 'complete' for record in records if record['event'] == 'gateway_request'), records
            print('Reasoning, cooldown, recovery and diagnostic privacy tests passed')
        finally:
            proc.terminate()
            proc.wait(timeout=5)
server.shutdown()
server.server_close()
