import { describe, it, expect } from 'vitest'
import {
  parent,
  join,
  basename,
  displayAddress,
  normalizeAddress,
  pathCrumbs,
  sortEntries,
  nextSort,
  groupKey,
  uniqueName,
  corePath,
} from './files'
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
  it('starts a new header sort ascending and flips the active one', () => {
    expect(nextSort('name', false, 'size')).toEqual({ sort: 'size', descending: false })
    // Re-clicking a different column always starts ascending, even if the
    // previous column was descending.
    expect(nextSort('name', true, 'size')).toEqual({ sort: 'size', descending: false })
    expect(nextSort('size', false, 'size')).toEqual({ sort: 'size', descending: true })
    expect(nextSort('size', true, 'size')).toEqual({ sort: 'size', descending: false })
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
    expect(pathCrumbs('recent://').map((crumb) => crumb.path)).toEqual(['recent://'])
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

describe('virtual addresses', () => {
  it('normalizes roots, aliases and paths without touching local paths', () => {
    for (const input of ['recent', 'recent:', 'recent:/', 'recent://'])
      expect(normalizeAddress(input)).toBe('recent://')
    for (const input of ['applications', 'applications:', 'applications:/', 'applications://'])
      expect(normalizeAddress(input)).toBe('applications://')
    for (const input of ['trash', 'trash:', 'trash:/', 'trash://'])
      expect(normalizeAddress(input)).toBe('trash://')
    expect(normalizeAddress('回收站')).toBe('recyclebin://')
    expect(normalizeAddress('废纸篓')).toBe('trash://')
    expect(normalizeAddress('C:\\Users')).toBe('C:\\Users')
    expect(normalizeAddress('fiex://mount/account/')).toBe('fiex://mount/account/')
    for (const [input, expected] of [
      ['recent:\\', 'recent://'],
      ['applications:\\', 'applications://'],
      ['trash:\\', 'trash://'],
    ])
      expect(normalizeAddress(input)).toBe(expected)
    expect(normalizeAddress('applications:\\未命名文件夹')).toBe(
      normalizeAddress('applications://未命名文件夹'),
    )
    expect(displayAddress(normalizeAddress('applications:\\工作\\文档'))).toBe(
      'applications://工作/文档',
    )
  })

  it('shows Chinese folder names while keeping one opaque segment', () => {
    const encoded = normalizeAddress('applications://未命名文件夹')
    expect(encoded.startsWith('applications://~')).toBe(true)
    expect(encoded).not.toContain('/user/')
    expect(displayAddress(encoded)).toBe('applications://未命名文件夹')
    expect(basename(encoded)).toBe('未命名文件夹')
    expect(parent(encoded)).toBe('applications://')
    expect(pathCrumbs(encoded).map((crumb) => crumb.path)).toEqual(['applications://', encoded])
  })

  it('keeps breadcrumb parents virtual instead of producing applications:\\', () => {
    expect(pathCrumbs('applications://')[0].path).toBe('applications://')
    expect(pathCrumbs('trash://')[0].path).toBe('trash://')
    const trash = normalizeAddress('trash://Old draft.txt')
    expect(displayAddress(trash)).toBe('trash://Old draft.txt')
    expect(pathCrumbs(trash)[0].path).toBe('trash://')
  })

  it('accepts legacy user prefixes and canonicalizes them', () => {
    expect(normalizeAddress('applications://user/5pyq5ZG95ZCN5paH5Lu25aS5')).toBe(
      'applications://~5pyq5ZG95ZCN5paH5Lu25aS5',
    )
  })
})
