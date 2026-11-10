import { describe, expect, it } from 'vitest'
import { formatDisplayFileName } from './fileNameDisplay'

const segment = (value: string) =>
  Array.from(
    new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(value),
    (part) => part.segment,
  )

describe('formatDisplayFileName', () => {
  it('uses a 30-unit stem and preserves a normal extension', () => {
    const stem = 'x'.repeat(31)
    expect(formatDisplayFileName(`${stem}.pdf`)).toBe(`${'x'.repeat(14)}…${'x'.repeat(15)}.pdf`)
  })

  it('keeps dotfiles without treating their leading dot as an extension', () => {
    expect(formatDisplayFileName('.gitignore')).toBe('.gitignore')
    expect(formatDisplayFileName('.'.repeat(31))).toBe(`${'.'.repeat(14)}…${'.'.repeat(15)}`)
  })

  it('does not split combining marks or emoji grapheme clusters', () => {
    const unit = '👩‍🔬e\u0301'
    const name = `${unit.repeat(12)}.gz`
    const shown = formatDisplayFileName(name)
    const stem = shown.slice(0, -'.gz'.length)
    const parts = segment(name.slice(0, -'.gz'.length))
    expect(stem).toBe(`${parts.slice(0, 9).join('')}…${parts.slice(-10).join('')}`)
    expect(shown.endsWith('.gz')).toBe(true)
  })

  it('does not change names at or below the stem limit', () => {
    expect(formatDisplayFileName('x'.repeat(30))).toBe('x'.repeat(30))
    expect(formatDisplayFileName('README')).toBe('README')
    expect(formatDisplayFileName('archive.tar.gz')).toBe('archive.tar.gz')
    expect(formatDisplayFileName(`${'中'.repeat(15)}.txt`)).toBe(`${'中'.repeat(15)}.txt`)
    expect(formatDisplayFileName(`${'x'.repeat(31)} (2).txt`)).toBe(
      `${'x'.repeat(14)}…${'x'.repeat(15)} (2).txt`,
    )
  })

  it('counts Japanese kana as two display units', () => {
    expect(formatDisplayFileName(`${'カ'.repeat(16)}.txt`)).toBe(
      `${'カ'.repeat(7)}…${'カ'.repeat(7)}.txt`,
    )
  })

  it('keeps the extension even when it is longer than the truncated stem', () => {
    const extension = `.${'extension'.repeat(4)}`
    const name = `${'x'.repeat(31)}${extension}`
    expect(formatDisplayFileName(name)).toBe(`${'x'.repeat(14)}…${'x'.repeat(15)}${extension}`)
  })

  it('treats a trailing dot as part of a name without an extension', () => {
    expect(formatDisplayFileName(`${'x'.repeat(31)}.`)).toBe(`${'x'.repeat(14)}…${'x'.repeat(14)}.`)
  })
})
