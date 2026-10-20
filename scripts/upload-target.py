#!/usr/bin/env python3
"""Real Chromium upload target used only by the local native recording."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import threading
from urllib.parse import unquote

destination = Path(sys.argv[1])
page = b'''<!doctype html><meta charset=utf-8><title>Browser Upload</title>
<style>body{font:15px system-ui;background:#f6f8fc;color:#243849;margin:32px}small{color:#688092}h1{font-size:25px;margin-top:34px}#drop{border:2px dashed #86abc6;border-radius:18px;background:white;padding:58px 20px;text-align:center;margin-top:30px;min-height:180px;display:grid;place-content:center;gap:14px}#drop.over{background:#e5f2fc;border-color:#337ba6}#status{font-size:20px;font-weight:600}#file{overflow-wrap:anywhere}#done{color:#23734e;margin-top:30px}</style>
<small>CHROMIUM BROWSER / LOCAL UPLOAD</small><h1>Drop a file to upload</h1><p>Drag from Fiex into this browser.</p><div id=drop><div id=status>Drop here</div><small>Your source file stays in Fiex.</small><input id=pick type=file></div><div id=done></div><p id=file></p>
<script>const zone=document.querySelector('#drop');async function upload(file){if(!file)return;document.querySelector('#status').textContent='Uploading...';const r=await fetch('/',{method:'POST',headers:{'X-File-Name':encodeURIComponent(file.name)},body:file});if(!r.ok)throw Error('Upload failed');document.querySelector('#status').textContent='Upload complete';document.querySelector('#done').textContent='Received by the upload server';document.querySelector('#file').textContent=file.name+' / '+file.size+' bytes';zone.classList.remove('over')}window.addEventListener('dragover',e=>{e.preventDefault();e.dataTransfer.dropEffect='copy';zone.classList.add('over')});window.addEventListener('drop',e=>{e.preventDefault();upload(e.dataTransfer.files[0]).catch(e=>document.querySelector('#status').textContent=e.message)});document.querySelector('#pick').onchange=e=>upload(e.target.files[0]);</script>'''

class Upload(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.end_headers()
        self.wfile.write(page)
    def do_POST(self):
        data = self.rfile.read(int(self.headers['Content-Length']))
        destination.write_bytes(data)
        destination.with_suffix('.json').write_text(json.dumps({'name': unquote(self.headers.get('X-File-Name', '')), 'bytes': len(data)}))
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'ok')
    def log_message(self, *_):
        pass

server = ThreadingHTTPServer(('127.0.0.1', 0), Upload)
threading.Thread(target=server.serve_forever, daemon=True).start()
import os
import subprocess
import tempfile

browser = os.environ.get('FIEX_UPLOAD_ELECTRON')
if not browser:
    raise SystemExit('Set FIEX_UPLOAD_ELECTRON to an Electron executable for upload verification.')
with tempfile.TemporaryDirectory(prefix='fiex-upload-browser-') as profile:
    command = [browser, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
               '--user-data-dir=' + profile, str(Path(__file__).with_name('upload-browser.cjs')),
               f'http://127.0.0.1:{server.server_port}/']
    raise SystemExit(subprocess.call(command))
