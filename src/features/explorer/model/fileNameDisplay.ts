const MAX_STEM_WIDTH = 30
const PREFIX_WIDTH = 14
const SUFFIX_WIDTH = MAX_STEM_WIDTH - PREFIX_WIDTH - 1

const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' })

function graphemes(value: string): string[] {
  return Array.from(segmenter.segment(value), (part) => part.segment)
}

function isWide(codePoint: number): boolean {
  return (
    codePoint === 0x2329 ||
    codePoint === 0x232a ||
    (codePoint >= 0x1100 && codePoint <= 0x115f) ||
    (codePoint >= 0x2e80 && codePoint <= 0xa4cf) ||
    (codePoint >= 0x3040 && codePoint <= 0x30ff) ||
    (codePoint >= 0x3100 && codePoint <= 0x312f) ||
    (codePoint >= 0x3130 && codePoint <= 0x318f) ||
    (codePoint >= 0x31a0 && codePoint <= 0x31bf) ||
    (codePoint >= 0x31f0 && codePoint <= 0x31ff) ||
    (codePoint >= 0xac00 && codePoint <= 0xd7a3) ||
    (codePoint >= 0xd7b0 && codePoint <= 0xd7ff) ||
    (codePoint >= 0xf900 && codePoint <= 0xfaff) ||
    (codePoint >= 0xfe10 && codePoint <= 0xfe6f) ||
    (codePoint >= 0xff00 && codePoint <= 0xff60) ||
    (codePoint >= 0xffe0 && codePoint <= 0xffe6) ||
    (codePoint >= 0x1f000 && codePoint <= 0x1faff) ||
    (codePoint >= 0x20000 && codePoint <= 0x3fffd)
  )
}

function graphemeWidth(value: string): number {
  return Array.from(value).some((character) => {
    const codePoint = character.codePointAt(0)!
    return isWide(codePoint) || (codePoint >= 0x2600 && codePoint <= 0x27bf)
  })
    ? 2
    : 1
}

function takeWidth(parts: string[], budget: number, fromEnd = false): string[] {
  const chosen: string[] = []
  let width = 0
  const candidates = fromEnd ? [...parts].reverse() : parts
  for (const part of candidates) {
    const nextWidth = graphemeWidth(part)
    if (width + nextWidth > budget) break
    chosen.push(part)
    width += nextWidth
  }
  return fromEnd ? chosen.reverse() : chosen
}

/** Keep names stable across layouts while preserving the original extension. */
export function formatDisplayFileName(name: string): string {
  const dot = name.lastIndexOf('.')
  const hasExtension = dot > 0 && dot < name.length - 1
  const stem = hasExtension ? name.slice(0, dot) : name
  const extension = hasExtension ? name.slice(dot) : ''
  const serial = stem.match(/ \(\d+\)$/)?.[0] ?? ''
  const numberedStem = serial ? stem.slice(0, -serial.length) : stem
  const parts = graphemes(numberedStem)
  if (parts.reduce((total, part) => total + graphemeWidth(part), 0) <= MAX_STEM_WIDTH) return name
  const prefix = takeWidth(parts, PREFIX_WIDTH)
  const suffix = takeWidth(parts.slice(prefix.length), SUFFIX_WIDTH, true)
  return `${prefix.join('')}…${suffix.join('')}${serial}${extension}`
}
