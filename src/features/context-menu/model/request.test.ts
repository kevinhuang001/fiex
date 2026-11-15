import { describe, expect, it } from 'vitest'
import { contextMenuPaths, contextMenuRequestKey } from './request'

describe('context menu request model', () => {
  it('uses only the clicked path when it is outside the current selection', () => {
    expect(contextMenuPaths('/work/clicked.txt', ['/work/other.txt'])).toEqual([
      '/work/clicked.txt',
    ])
  })

  it('puts the clicked item first and keeps the other selected paths once', () => {
    expect(
      contextMenuPaths('/work/second.txt', [
        '/work/first.txt',
        '/work/second.txt',
        '/work/third.txt',
      ]),
    ).toEqual(['/work/second.txt', '/work/first.txt', '/work/third.txt'])
  })

  it('returns an empty target list when no entry was clicked', () => {
    expect(contextMenuPaths(undefined, ['/work/selected.txt'])).toEqual([])
  })

  it('keys snapshots by position, directory, targets, and locale', () => {
    const request = {
      x: 20,
      y: 40,
      directory: '/work',
      paths: ['/work/a.txt'],
      locale: 'en',
    }
    expect(contextMenuRequestKey(request)).toBe(contextMenuRequestKey({ ...request }))
    expect(contextMenuRequestKey(request)).not.toBe(
      contextMenuRequestKey({ ...request, locale: 'zh-CN' }),
    )
    expect(contextMenuRequestKey(request)).not.toBe(
      contextMenuRequestKey({ ...request, paths: ['/work/b.txt'] }),
    )
  })
})
