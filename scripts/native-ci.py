#!/usr/bin/env python3
"""CI-only desktop integration recording. Native drags use actual OS input.
The optional Rust UI driver locates controls; it never synthesizes drag payloads.
"""
import json
import os
from pathlib import Path
import platform
import secrets
import socket
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import urllib.request
import urllib.error

import pyautogui as mouse
from PIL import Image, ImageDraw
import mss

OUT = Path('artifacts/native').resolve()
OUT.mkdir(parents=True, exist_ok=True)
SYSTEM = platform.system()
TOKEN = secrets.token_hex(32)
with socket.socket() as sock:
    sock.bind(('127.0.0.1', 0))
    PORT = sock.getsockname()[1]
URL = f'http://127.0.0.1:{PORT}/'
processes = []
results = {}
# Optional stages that need extra services can be skipped while recording a
# demo on a workstation. CI must always exercise every stage.
SKIP = {item.strip() for item in os.environ.get('FIEX_CI_SKIP', '').split(',') if item.strip()}
if SKIP and os.environ.get('GITHUB_ACTIONS'):
    raise SystemExit('FIEX_CI_SKIP is not allowed in CI')
stage = 'Starting Fiex'
mouse.PAUSE = .12
# Dedicated CI desktop; the workflow timeout bounds unattended input.
mouse.FAILSAFE = False


def wait(check, timeout=40):
    end = time.monotonic() + timeout
    last = None
    while time.monotonic() < end:
        try:
            value = check()
            if value:
                return value
        except Exception as exc:
            last = exc
        time.sleep(.3)
    raise AssertionError(f'Timed out: {check}; last error: {last}')


def request(action='script', window='main', **kwargs):
    data = json.dumps(dict(action=action, window=window, **kwargs)).encode()
    req = urllib.request.Request(URL, data=data, headers={'Authorization': 'Bearer ' + TOKEN, 'Content-Type': 'application/json'})
    try:
        with urllib.request.urlopen(req, timeout=25) as response:
            return json.load(response)
    except urllib.error.HTTPError as exc:
        raise AssertionError(f'UI driver {action}: {exc.read().decode()}') from exc


def js(script, window='main'):
    response = request(script=script, window=window)
    result = response['result']
    if 'error' in result:
        raise AssertionError(result['error'])
    return result.get('value')


def press_element(expression, window='main'):
    response = request(script=f"const e={expression}; if(!e || e.disabled) throw Error('missing or disabled control'); e.scrollIntoView({{block:'nearest'}}); const r=e.getBoundingClientRect(); if(!r.width || !r.height) throw Error('hidden control'); return {{x:r.x+r.width/2,y:r.y+r.height/2}}", window=window)
    point = response['result'].get('value')
    if not point:
        raise AssertionError(response)
    mouse.click(point['x'] + response['origin']['x'], point['y'] + response['origin']['y'])
    time.sleep(.3)


def click(selector, window='main'):
    expression = f'document.querySelector({json.dumps(selector)})'
    wait(lambda: js(f'const e={expression}; return !!e && !e.disabled', window))
    press_element(expression, window)


def button(text, window='main', scope='document'):
    expression = f'Array.from({scope}.querySelectorAll("button,[role=menuitem]")).find(e=>{{const label=e.cloneNode(true);label.querySelectorAll(".shortcut").forEach(n=>n.remove());return label.textContent.trim()==={json.dumps(text)}}})'
    wait(lambda: js(f'const e={expression}; return !!e && !e.disabled', window))
    press_element(expression, window)


def fill(selector, value, window='main'):
    js(f'''const e=document.querySelector({json.dumps(selector)}); if(!e) throw Error('missing input');
const proto=e.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;
Object.getOwnPropertyDescriptor(proto,'value').set.call(e,{json.dumps(str(value))});
e.dispatchEvent(new Event('input',{{bubbles:true}}));e.dispatchEvent(new Event('change',{{bubbles:true}})); return true''', window)
    time.sleep(.15)


def navigate(path, window='main'):
    request(action='focus', window=window)
    # A freshly created window can swallow the first native click while it is
    # still being mapped, so retry until the inline path editor is really open.
    editor = 'return !!document.querySelector(\'[aria-label="文件夹路径"]\')'
    for _ in range(3):
        click('[aria-label="编辑路径"]', window)
        try:
            wait(lambda: js(editor, window), timeout=5)
            break
        except AssertionError:
            continue
    else:
        raise AssertionError('Path editor did not open')
    fill('[aria-label="文件夹路径"]', str(path), window)
    js('document.querySelector(".path-editor").requestSubmit(); return true', window)
    wait(lambda: js('return !!document.querySelector("[role=grid]") && !document.querySelector(".loading") && !document.querySelector(".path-editor")', window))
    time.sleep(.6)
    error = js('return document.querySelector(".error-banner")?.textContent || ""', window)
    if error:
        raise AssertionError(error)


def row(name):
    return f'[data-path] .file-label[title={json.dumps(name)}], [data-path] .tile-label[title={json.dumps(name)}]'


def point(selector, window='main'):
    response = request(script=f'''const e=document.querySelector({json.dumps(selector)}); if(!e) throw Error('missing drag source');
e.scrollIntoView({{block:'nearest'}});const r=e.getBoundingClientRect();return {{x:r.x+Math.min(r.width/2,35),y:r.y+r.height/2}}''', window=window)
    p = response['result'].get('value')
    if p is None:
        raise AssertionError(response)
    return p['x'] + response['origin']['x'], p['y'] + response['origin']['y']


def drag(start, end, leave=None):
    mouse.moveTo(*start, duration=.3)
    mouse.click()
    time.sleep(.3)
    mouse.mouseDown()
    mouse.moveRel(12, 0, duration=.25)
    if leave:
        mouse.moveTo(*leave, duration=1)
        time.sleep(1.8)
    mouse.moveTo(*end, duration=1.1)
    time.sleep(.8)
    mouse.mouseUp()
    time.sleep(1)


def screenshot(name):
    with mss.mss() as screen:
        frame = screen.grab(screen.monitors[1])
        image = Image.frombytes('RGB', frame.size, frame.rgb)
        image.save(OUT / name)


class Recording:
    def __init__(self):
        self.stop = threading.Event()
        self.failure = None
        self.worker = threading.Thread(target=self.capture)
        self.worker.start()

    def capture(self):
        try:
            with mss.mss() as screen:
                monitor = screen.monitors[1]
                width, height = monitor['width'] // 2 * 2, monitor['height'] // 2 * 2
                output = OUT / f'fiex-native-{SYSTEM.lower()}.mp4'
                encoder = subprocess.Popen(['ffmpeg', '-y', '-loglevel', 'error', '-f', 'rawvideo', '-pixel_format', 'rgb24', '-video_size', f'{width}x{height}', '-framerate', '8', '-i', '-', '-an', '-c:v', 'libx264', '-threads', '2', '-preset', 'veryfast', '-crf', '23', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', str(output)], stdin=subprocess.PIPE)
                try:
                    while not self.stop.is_set():
                        started = time.monotonic()
                        frame = screen.grab(monitor)
                        image = Image.frombytes('RGB', frame.size, frame.rgb).crop((0, 0, width, height))
                        draw = ImageDraw.Draw(image)
                        draw.rectangle((0, height - 28, width, height), fill='#172d40')
                        draw.text((12, height - 21), f'CI / {SYSTEM} / {stage}', fill='white')
                        px, py = mouse.position()
                        factor = width / mouse.size().width
                        draw.ellipse((px*factor-4, py*factor-4, px*factor+4, py*factor+4), fill='white', outline='#174c78')
                        encoder.stdin.write(image.tobytes())
                        self.stop.wait(max(0, .125 - (time.monotonic() - started)))
                finally:
                    encoder.stdin.close()
                    if encoder.wait(timeout=30) != 0:
                        raise AssertionError('video encoding failed')
        except Exception as error:
            self.failure = error

    def finish(self):
        self.stop.set(); self.worker.join(timeout=45)
        if self.worker.is_alive():
            raise AssertionError('recording did not finish')
        if self.failure:
            raise self.failure


def passed(name):
    results[name] = True
    (OUT / 'verified-results.json').write_text(json.dumps(results, indent=2))
    print('PASS:', name, flush=True)
    time.sleep(1)


def run(*args):
    return subprocess.check_output([str(a) for a in args], text=True).strip()


def arrange_file_manager(path, x, y, width, height):
    if SYSTEM == 'Linux':
        processes.append(subprocess.Popen(['thunar', str(path)]))
        handle = wait(lambda: run('xdotool', 'search', '--name', path.name).splitlines()[-1])
        run('xdotool', 'windowmove', handle, x, y)
        run('xdotool', 'windowsize', handle, width, height)
        run('xdotool', 'windowactivate', '--sync', handle)
    elif SYSTEM == 'Darwin':
        subprocess.check_call(['open', str(path)])
        time.sleep(2)
        script = f'tell application "Finder"\nactivate\nset bounds of front window to {{{x}, {y}, {x+width}, {y+height}}}\nend tell'
        subprocess.check_call(['osascript', '-e', script])
    else:
        import ctypes
        from ctypes import wintypes
        user = ctypes.windll.user32
        subprocess.Popen(['explorer.exe', str(path)])
        def find():
            found = []
            callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
            def visit(hwnd, _):
                title = ctypes.create_unicode_buffer(1024)
                user.GetWindowTextW(hwnd, title, len(title))
                if path.name.lower() in title.value.lower() and user.IsWindowVisible(hwnd):
                    found.append(hwnd)
                return True
            user.EnumWindows(callback_type(visit), 0)
            return found[0] if found else None
        handle = wait(find)
        user.ShowWindow(handle, 9)
        user.MoveWindow(handle, x, y, width, height, True)
        user.SetForegroundWindow(handle)
    time.sleep(1)


def configure_display():
    if mouse.size().width < 1680:
        if SYSTEM == 'Windows':
            subprocess.run(['powershell.exe', '-NoProfile', '-Command', 'Set-DisplayResolution -Width 1920 -Height 1080 -Force'], check=True)
        elif SYSTEM == 'Darwin':
            import Quartz
            display = Quartz.CGMainDisplayID()
            modes = Quartz.CGDisplayCopyAllDisplayModes(display, {Quartz.kCGDisplayShowDuplicateLowResolutionModes: True})
            choices = [mode for mode in modes if Quartz.CGDisplayModeGetWidth(mode) >= 1680 and Quartz.CGDisplayModeGetHeight(mode) >= 900]
            if not choices:
                raise AssertionError('No suitable CI display mode: ' + str(sorted({(Quartz.CGDisplayModeGetWidth(m), Quartz.CGDisplayModeGetHeight(m)) for m in modes})))
            choice = min(choices, key=lambda m: abs(Quartz.CGDisplayModeGetWidth(m)-1920)+abs(Quartz.CGDisplayModeGetHeight(m)-1080))
            if Quartz.CGDisplaySetDisplayMode(display, choice, None) != 0:
                raise AssertionError('Cannot configure the CI display')
        time.sleep(1)
    print('CI display:', mouse.size(), flush=True)
    assert mouse.size().width >= 1680 and mouse.size().height >= 700, 'CI needs room for two visible application windows'


recorder = None
try:
    configure_display()
    root = Path(tempfile.mkdtemp(prefix='fiex-ci-'))
    source, target, external = root/'source', root/'window-target', root/'external-target'
    for folder in (source, target, external): folder.mkdir()
    inside = source/'MOVE_TARGET'; inside.mkdir()
    for name in ['01-internal.txt', '02-window.txt', '03-external.txt']:
        (source/name).write_text('Fiex native drag verified: ' + name + '\n')
    env = {**os.environ, 'FIEX_CI_PORT': str(PORT), 'FIEX_CI_TOKEN': TOKEN, 'FIEX_DRAG_DIAGNOSTICS': '1'}
    log = open(OUT/'fiex.log', 'w')
    processes.append(subprocess.Popen([sys.argv[1], str(source)], env=env, stdout=log, stderr=log))
    wait(lambda: request(action='windows'))
    wait(lambda: js(f'return !!document.querySelector({json.dumps(row("01-internal.txt"))})'))
    screen_width, screen_height = mouse.size()
    sw, sh = max(820, min(1000, screen_width - 840)), min(820, screen_height - 90)
    tx, ty, tw, th = sw+10, 35, max(500, screen_width-sw-20), sh
    request(action='layout', x=0, y=35, width=sw, height=sh)
    click('[aria-label="列表视图"]')
    recorder = Recording()
    stage = 'Move into a folder without a confirmation dialog'
    drag(point(row('01-internal.txt')), point(row('MOVE_TARGET')))
    wait(lambda: (inside/'01-internal.txt').is_file())
    assert not (source/'01-internal.txt').exists()
    assert not js('return !!document.querySelector("[role=dialog]")')
    passed('internal_folder_move')
    stage = 'Bookmark a folder without moving it'
    drag(point(row('MOVE_TARGET')), point('[data-bookmark-drop]'))
    wait(lambda: js('return document.querySelector(".favorites-section")?.textContent.includes("MOVE_TARGET")'))
    assert inside.is_dir()
    passed('favorite_preserves_folder')
    click('[aria-label="切换侧栏"]')
    stage = 'Move between two Fiex windows'
    click('[aria-label="新建窗口"]')
    second = wait(lambda: next((w for w in request(action='windows') if w != 'main'), None))
    wait(lambda: js(f'return !!document.querySelector({json.dumps(row("03-external.txt"))})', second))
    request(action='layout', window=second, x=tx, y=ty, width=tw, height=th)
    navigate(target, second)
    request(action='focus')
    drag(point(row('02-window.txt')), (tx+min(340, tw-60), ty+350), (sw-1, ty+260))
    wait(lambda: (target/'02-window.txt').is_file())
    assert not (source/'02-window.txt').exists(), 'Same-app windows must move'
    passed('same_app_window_move')
    # Close the other Fiex window using its real window control.
    click('[title="关闭窗口"]', second)
    if 'file-manager' not in SKIP:
        stage = 'Copy to the OS file manager and preserve the source'
        arrange_file_manager(external, tx, ty, tw, th)
        request(action='focus')
        original = (source/'03-external.txt').read_bytes()
        drag(point(row('03-external.txt')), (tx+min(340, tw-60), ty+350), (sw-1, ty+260))
        wait(lambda: (external/'03-external.txt').is_file())
        assert (external/'03-external.txt').read_bytes() == original
        assert (source/'03-external.txt').read_bytes() == original
        passed('external_file_manager_copy')
    stage = 'Drag into Chromium and verify server-received bytes'
    upload = root/'uploaded.txt'
    browser_env = {**env, 'FIEX_CI_RECEIVER_BOUNDS': json.dumps(dict(x=tx, y=ty, width=tw, height=th))}
    processes.append(subprocess.Popen([sys.executable, str(Path(__file__).with_name('upload-target.py')), str(upload)], env=browser_env, stdout=log, stderr=log))
    time.sleep(6)
    request(action='focus')
    drag(point(row('03-external.txt')), (tx+min(340, tw-60), ty+300), (sw-1, ty+260))
    wait(lambda: upload.is_file() and upload.with_suffix('.json').is_file())
    assert upload.read_bytes() == original
    assert json.loads(upload.with_suffix('.json').read_text())['name'] == '03-external.txt'
    assert (source/'03-external.txt').read_bytes() == original
    screenshot('browser-upload.png')
    passed('browser_upload_and_source_preserved')
    stage = 'Windows-style conflict choices: skip, compare and keep both'
    request(action='focus')
    click(row('03-external.txt'))
    mouse.hotkey('command' if SYSTEM == 'Darwin' else 'ctrl', 'c')
    mouse.hotkey('command' if SYSTEM == 'Darwin' else 'ctrl', 'v')
    wait(lambda: js('return document.querySelector("[role=dialog]")?.textContent.includes("替换或跳过文件")'))
    button('跳过这些文件')
    wait(lambda: not js('return !!document.querySelector("[role=dialog]")'))
    click(row('03-external.txt'))
    mouse.hotkey('command' if SYSTEM == 'Darwin' else 'ctrl', 'v')
    button('逐个比较文件')
    screenshot('conflicts.png')
    button('继续')
    wait(lambda: (source/'03-external (2).txt').is_file())
    assert (source/'03-external (2).txt').read_bytes() == original
    passed('conflict_skip_compare_keep_both')
    stage = 'Git init, commit, remote push and fast-forward pull'
    remote = root/'remote.git'
    run('git', 'init', '--bare', '--initial-branch=main', remote)
    click('[aria-label="源代码管理"]')
    wait(lambda: js('return document.querySelector(".git-welcome")?.textContent.trim()==="初始化 Git 仓库"'))
    button('初始化 Git 仓库')
    wait(lambda: js('return !!document.querySelector(".git-repository")'))
    button('暂存全部')
    wait(lambda: js('return document.querySelector(".git-commit-action button")?.disabled===false'))
    click('.git-commit-action button')
    fill('[aria-label="提交者姓名"]', 'Fiex CI')
    fill('[aria-label="提交者邮箱"]', 'fiex-ci@example.com')
    fill('[aria-label="提交说明"]', 'Verify native operations')
    click('.git-commit-form button[type=submit]')
    wait(lambda: (source/'.git/refs/heads/main').is_file())
    click('.git-remote [aria-label="配置远程仓库"]')
    fill('[aria-label="仓库地址"]', str(remote))
    button('保存')
    button('推送')
    wait(lambda: run('git', '--git-dir', remote, 'rev-parse', 'main') == run('git', '-C', source, 'rev-parse', 'HEAD'))
    producer = root/'producer'
    run('git', 'clone', remote, producer)
    (producer/'remote.txt').write_text('remote pull verified\n')
    run('git', '-C', producer, 'add', '.')
    run('git', '-C', producer, '-c', 'user.name=Fiex CI', '-c', 'user.email=fiex-ci@example.com', 'commit', '-m', 'Remote change')
    run('git', '-C', producer, 'push')
    button('拉取')
    wait(lambda: (source/'remote.txt').is_file())
    assert (source/'remote.txt').read_text() == 'remote pull verified\n'
    passed('git_push_pull')
    click('[aria-label="关闭源代码管理"]')
    stage = 'WebDAV and SMB: connect, list, download and preview'
    click('[aria-label="切换侧栏"]')
    for protocol in ('webdav', 'smb'):
        if protocol in SKIP: continue
        click('[title="添加 WebDAV 或 SMB 连接"]')
        if protocol == 'smb': button('SMB')
        def field(label, value):
            selector = js(f'return Array.from(document.querySelectorAll(".connection-form label")).find(e=>e.querySelector("span")?.textContent==={json.dumps(label)})?.querySelector("input")?.outerHTML')
            if not selector: raise AssertionError('Missing field: ' + label)
            js(f'''const e=Array.from(document.querySelectorAll('.connection-form label')).find(e=>e.querySelector('span')?.textContent==={json.dumps(label)}).querySelector('input');
Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(e,{json.dumps(str(value))});e.dispatchEvent(new Event('input',{{bubbles:true}})); return true''')
        field('名称', protocol.upper()+' CI')
        if protocol == 'webdav': field('地址', os.environ['FIEX_WEBDAV_URL'])
        else:
            field('主机', os.environ['FIEX_SMB_HOST']); field('端口', os.environ.get('FIEX_SMB_PORT', '445'))
            field('共享名', os.environ.get('FIEX_SMB_SHARE', 'fiex')); field('用户名', os.environ['FIEX_SMB_USER']); field('密码', os.environ['FIEX_SMB_PASSWORD'])
        button('添加')
        wait(lambda: not js('return !!document.querySelector(".connection-form")'))
        wait(lambda: js(f'return !!document.querySelector({json.dumps(row("hello.txt"))})'))
        click(row('hello.txt'))
        mouse.press('space')
        wait(lambda: js(f'return document.querySelector(".modal pre")?.textContent.includes({json.dumps("hello "+protocol)})'))
        screenshot(protocol+'-preview.png')
        mouse.press('space')
        passed(protocol+'_live_download_preview')
    stage = 'Disk images: list, read, copy out and verify read-only behavior'
    images = request(action='images')
    expected = {p.stem for p in Path(os.environ['FIEX_CI_IMAGES']).glob('*.img')}
    assert {i['name'] for i in images} == expected, images
    for image in sorted(images, key=lambda i: i['name']):
        name, path = image['name'], image['path']
        stage = 'Filesystem / '+name.upper()
        navigate(path)
        wait(lambda: js(f'return !!document.querySelector({json.dumps(row("hello.txt"))})'))
        click(row('hello.txt')); mouse.press('space')
        wait(lambda: js(f'return document.querySelector(".modal pre")?.textContent.includes({json.dumps("hello "+name)})'))
        screenshot(name+'-preview.png')
        mouse.press('space')
        # Native file URLs must contain actual data from each raw filesystem.
        upload.unlink(missing_ok=True)
        upload.with_suffix('.json').unlink(missing_ok=True)
        request(action='focus')
        drag(point(row('hello.txt')), (tx+min(340, tw-60), ty+300), (sw-1, ty+260))
        wait(lambda: upload.is_file() and upload.with_suffix('.json').is_file())
        assert upload.read_text().strip() == 'hello '+name
        assert json.loads(upload.with_suffix('.json').read_text())['name'] == 'hello.txt'
        passed(name+'_native_browser_upload')
        # Use the UI clipboard to export a file through the real driver.
        click(row('hello.txt')); mouse.hotkey('command' if SYSTEM == 'Darwin' else 'ctrl', 'c')
        export = root/('export-'+name); export.mkdir()
        navigate(export)
        mouse.click(*point('[role=grid]'))
        mouse.hotkey('command' if SYSTEM == 'Darwin' else 'ctrl', 'v')
        wait(lambda: (export/'hello.txt').is_file())
        assert (export/'hello.txt').read_text().strip() == 'hello '+name
        navigate(path)
        if name in ('ntfs', 'ext4'):
            click('[aria-label="新建文件夹"]')
            wait(lambda: js('return !!document.querySelector(".inline-rename")'))
            mouse.write('CI-folder', interval=.05); mouse.press('enter')
            wait(lambda: js(f'return !!document.querySelector({json.dumps(row("CI-folder"))})'))
            navigate(export)
            click(row('hello.txt')); mouse.hotkey('command' if SYSTEM == 'Darwin' else 'ctrl', 'c')
            navigate(path)
            mouse.click(*point('[role=grid]'))
            mouse.hotkey('command' if SYSTEM == 'Darwin' else 'ctrl', 'v')
            button('逐个比较文件'); button('全部保留两者')
            wait(lambda: js(f'return !!document.querySelector({json.dumps(row("hello (2).txt"))})'))
            mouse.click(*point(row('hello (2).txt')), button='right')
            button('重新命名')
            wait(lambda: js('return !!document.querySelector(".inline-rename")'))
            fill('.inline-rename', 'written.txt'); mouse.press('enter')
            wait(lambda: js(f'return !!document.querySelector({json.dumps(row("written.txt"))})'))
            click(row('written.txt')); mouse.press('space')
            wait(lambda: js(f'return document.querySelector(".modal pre")?.textContent.includes({json.dumps("hello "+name)})'))
            screenshot(name+'-written.png'); mouse.press('space')
            mouse.click(*point(row('written.txt')), button='right')
            button('移到废纸篓')
            wait(lambda: js('return !!document.querySelector("[role=dialog]")'))
            button('移到废纸篓', scope='document.querySelector("[role=dialog]")')
            wait(lambda: not js(f'return !!document.querySelector({json.dumps(row("written.txt"))})'))

        else:
            assert js('return document.querySelector("[aria-label=新建文件夹]").disabled')
        passed(name+'_image_read_export_'+('write' if name in ('ntfs','ext4') else 'readonly'))
    stage = 'All native integrations verified'
    screenshot('completed.png')
    passed('all_integrations')
    time.sleep(2)
except Exception:
    traceback.print_exc()
    results['failure_stage'] = stage
    (OUT/'verified-results.json').write_text(json.dumps(results, indent=2))
    screenshot('failure.png')
    raise
finally:
    if recorder: recorder.finish()
    for process in reversed(processes):
        if process.poll() is None: process.terminate()
