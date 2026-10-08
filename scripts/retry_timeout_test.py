"""Verify the real gateway shares a deadline across retries and response streaming."""
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
        scenario = body['messages'][0]['content']
        seen.append((scenario, self.server.provider, self.headers['Authorization']))
        time.sleep(.35 if scenario != 'success' else .1)
        if scenario == 'exhaust' or self.server.provider == 'opencode':
            self.send_response(429)
            self.end_headers()
            return
        self.send_response(200)
        streaming = scenario in ('stream', 'first-event-timeout')
        self.send_header('Content-Type', 'text/event-stream' if streaming else 'application/json')
        self.send_header('Content-Length', '1000' if streaming else '2')
        self.end_headers()
        try:
            if scenario == 'first-event-timeout':
                self.wfile.write(b': keep-alive\r\n\r\n')
                self.wfile.flush()
                time.sleep(.6)
            self.wfile.write(b'data: hello\n\n' if streaming else b'{}')
            self.wfile.flush()
            if scenario == 'stream':
                time.sleep(.6)
                self.wfile.write(b'data: [DONE]\n\n')
        except (BrokenPipeError, ConnectionResetError):
            pass

    def log_message(self, *args):
        pass


servers = []
for provider in ('opencode', 'openrouter'):
    server = ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
    server.provider = provider
    threading.Thread(target=server.serve_forever, daemon=True).start()
    servers.append(server)
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    port = sock.getsockname()[1]

with tempfile.TemporaryDirectory() as directory:
    settings = Path(directory) / 'settings.json'
    settings.write_text(json.dumps({'opencode': ['a', 'b', 'c', 'd'], 'openrouter': ['e'], 'default_provider': 'opencode'}))
    env = dict(os.environ, SETTINGS_FILE=str(settings), HOST='127.0.0.1', PORT=str(port),
               GATEWAY_API_KEY='test-local', GATEWAY_KEY_COOLDOWN_SECS='0', GATEWAY_REQUEST_TIMEOUT_SECS='1',
               OPENCODE_API_KEY='', OPENROUTER_API_KEY='', COMMANDCODE_API_KEY='',
               OPENCODE_BASE_URL=f'http://127.0.0.1:{servers[0].server_port}/v1',
               OPENROUTER_BASE_URL=f'http://127.0.0.1:{servers[1].server_port}/v1')
    proc = subprocess.Popen([str(ROOT / 'backend/target/debug/free-router')], env=env, cwd=ROOT, stdout=subprocess.DEVNULL)
    try:
        def request(path, body=None):
            conn = http.client.HTTPConnection('127.0.0.1', port, timeout=3)
            conn.request('POST' if body is not None else 'GET', path,
                         json.dumps(body) if body is not None else None,
                         {'Content-Type': 'application/json', 'Authorization': 'Bearer test-local', 'X-Gateway-Settings': '1'})
            return conn, conn.getresponse()

        for _ in range(50):
            try:
                conn, res = request('/api/status')
                res.read()
                conn.close()
                break
            except OSError:
                time.sleep(.1)
        else:
            raise AssertionError('gateway did not start')

        def chat(scenario):
            return request('/v1/chat/completions', {'model': 'space-bunny', 'messages': [{'role': 'user', 'content': scenario}], 'stream': scenario in ('stream', 'first-event-timeout')})

        started = time.monotonic()
        conn, res = chat('exhaust')
        assert res.status == 504, res.status
        assert json.loads(res.read())['error']['code'] == 504
        conn.close()
        assert .8 < time.monotonic() - started < 1.8
        assert len([entry for entry in seen if entry[0] == 'exhaust']) == 3, seen
        assert all(entry[1] == 'opencode' for entry in seen)

        conn, res = request('/api/settings', {'opencode': ['a']})
        assert res.status == 200
        res.read()
        conn.close()
        conn, res = chat('success')
        assert res.status == 200 and res.read() == b'{}'
        assert res.getheader('x-gateway-provider') == 'openrouter'
        conn.close()

        started = time.monotonic()
        conn, res = chat('stream')
        assert res.status == 200
        try:
            res.read()
            raise AssertionError('stream unexpectedly completed')
        except http.client.IncompleteRead as error:
            assert b'data: hello' in error.partial
            assert b'[DONE]' not in error.partial
        finally:
            conn.close()
        assert .8 < time.monotonic() - started < 1.8
        assert [entry[1] for entry in seen if entry[0] == 'stream'] == ['opencode', 'openrouter']
        started = time.monotonic()
        conn, res = chat('first-event-timeout')
        assert res.status == 504, res.status
        assert json.loads(res.read())['error']['code'] == 504
        conn.close()
        assert .8 < time.monotonic() - started < 1.8
        assert [entry[1] for entry in seen if entry[0] == 'first-event-timeout'] == ['opencode', 'openrouter']
        print('PASS: shared retry deadline returns 504, fallback succeeds within budget, SSE uses remaining budget without retry, first-event timeout returns 504')
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
for server in servers:
    server.shutdown()
    server.server_close()
