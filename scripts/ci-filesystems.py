#!/usr/bin/env python3
"""Create real, populated disk images for the cross-platform GUI tests."""
from pathlib import Path
import platform
import subprocess
import tempfile

out = Path('.ci/filesystems').resolve()
out.mkdir(parents=True, exist_ok=True)

def run(*args):
    subprocess.run([str(a) for a in args], check=True)

with tempfile.TemporaryDirectory(prefix='fiex-images-') as work:
    work = Path(work)
    if platform.system() == 'Linux':
        name = 'btrfs'
        source = work/name
        source.mkdir()
        (source/'hello.txt').write_text('hello '+name+'\n')
        image = out/(name+'.img')
        with image.open('wb') as stream:
            stream.truncate(256*1024*1024)
        run('mkfs.btrfs', '-f', '--rootdir', source, image)
    else:
        for name, fs in [('apfs', 'APFS'), ('hfs', 'HFS+')]:
            image = work/(name+'.dmg')
            mount = work/('mount-'+name)
            mount.mkdir()
            run('hdiutil', 'create', '-size', '96m', '-fs', fs, '-type', 'UDIF', '-layout', 'NONE', '-volname', 'Fiex-'+name, image)
            run('hdiutil', 'attach', '-nobrowse', '-mountpoint', mount, image)
            try:
                (mount/'hello.txt').write_text('hello '+name+'\n')
            finally:
                run('hdiutil', 'detach', mount)
            (out/(name+'.img')).write_bytes(image.read_bytes())
