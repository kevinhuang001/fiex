#!/usr/bin/env python3
"""Real WebKit upload target used only by the local native recording."""
import ctypes as c
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import threading
from urllib.parse import unquote

destination = Path(sys.argv[1])
page = b'''<!doctype html><meta charset=utf-8><title>Browser Upload</title>
<style>body{font:15px system-ui;background:#f6f8fc;color:#243849;margin:32px}small{color:#688092}h1{font-size:25px;margin-top:34px}#drop{border:2px dashed #86abc6;border-radius:18px;background:white;padding:58px 20px;text-align:center;margin-top:30px;min-height:180px;display:grid;place-content:center;gap:14px}#drop.over{background:#e5f2fc;border-color:#337ba6}#status{font-size:20px;font-weight:600}#file{overflow-wrap:anywhere}#done{color:#23734e;margin-top:30px}</style>
<small>WEBKIT BROWSER / LOCAL UPLOAD</small><h1>Drop a file to upload</h1><p>Drag from Fiex into this browser.</p><div id=drop><div id=status>Drop here</div><small>Your source file stays in Fiex.</small><input id=pick type=file></div><div id=done></div><p id=file></p>
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
gtk = c.CDLL('libgtk-3.so.0')
webkit = c.CDLL('libwebkit2gtk-4.1.so.0')
gtk.gtk_init.argtypes = [c.c_void_p, c.c_void_p]
gtk.gtk_init(None, None)
gtk.gtk_window_new.argtypes = [c.c_int]
gtk.gtk_window_new.restype = c.c_void_p
gtk.gtk_window_set_title.argtypes = [c.c_void_p, c.c_char_p]
gtk.gtk_window_set_default_size.argtypes = [c.c_void_p, c.c_int, c.c_int]
gtk.gtk_window_move.argtypes = [c.c_void_p, c.c_int, c.c_int]
gtk.gtk_container_add.argtypes = [c.c_void_p, c.c_void_p]
gtk.gtk_widget_show_all.argtypes = [c.c_void_p]
webkit.webkit_web_view_new.restype = c.c_void_p
webkit.webkit_web_view_load_uri.argtypes = [c.c_void_p, c.c_char_p]
window = gtk.gtk_window_new(0)
view = webkit.webkit_web_view_new()
gtk.gtk_window_set_title(window, b'Browser Upload')
gtk.gtk_window_set_default_size(window, 500, 850)
gtk.gtk_window_move(window, 1200, 0)
gtk.gtk_container_add(window, view)
webkit.webkit_web_view_load_uri(view, f'http://127.0.0.1:{server.server_port}/'.encode())
gtk.gtk_widget_show_all(window)
gtk.gtk_main()
