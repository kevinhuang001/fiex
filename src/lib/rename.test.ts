import { describe, expect, it } from 'vitest'
import { buildRenames, DEFAULT_RENAME_OPTIONS, splitExtension, validateRenames } from './rename'
import type { Entry } from './types'

const entry = (name: string, modified = 0): Entry => ({
  name,
  path: `/tmp/${name}`,
  isDir: false,
  size: 1,
  modified,
  created: 0,
  extension: name.split('.').pop() || '',
  mime: '',
  isSymlink: false,
  readonly: false,
  linkTarget: null,
  tags: [],
  sync: null,
})

describe('batch rename engine', () => {
  it('splits extension and preserves it when the template has no extension token', () => {
    expect(splitExtension('photo.jpg', false)).toEqual({ stem: 'photo', ext: '.jpg' })
    expect(
      buildRenames([entry('a.txt')], { ...DEFAULT_RENAME_OPTIONS, template: '{n} - {original}' }),
    ).toEqual(['1 - a.txt'])
  })

  it('supports regex replace, case, extension and sequence options', () => {
    const names = buildRenames([entry('IMG_001.JPG'), entry('IMG_002.JPG')], {
      ...DEFAULT_RENAME_OPTIONS,
      find: 'IMG_(\\d+)',
      replace: 'photo-$1',
      regex: true,
      caseMode: 'lower',
      extensionMode: 'lower',
      start: 10,
      step: 5,
      padding: 3,
      template: '{name}_{n}',
    })
    expect(names).toEqual(['photo-001_010.jpg', 'photo-002_015.jpg'])
  })

  it('uses date, folder and explicit sequence widths', () => {
    const names = buildRenames([entry('a.txt', 1710000000)], {
      ...DEFAULT_RENAME_OPTIONS,
      template: '{folder}_{date}_{n3}',
    })
    expect(names[0]).toMatch(/^tmp_\d{4}-\d{2}-\d{2}_001\.txt$/)
  })

  it('rejects empty, duplicate and Windows-reserved names', () => {
    const values = [entry('a.txt'), entry('b.txt'), entry('c.txt')]
    const problems = validateRenames(values, ['x.txt', 'x.txt', 'CON.txt'])
    expect(problems.get('/tmp/a.txt')).toContain('重复')
    expect(problems.get('/tmp/b.txt')).toContain('重复')
    expect(problems.get('/tmp/c.txt')).toContain('保留')
  })
})
