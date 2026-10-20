#!/usr/bin/env python3
"""Record real GTK file transfers between Fiex and Thunar under Xvfb."""
import csv
import io
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

out = Path('artifacts/native').resolve()
out.mkdir(parents=True, exist_ok=True)
base = Path(tempfile.mkdtemp(prefix='fiex-native-'))
source, target = base / 'source', base / 'other-app'
source.mkdir(); target.mkdir()
(source / 'MOVE_TARGET').mkdir()
(source / 'MOVE_INSIDE.txt').write_text('internal file move verified\n')
(source / 'MOVE_OUTSIDE.txt').write_text('native external file move verified\n')
processes = []

def command(*args):
    return subprocess.check_output(list(map(str, args)), text=True).strip()

def key(*keys):
    command('xdotool', 'key', '--clearmodifiers', *keys)

def wait_for(fn, timeout=25):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        result = fn()
        if result:
            return result
        time.sleep(.3)
    raise RuntimeError('Timed out waiting for native UI or transfer result')

def window(title):
    try:
        return command('xdotool', 'search', '--onlyvisible', '--name', title).splitlines()[-1]
    except subprocess.CalledProcessError:
        return None

def screenshot(name):
    path = out / name
    command('import', '-window', 'root', path)
    return path

def text_points(text, max_x=1200):
    shot = screenshot('latest.png')
    enlarged = out / 'ocr.png'
    command('convert', shot, '-resize', '200%', enlarged)
    tsv = command('tesseract', enlarged, 'stdout', '-l', 'eng+chi_sim', '--psm', '11', 'tsv')
    normalize = lambda value: re.sub(r'[^a-z0-9\u4e00-\u9fff]', '', value.lower())
    desired = normalize(text)
    lines = {}
    for row in csv.DictReader(io.StringIO(tsv), delimiter='\t'):
        if not (row.get('text') or '').strip():
            continue
        identity = (row['block_num'], row['par_num'], row['line_num'])
        lines.setdefault(identity, []).append(row)
    matches = []
    for rows in lines.values():
        actual = normalize(''.join(row['text'] for row in rows))
        if desired in actual:
            left = min(int(row['left']) for row in rows)
            top = min(int(row['top']) for row in rows)
            right = max(int(row['left'])+int(row['width']) for row in rows)
            bottom = max(int(row['top'])+int(row['height']) for row in rows)
            x, y = (left+right)/4, (top+bottom)/4
            if x < max_x:
                matches.append((int(x),int(y)))
    return matches

def drag(start, end):
    command('xdotool', 'mousemove', *start)
    command('xdotool', 'mousedown', '1')
    for i in range(1, 31):
        command('xdotool', 'mousemove', int(start[0] + (end[0]-start[0])*i/30), int(start[1] + (end[1]-start[1])*i/30))
        time.sleep(.04)
    time.sleep(.6)
    command('xdotool', 'mouseup', '1')
    time.sleep(1)

recorder = None
try:
    processes.append(subprocess.Popen(['openbox'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    processes.append(subprocess.Popen(['xcompmgr'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    app_env = dict(os.environ, WEBKIT_DISABLE_COMPOSITING_MODE='1', WEBKIT_DISABLE_DMABUF_RENDERER='1', LIBGL_ALWAYS_SOFTWARE='1')
    app_log = (out / 'fiex.log').open('w')
    processes.append(subprocess.Popen([str(Path(sys.argv[1]).resolve())], env=app_env, stdout=app_log, stderr=subprocess.STDOUT))
    fiex = wait_for(lambda: window('^Fiex$'))
    command('xdotool', 'windowmove', fiex, 0, 0)
    command('xdotool', 'windowsize', fiex, 1200, 850)
    command('xdotool', 'windowactivate', '--sync', fiex)
    time.sleep(2)
    key('Escape')
    key('ctrl+l')
    command('xdotool', 'type', '--clearmodifiers', '--delay', 8, source)
    key('Return')
    wait_for(lambda: text_points('MOVE_INSIDE'))
    recorder = subprocess.Popen(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'x11grab', '-framerate', '25', '-video_size', '1700x950', '-i', os.environ['DISPLAY'], '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '22', '-pix_fmt', 'yuv420p', str(out/'fiex-native-drag.mp4')])
    time.sleep(2)
    drag(text_points('MOVE_INSIDE')[0], text_points('MOVE_TARGET')[0])
    key('Tab', 'Tab', 'Return')
    wait_for(lambda: (source/'MOVE_TARGET'/'MOVE_INSIDE.txt').is_file())
    assert not (source/'MOVE_INSIDE.txt').exists()
    screenshot('internal-move.png')
    time.sleep(2)
    # A favorite is an entry pointing at a folder, not a destructive move.
    favorites = wait_for(lambda: text_points('收藏', max_x=220))
    drag(text_points('MOVE_TARGET')[0], sorted(favorites, key=lambda point: point[1])[0])
    wait_for(lambda: text_points('MOVE_TARGET', max_x=220))
    assert (source/'MOVE_TARGET').is_dir()
    screenshot('favorite.png')
    time.sleep(2)
    processes.append(subprocess.Popen(['thunar', '--new-window', str(target)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
    thunar = wait_for(lambda: window('other-app'))
    command('xdotool', 'windowmove', thunar, 1200, 0)
    command('xdotool', 'windowsize', thunar, 500, 850)
    command('xdotool', 'windowactivate', '--sync', fiex)
    time.sleep(1)
    drag(text_points('MOVE_OUTSIDE')[0], (1460, 350))
    wait_for(lambda: (target/'MOVE_OUTSIDE.txt').is_file())
    assert (target/'MOVE_OUTSIDE.txt').read_text() == 'native external file move verified\n'
    result = {'internal_move': True, 'favorite_added_without_moving_folder': True, 'external_drop_received': True, 'external_source_removed': not (source/'MOVE_OUTSIDE.txt').exists()}
    import json
    (out/'verified-results.json').write_text(json.dumps(result, indent=2))
    screenshot('external-drop.png')
    time.sleep(4)
finally:
    screenshot('final.png')
    if recorder:
        recorder.terminate()
        recorder.wait(timeout=20)
    for process in reversed(processes):
        process.terminate()
