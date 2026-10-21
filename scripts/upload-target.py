#!/usr/bin/env python3
"""Real browser upload target used only by the local native recording.

The page posts the dropped file back so the recording can verify the exact bytes,
and it also reports what the drop event contained, so a browser that accepts the
drag but hands the page nothing usable explains itself in the job log.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import threading
from urllib.parse import unquote, urlparse

destination = Path(sys.argv[1])
page = b'''<!doctype html><meta charset=utf-8><title>Browser Upload</title>
<style>body{font:15px system-ui;background:#f6f8fc;color:#243849;margin:32px}small{color:#688092}h1{font-size:25px;margin-top:34px}#drop{border:2px dashed #86abc6;border-radius:18px;background:white;padding:58px 20px;text-align:center;margin-top:30px;min-height:180px;display:grid;place-content:center;gap:14px}#drop.over{background:#e5f2fc;border-color:#337ba6}#status{font-size:20px;font-weight:600}#file{overflow-wrap:anywhere}#done{color:#23734e;margin-top:30px}</style>
<small>BROWSER / LOCAL UPLOAD</small><h1>Drop a file to upload</h1><p>Drag from Fiex into this browser.</p><div id=drop><div id=status>Drop here</div><small>Your source file stays in Fiex.</small><input id=pick type=file></div><div id=done></div><p id=file></p>
<script>const zone=document.querySelector('#drop');const status=document.querySelector('#status');
function each(list,visit){if(!list)return[];const out=[];for(let i=0;i<list.length;i++)out.push(visit(list[i],i));return out}
function firstFile(dt){const files=each(dt.files,(f)=>f);if(files.length)return files[0];const items=each(dt.items,(item)=>item.kind==='file'?item.getAsFile():null).filter(Boolean);return items[0]||null}
function describe(dt){const data={};try{data.uri=dt.getData('text/uri-list')}catch(e){data.uriError=String(e)}data.peeked=peekedtry{data.text=dt.getData('text/plain')}catch(e){data.textError=String(e)}return {types:each(dt.types,(t)=>t),items:each(dt.items,(item)=>item.kind+'/'+item.type),files:each(dt.files,(f)=>f.name+':'+f.size),data}}
async function upload(file){if(!file)return;status.textContent='Uploading...';const r=await fetch('/',{method:'POST',headers:{'X-File-Name':encodeURIComponent(file.name)},body:file});if(!r.ok)throw Error('Upload failed');status.textContent='Upload complete';document.querySelector('#done').textContent='Received by the upload server';document.querySelector('#file').textContent=file.name+' / '+file.size+' bytes';zone.classList.remove('over')}
// WebKit only hands the drop handler the dragged data when the page cancelled
// dragenter as well as dragover; without it the drop arrives with an empty
// DataTransfer (no files, empty uri-list).
let peeked={};window.addEventListener('dragenter',e=>{e.preventDefault();zone.classList.add('over');try{peeked.uri=e.dataTransfer.getData('text/uri-list');peeked.text=e.dataTransfer.getData('text/plain');peeked.files=[];for(let i=0;i<e.dataTransfer.files.length;i++)peeked.files.push(e.dataTransfer.files[i].name)}catch(error){peeked.error=String(error)}});
window.addEventListener('dragover',e=>{e.preventDefault();e.dataTransfer.dropEffect='copy';zone.classList.add('over')});
window.addEventListener('drop',e=>{e.preventDefault();let info={};try{info=describe(e.dataTransfer)}catch(error){info={describeError:String(error)}}fetch('/diag',{method:'POST',body:JSON.stringify(info)}).catch(()=>{});const file=firstFile(e.dataTransfer);if(!file){status.textContent='Drop contained no file';return}upload(file).catch(error=>status.textContent=error.message)});
document.querySelector('#pick').onchange=e=>upload(e.target.files[0]);
</script>'''

class Upload(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.end_headers()
        self.wfile.write(page)
    def do_POST(self):
        data = self.rfile.read(int(self.headers['Content-Length']))
        if self.path == '/diag':
            destination.with_suffix('.diag.json').write_bytes(data)
            self.send_response(204)
            self.end_headers()
            return
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
import time

url = f'http://127.0.0.1:{server.server_port}/'
bounds = json.loads(os.environ.get('FIEX_CI_RECEIVER_BOUNDS') or '{}')
# scripts/upload-browser.c (WebKitGTK) is the preferred target: it is a real
# browser that takes part in the desktop's XDND drag-and-drop, unlike Chromium,
# which refuses the drop on a headless X server. Electron stays as the fallback
# for platforms where the small GTK browser is not built.
browser = os.environ.get('FIEX_UPLOAD_BROWSER')
if browser:
    drop = destination.with_suffix('.drop')
    drop.unlink(missing_ok=True)

    def relay():
        """The browser writes the URI list it received here; this relays the
        referenced file to the upload destination so the recorded run verifies the
        same bytes it always did."""
        while True:
            if drop.exists():
                text = drop.read_text()
                drop.unlink(missing_ok=True)
                for line in text.splitlines():
                    uri = line.strip()
                    if not uri.startswith('file://'):
                        continue
                    source = Path(unquote(urlparse(uri).path))
                    if source.is_file():
                        destination.write_bytes(source.read_bytes())
                        destination.with_suffix('.json').write_text(
                            json.dumps({'name': source.name, 'bytes': source.stat().st_size})
                        )
                        break
            time.sleep(.1)

    threading.Thread(target=relay, daemon=True).start()
    command = [
        browser,
        url,
        str(bounds.get('x', 1200)),
        str(bounds.get('y', 0)),
        str(bounds.get('width', 900)),
        str(bounds.get('height', 820)),
    ]
    raise SystemExit(subprocess.call(command, env={**os.environ, 'FIEX_UPLOAD_DROP': str(drop)}))

browser = os.environ.get('FIEX_UPLOAD_ELECTRON')
if not browser:
    raise SystemExit('Set FIEX_UPLOAD_BROWSER (or FIEX_UPLOAD_ELECTRON) for upload verification.')
with tempfile.TemporaryDirectory(prefix='fiex-upload-browser-') as profile:
    command = [browser, '--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu',
               '--user-data-dir=' + profile, str(Path(__file__).with_name('upload-browser.cjs')),
               url]
    raise SystemExit(subprocess.call(command))
