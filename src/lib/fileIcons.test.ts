import { describe, expect, it } from 'vitest'
import { fileVisual } from './fileIcons'
import type { Entry } from './types'

function entry(name: string, isDir = false): Entry {
  return {
    path: '/tmp/' + name,
    name,
    isDir,
    isSymlink: false,
    size: 1,
    modified: 0,
    created: 0,
    extension: isDir ? '' : (name.split('.').pop() || '').toLowerCase(),
    mime: 'application/octet-stream',
    readonly: false,
    linkTarget: null,
    tags: [],
    sync: null,
  }
}

describe('file icons', () => {
  it('resolves language and specialized file icons', () => {
    expect(fileVisual(entry('main.ts')).url).toContain('typescript.svg')
    expect(fileVisual(entry('App.tsx')).url).toContain('react_ts.svg')
    expect(fileVisual(entry('main.rs')).url).toContain('rust.svg')
    expect(fileVisual(entry('server.py')).url).toContain('python.svg')
    expect(fileVisual(entry('index.d.ts')).url).toContain('typescript-def.svg')
    expect(fileVisual(entry('Dockerfile')).url).toContain('docker.svg')
    expect(fileVisual(entry('.gitignore')).url).toContain('git.svg')
    expect(fileVisual(entry('README.md')).url).toContain('readme.svg')
    expect(fileVisual(entry('photo.png')).url).toContain('image.svg')
    expect(fileVisual(entry('archive.tar.gz')).url).toContain('zip.svg')
  })
  it('uses colored folder icons and a non-grey generic fallback', () => {
    const folder = fileVisual(entry('node_modules', true))
    expect(folder.folder).toBe(true)
    expect(folder.generic).toBe(false)
    expect(folder.url).toContain('folder-node.svg')
    const generic = fileVisual(entry('Unknown Folder', true))
    expect(generic.generic).toBe(true)
    const unknown = fileVisual(entry('data.zzz'))
    expect(unknown.url).toContain('document.svg')
  })
})
