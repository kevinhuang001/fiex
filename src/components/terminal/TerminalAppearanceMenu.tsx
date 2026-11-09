import { t } from '@/i18n'
import type { Preferences } from '@/lib/types'
import { BUILTIN_TERMINAL_FONT } from '@/lib/terminalFonts'
import { Check, ChevronDown } from 'lucide-react'
import { useState } from 'react'

export function TerminalAppearanceMenu({
  terminalFont,
  terminalFontSize,
  fonts,
  onSettingsChange,
}: {
  terminalFont?: string
  terminalFontSize?: number
  fonts: string[]
  onSettingsChange: (patch: Partial<Preferences>) => void
}) {
  const [fontMenuOpen, setFontMenuOpen] = useState(false)
  const fontOptions = [
    ...new Set([BUILTIN_TERMINAL_FONT, ...fonts, ...(terminalFont ? [terminalFont] : [])]),
  ]
  const fontLabel = (font: string) =>
    font === BUILTIN_TERMINAL_FONT ? `${font} · ${t('内置')}` : font
  return (
    <div className="terminal-settings-menu" role="group" aria-label={t('终端设置')}>
      <label>
        <span>{t('终端字体')}</span>
        <div className="terminal-font-picker">
          <button
            type="button"
            className="terminal-font-picker-trigger"
            aria-haspopup="listbox"
            aria-expanded={fontMenuOpen}
            onClick={() => setFontMenuOpen((open) => !open)}
          >
            <span>{fontLabel(terminalFont || BUILTIN_TERMINAL_FONT)}</span>
            <ChevronDown size={13} />
          </button>
          {fontMenuOpen && (
            <div className="terminal-font-options" role="listbox" aria-label={t('终端字体')}>
              {fontOptions.map((font) => (
                <button
                  key={font}
                  type="button"
                  role="option"
                  aria-selected={(terminalFont || BUILTIN_TERMINAL_FONT) === font}
                  onClick={() => {
                    onSettingsChange({ terminalFont: font })
                    setFontMenuOpen(false)
                  }}
                >
                  <span>{fontLabel(font)}</span>
                  {(terminalFont || BUILTIN_TERMINAL_FONT) === font && <Check size={13} />}
                </button>
              ))}
            </div>
          )}
        </div>
      </label>
      <label>
        <span>
          {t('终端字号')} · {terminalFontSize ?? 13}px
        </span>
        <input
          type="range"
          min="9"
          max="24"
          value={terminalFontSize ?? 13}
          onChange={(event) => onSettingsChange({ terminalFontSize: Number(event.target.value) })}
        />
      </label>
    </div>
  )
}
