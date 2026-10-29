import { t } from '@/i18n'
import type { Preferences } from '@/lib/types'

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
  return (
    <div className="terminal-settings-menu" role="group" aria-label={t('终端设置')}>
      <label>
        <span>{t('终端字体')}</span>
        <select
          value={terminalFont || ''}
          onChange={(event) => onSettingsChange({ terminalFont: event.target.value })}
        >
          <option value="">{t('Fiex 默认（含 Nerd Font 回退）')}</option>
          {terminalFont && !fonts.includes(terminalFont) && (
            <option value={terminalFont}>{terminalFont}</option>
          )}
          {fonts.map((font) => (
            <option key={font} value={font}>
              {font}
            </option>
          ))}
        </select>
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
