import { describe, expect, it } from 'vitest'
import { renderToStaticMarkup } from 'react-dom/server'
import { DecorationBadges } from './DecorationBadges'

it('groups tag dots and independent connection badges without changing layout', () => {
  const html = renderToStaticMarkup(
    <DecorationBadges
      badges={[
        { text: 'A', color: '#fff', dot: true },
        { text: 'B', color: '#000', dot: true },
        { text: 'Pinned', color: 'purple', status: 'pinned', icon: 'pin' },
        { text: 'Uploaded', color: 'blue', status: 'uploaded', icon: 'cloud-upload' },
      ]}
    />,
  )
  expect(html.match(/class="tag-dots"/g)).toHaveLength(1)
  expect(html.match(/<i /g)).toHaveLength(2)
  expect(html).toContain('title="A、B"')
  expect(html.match(/class="sync-badges"/g)).toHaveLength(1)
  expect(html).toContain('sync-badge pinned')
  expect(html).toContain('sync-badge uploaded')
})

describe('decoration text', () => {
  it('escapes provider text and labels', () => {
    const html = renderToStaticMarkup(
      <DecorationBadges
        badges={[{ text: '<script>bad</script>', color: 'red', tooltip: '" onload="bad' }]}
      />,
    )
    expect(html).not.toContain('<script>')
    expect(html).toContain('&lt;script&gt;')
    expect(html).toContain('&quot;')
  })
})
