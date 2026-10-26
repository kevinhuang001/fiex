import { afterEach, describe, expect, it } from 'vitest'
import { getLocale, setLocale, subscribeLocale, t, translateMessage } from './index'
import { commonPlaces } from '@/lib/places'
import type { Location } from '@/lib/types'
afterEach(() => setLocale('zh-CN'))
describe('application languages', () => {
  it('uses all three catalogs and keeps interpolation values untouched', () => {
    expect(t('设置')).toBe('设置')
    setLocale('en')
    expect(t('设置')).toBe('Settings')
    expect(t('打开 {0}', '校验和.txt')).toBe('Open 校验和.txt')
    setLocale('zh-TW')
    expect(t('打开 {0}', '校验和.txt')).toBe('開啟 校验和.txt')
    expect(t('/Users/中文.txt')).toBe('/Users/中文.txt')
  })
  it('notifies the root once for each actual language change', () => {
    let changes = 0
    const stop = subscribeLocale(() => changes++)
    setLocale('en')
    setLocale('en')
    setLocale('zh-TW')
    expect(changes).toBe(2)
    stop()
    setLocale('zh-CN')
    expect(changes).toBe(2)
    expect(getLocale()).toBe('zh-CN')
  })
  it('localizes native errors without translating their filenames', () => {
    setLocale('en')
    expect(translateMessage('无法启动终端：中文.txt')).toBe('Cannot start terminal: 中文.txt')
  })
  it('localizes built-in places without mutating navigation identities', () => {
    const place = { name: '文稿', path: '/Users/example/Documents', kind: 'favorite' } as Location
    setLocale('en')
    expect(commonPlaces([place])[0].name).toBe('Documents')
    expect(place.name).toBe('文稿')
    expect(commonPlaces([place])[0].path).toBe(place.path)
  })
})
