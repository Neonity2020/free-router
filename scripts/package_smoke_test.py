"""Verify a packaged release serves its frontend outside the source checkout."""
import argparse
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tarfile
import tempfile
import time
import urllib.request
import urllib.error

parser = argparse.ArgumentParser()
parser.add_argument('archive')
args = parser.parse_args()
with tempfile.TemporaryDirectory(prefix='free-router-package-') as directory:
    with tarfile.open(args.archive) as tar:
        names = tar.getnames()
        assert 'free-router' in names and 'frontend/dist/index.html' in names
        assert not any(Path(name).name in ['settings.local.json','gateway-key.local.txt','update-settings.local.json','.env'] for name in names)
        tar.extractall(directory, filter='data')
    with socket.socket() as sock:
        sock.bind(('127.0.0.1',0))
        port = sock.getsockname()[1]
    env = dict(os.environ, PORT=str(port), HOST='127.0.0.1', GATEWAY_API_KEY='', SETTINGS_FILE=str(Path(directory) / 'settings.local.json'))
    proc = subprocess.Popen([str(Path(directory)/'free-router')], env=env, cwd=Path(directory).parent, stdout=subprocess.DEVNULL)
    base = f'http://127.0.0.1:{port}'
    try:
        for _ in range(50):
            try:
                with urllib.request.urlopen(base+'/api/status',timeout=2) as response: json.load(response)
                break
            except urllib.error.URLError: time.sleep(.1)
        with urllib.request.urlopen(base) as response: html=response.read().decode()
        asset = re.search(r'src="(/assets/[^"]+\.js)"', html).group(1)
        with urllib.request.urlopen(base+asset) as response: assert len(response.read())>1000
        with urllib.request.urlopen(base+'/api/updates') as response: assert json.load(response)['current_version']=='0.1.0'
        print('PASS: packaged binary, frontend assets, update API and no credentials in archive; started outside source checkout')
    finally:
        proc.terminate(); proc.wait(timeout=5)
