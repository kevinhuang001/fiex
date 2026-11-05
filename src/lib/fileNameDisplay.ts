const MAX_STEM_GRAPHEMES = 20
const PREFIX_GRAPHEMES = 10
const SUFFIX_GRAPHEMES = MAX_STEM_GRAPHEMES - PREFIX_GRAPHEMES - 1

const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' })

function graphemes(value: string): string[] {
  return Array.from(segmenter.segment(value), (part) => part.segment)
}

/** Keep names stable across layouts while preserving the original extension. */
export function formatDisplayFileName(name: string): string {
  const dot = name.lastIndexOf('.')
  const hasExtension = dot > 0 && dot < name.length - 1
  const stem = hasExtension ? name.slice(0, dot) : name
  const extension = hasExtension ? name.slice(dot) : ''
  const parts = graphemes(stem)
  if (parts.length <= MAX_STEM_GRAPHEMES) return name
  return `${parts.slice(0, PREFIX_GRAPHEMES).join('')}…${parts
    .slice(-SUFFIX_GRAPHEMES)
    .join('')}${extension}`
}
