import { describe, it, expect } from 'vitest'
import { parent, join, pathCrumbs, sortEntries, groupKey, uniqueName, corePath } from './files'
import type { Entry } from './types'
const e = (name: string, isDir = false): Entry => ({
  name,
  path: '/' + name,
  isDir,
  isSymlink: false,
  size: 1,
  modified: 0,
  created: 0,
  mime: 'text/plain',
  extension: 'txt',
  readonly: false,
  linkTarget: null,
  tags: [],
  sync: null,
})
describe('file models', () => {
  it('handles roots and Windows paths', () => {
    expect(parent('/a/b')).toBe('/a')
    expect(parent('/')).toBe('/')
    expect(parent('C:\\Users')).toBe('C:\\')
    expect(join('C:\\Users', 'a')).toBe('C:\\Users\\a')
  })
  it('sorts numerically and keeps directories first', () => {
    expect(
      sortEntries([e('10'), e('2'), e('folder', true)], 'name', false).map((e) => e.name),
    ).toEqual(['folder', '2', '10'])
    expect(sortEntries([e('a'), e('z', true)], 'name', true)[0].isDir).toBe(true)
  })
  it('makes Finder-style unique names', () => {
    expect(uniqueName([], '未命名文件夹')).toBe('未命名文件夹')
    expect(uniqueName(['未命名文件夹'], '未命名文件夹')).toBe('未命名文件夹 2')
    expect(uniqueName(['未命名文件夹', '未命名文件夹 2'], '未命名文件夹')).toBe('未命名文件夹 3')
    expect(uniqueName(['未命名.txt'], '未命名.txt')).toBe('未命名 2.txt')
  })
  it('groups dates at local midnight', () => {
    const now = new Date(2026, 9, 20, 12).getTime()
    expect(groupKey({ ...e('today'), modified: now / 1000 }, 'modified', now)).toBe('今天')
    expect(
      groupKey({ ...e('yesterday'), modified: (now - 86400000) / 1000 }, 'modified', now),
    ).toBe('昨天')
  })
})

describe('pathCrumbs', () => {
  it('starts at the root segment without an empty crumb', () => {
    expect(pathCrumbs('/Users/kevin/Documents').map((crumb) => crumb.name)).toEqual([
      'Users',
      'kevin',
      'Documents',
    ])
  })

  it('keeps a virtual location as one crumb', () => {
    expect(pathCrumbs('recent://')).toEqual([{ name: 'recent://', path: 'recent://' }])
  })
})

describe('Core mount paths', () => {
  const root = 'fiex://mount/account-one/'
  it('keeps roots stable and encodes each child exactly once', () => {
    expect(parent(root)).toBe(root)
    const nested = join(join(root, '中文 #?%'), 'report.docx')
    expect(corePath(nested)?.parts).toEqual(['中文 #?%', 'report.docx'])
    expect(parent(parent(nested))).toBe(root)
    expect(pathCrumbs(nested).map((c) => c.path)).toEqual([root, parent(nested), nested])
    expect(pathCrumbs(nested).map((c) => c.name)).toEqual([
      '虚拟文件系统',
      '中文 #?%',
      'report.docx',
    ])
    expect(join('fiex://mount/account-two/', 'same')).not.toBe(join(root, 'same'))
  })
  it('rejects malformed addresses before local path manipulation', () => {
    for (const value of [
      'fiex://wrong/x/',
      root + '..',
      root + '%2e%2e',
      root + '%252e%252e',
      root + 'a%2fb',
      root + 'a%5cb',
      root + '%00',
      root + '%zz',
      root + 'x#fragment',
      root + 'a//b',
    ]) {
      expect(() => parent(value)).toThrow()
      expect(() => pathCrumbs(value)).toThrow()
    }
  })
})
