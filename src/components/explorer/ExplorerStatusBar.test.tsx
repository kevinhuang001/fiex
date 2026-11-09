import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import type { AppController } from '@/hooks/useAppController'
import { ExplorerStatusBar } from './ExplorerStatusBar'

describe('ExplorerStatusBar terminal button', () => {
  it('stays enabled while browsing a virtual filesystem', () => {
    const x = {
      entries: [],
      path: 'recent://',
      searchStats: '',
      tagFilter: '',
    } as unknown as AppController['x']
    const html = renderToStaticMarkup(
      <ExplorerStatusBar
        x={x}
        chosenAll={[]}
        selectedAll={[]}
        clipboard={null}
        setClipboard={() => {}}
        workspace={{
          open: false,
          items: [],
          setOpen: () => {},
          add: () => {},
          remove: () => {},
          clear: () => {},
        }}
      />,
    )

    expect(html).toMatch(/class="statusbar-terminal"[^>]*aria-label="打开终端"/)
    expect(html).not.toMatch(/class="statusbar-terminal"[^>]*disabled/)
  })
})
