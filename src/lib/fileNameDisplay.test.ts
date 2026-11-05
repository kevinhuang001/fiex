import { describe, expect, it } from 'vitest'
import { formatDisplayFileName } from './fileNameDisplay'

const segment = (value: string) =>
  Array.from(
    new Intl.Segmenter(undefined, { granularity: 'grapheme' }).segment(value),
    (part) => part.segment,
  )

describe('formatDisplayFileName', () => {
  it('uses a fixed 20-grapheme stem and preserves a normal extension', () => {
    const stem = '0123456789abcdefghijK'
    expect(formatDisplayFileName(`${stem}.pdf`)).toBe('0123456789…cdefghijK.pdf')
  })

  it('keeps dotfiles without treating their leading dot as an extension', () => {
    expect(formatDisplayFileName('.gitignore')).toBe('.gitignore')
    expect(formatDisplayFileName('.'.repeat(21))).toBe(`${'.'.repeat(10)}…${'.'.repeat(9)}`)
  })

  it('does not split combining marks or emoji grapheme clusters', () => {
    const unit = '👩‍🔬e\u0301'
    const name = `${unit.repeat(12)}.gz`
    const shown = formatDisplayFileName(name)
    const stem = shown.slice(0, -'.gz'.length)
    const parts = segment(name.slice(0, -'.gz'.length))
    expect(segment(stem)).toHaveLength(20)
    expect(stem).toBe(`${parts.slice(0, 10).join('')}…${parts.slice(-9).join('')}`)
    expect(shown.endsWith('.gz')).toBe(true)
  })

  it('does not change names at or below the stem limit', () => {
    expect(formatDisplayFileName('x'.repeat(20))).toBe('x'.repeat(20))
    expect(formatDisplayFileName('README')).toBe('README')
    expect(formatDisplayFileName('archive.tar.gz')).toBe('archive.tar.gz')
  })

  it('keeps the extension even when it is longer than the truncated stem', () => {
    const name = `${'x'.repeat(25)}.${'extension'.repeat(4)}`
    expect(formatDisplayFileName(name)).toBe(`xxxxxxxxxx…xxxxxxxxx.${'extension'.repeat(4)}`)
  })

  it('treats a trailing dot as part of a name without an extension', () => {
    expect(formatDisplayFileName(`${'x'.repeat(21)}.`)).toBe(`${'x'.repeat(10)}…${'x'.repeat(8)}.`)
  })
})
