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
        for name, megabytes in [('ntfs', 64), ('ext4', 64), ('btrfs', 256)]:
            source = work/name
            source.mkdir()
            (source/'hello.txt').write_text('hello '+name+'\n')
            image = out/(name+'.img')
            with image.open('wb') as stream:
                stream.truncate(megabytes*1024*1024)
            if name == 'ntfs':
                run('mkntfs', '-F', '-Q', '-L', 'FIEX', image)
                run('ntfscp', image, source/'hello.txt', '/hello.txt')
            elif name == 'ext4':
                # Disable features not implemented by the embedded driver yet.
                run('mkfs.ext4', '-F', '-O', '^metadata_csum,^64bit,^orphan_file', '-d', source, image)
            else:
                run('mkfs.btrfs', '-f', '--rootdir', source, image)
    else:
        for name, fs in [('apfs', 'APFS'), ('hfs', 'HFS+')]:
            image = work/(name+'.dmg')
            mount = work/('mount-'+name)
            mount.mkdir()
            run('hdiutil', 'create', '-size', '96m', '-fs', fs, '-type', 'UDRW', '-layout', 'NONE', '-volname', 'Fiex-'+name, image)
            run('hdiutil', 'attach', '-nobrowse', '-mountpoint', mount, image)
            try:
                (mount/'hello.txt').write_text('hello '+name+'\n')
            finally:
                run('hdiutil', 'detach', mount)
            (out/(name+'.img')).write_bytes(image.read_bytes())
