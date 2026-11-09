import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { TerminalAppearanceMenu } from './TerminalAppearanceMenu'

describe('TerminalAppearanceMenu', () => {
  it('uses an in-app font picker with the bundled Nerd Font selected by default', () => {
    const html = renderToStaticMarkup(
      <TerminalAppearanceMenu fonts={['Consolas']} onSettingsChange={() => {}} />,
    )

    expect(html).not.toContain('<select')
    expect(html).toContain('aria-haspopup="listbox"')
    expect(html).toContain('JetBrainsMono Nerd Font Mono')
    expect(html).toContain('aria-expanded="false"')
  })
})
