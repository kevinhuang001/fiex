import type { Ide, Preferences, Terminal } from '@/lib/types'
import { Check, Code2, HelpCircle, TerminalSquare } from 'lucide-react'

export function IntegrationSettings({
  prefs,
  update,
  ides,
  terminals,
  onHelp,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  ides: Ide[]
  terminals: Terminal[]
  onHelp: () => void
}) {
  return (
    <>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>默认 IDE</span>
          <button
            className="settings-reset"
            title="查看支持的 IDE 和终端"
            aria-label="查看支持的 IDE 和终端"
            onClick={onHelp}
          >
            <HelpCircle size={12} />
            支持列表
          </button>
        </div>
        {ides.length ? (
          ides.map((ide) => (
            <button
              key={ide.id}
              className={`settings-choice ${prefs.ide === ide.id ? 'selected' : ''}`}
              aria-pressed={prefs.ide === ide.id}
              onClick={() => update({ ide: ide.id })}
            >
              <span className="settings-choice-icon">
                <Code2 size={15} />
              </span>
              <span>{ide.name}</span>
              {prefs.ide === ide.id && <Check size={15} />}
            </button>
          ))
        ) : (
          <div className="settings-row">
            <div className="settings-row-label">没有检测到可用的 IDE</div>
          </div>
        )}
      </div>
      <div className="settings-card">
        <div className="settings-card-header">
          <span>默认终端</span>
        </div>
        {terminals.length ? (
          terminals.map((terminal) => (
            <button
              key={terminal.id}
              className={`settings-choice ${prefs.terminal === terminal.id ? 'selected' : ''}`}
              aria-pressed={prefs.terminal === terminal.id}
              onClick={() => update({ terminal: terminal.id })}
            >
              <span className="settings-choice-icon">
                <TerminalSquare size={15} />
              </span>
              <span>{terminal.name}</span>
              {prefs.terminal === terminal.id && <Check size={15} />}
            </button>
          ))
        ) : (
          <div className="settings-row">
            <div className="settings-row-label">没有检测到可用的终端应用</div>
          </div>
        )}
      </div>
    </>
  )
}
