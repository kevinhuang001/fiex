import { t } from '@/i18n'
import {
  SHORTCUT_ACTIONS,
  SHORTCUT_GROUPS,
  SHORTCUT_LABELS,
  SHORTCUT_STYLES,
  comboFromEvent,
  defaultShortcuts,
  formatShortcut,
  type ShortcutAction,
  type ShortcutMap,
} from '@/lib/shortcuts'
import type { Preferences } from '@/lib/types'
import { Keyboard, RotateCcw, X } from 'lucide-react'
import { useEffect, useState } from 'react'

export function ShortcutSettings({
  prefs,
  update,
}: {
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
}) {
  const [recording, setRecording] = useState<{ action: ShortcutAction; index: number } | null>(null)
  const [note, setNote] = useState('')
  const assign = (action: ShortcutAction, index: number, combo: string) => {
    const next = Object.fromEntries(
      SHORTCUT_ACTIONS.map((id) => [id, [...(prefs.shortcuts[id] || [])]]),
    ) as ShortcutMap
    let conflict: ShortcutAction | undefined
    for (const other of SHORTCUT_ACTIONS) {
      if (other === action) continue
      const filtered = next[other].filter((binding) => binding !== combo)
      if (filtered.length !== next[other].length) {
        conflict = other
        next[other] = filtered
      }
    }
    const bindings = next[action]
    if (index >= bindings.length) bindings.push(combo)
    else bindings[index] = combo
    next[action] = [...new Set(bindings)]
    update({ shortcuts: next })
    setNote(
      conflict
        ? t(
            '已从「{0}」移除 {1}。',
            SHORTCUT_LABELS[conflict],
            formatShortcut(combo, prefs.shortcutStyle),
          )
        : '',
    )
  }

  const remove = (action: ShortcutAction, index: number) => {
    const next = {
      ...prefs.shortcuts,
      [action]: prefs.shortcuts[action].filter((_, i) => i !== index),
    }
    update({ shortcuts: next })
    setNote('')
    setRecording(null)
  }

  useEffect(() => {
    if (!recording) return
    const handler = (event: KeyboardEvent) => {
      event.preventDefault()
      event.stopPropagation()
      if (event.key === 'Escape') {
        setRecording(null)
        return
      }
      if (
        (event.key === 'Backspace' || event.key === 'Delete') &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        !event.shiftKey
      ) {
        const bindings = prefs.shortcuts[recording.action] || []
        if (recording.index < bindings.length) remove(recording.action, recording.index)
        setRecording(null)
        return
      }
      const combo = comboFromEvent(event)
      if (!combo) return
      assign(recording.action, recording.index, combo)
      setRecording(null)
    }
    window.addEventListener('keydown', handler, true)
    return () => window.removeEventListener('keydown', handler, true)
  })

  const chooseStyle = (style: Preferences['shortcutStyle']) => {
    update({ shortcutStyle: style, shortcuts: defaultShortcuts(style) })
    setRecording(null)
    setNote('')
  }

  const styleName = SHORTCUT_STYLES.find((item) => item.id === prefs.shortcutStyle)?.name

  return (
    <>
      <div className="settings-card">
        <div className="settings-row">
          <div className="settings-row-label">
            <span className="settings-row-icon" style={{ background: '#8b5cf6' }}>
              <Keyboard size={15} />
            </span>
            {t('快捷键风格')}
          </div>
          <div className="settings-row-control">
            <div className="segmented">
              {SHORTCUT_STYLES.map((style) => (
                <button
                  key={style.id}
                  aria-pressed={prefs.shortcutStyle === style.id}
                  onClick={() => chooseStyle(style.id)}
                >
                  {style.name}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>
      {note && (
        <p className="settings-inline-error" role="status">
          {note}
        </p>
      )}
      {SHORTCUT_GROUPS.map((group) => (
        <div className="settings-card settings-shortcut-group" key={group.title}>
          <div className="settings-card-header">{group.title}</div>
          {group.actions.map((action) => {
            const bindings = prefs.shortcuts[action] || []
            const adding = recording?.action === action && recording.index >= bindings.length
            return (
              <div className="shortcut-row" key={action}>
                <span>{SHORTCUT_LABELS[action]}</span>
                <div className="shortcut-keys">
                  {bindings.map((combo, index) => {
                    const activeRecording =
                      recording?.action === action && recording.index === index
                    return (
                      <span
                        className={`shortcut-pill ${activeRecording ? 'recording' : ''}`}
                        key={index}
                      >
                        <button
                          aria-label={t('设置 {0} 快捷键', SHORTCUT_LABELS[action])}
                          onClick={() => {
                            setRecording({ action, index })
                            setNote('')
                          }}
                        >
                          {activeRecording
                            ? t('按下快捷键…')
                            : combo
                              ? formatShortcut(combo, prefs.shortcutStyle)
                              : t('未设置')}
                        </button>
                        <button
                          className="shortcut-remove"
                          aria-label={t('移除 {0} 快捷键', SHORTCUT_LABELS[action])}
                          onClick={() => remove(action, index)}
                        >
                          <X size={10} />
                        </button>
                      </span>
                    )
                  })}
                  {adding && (
                    <span className="shortcut-pill recording">
                      <button>{t('按下快捷键…')}</button>
                    </span>
                  )}
                  <button
                    className="shortcut-add"
                    title={t('添加快捷键')}
                    aria-label={t('添加 {0} 快捷键', SHORTCUT_LABELS[action])}
                    onClick={() => {
                      setRecording({ action, index: bindings.length })
                      setNote('')
                    }}
                  >
                    +
                  </button>
                </div>
              </div>
            )
          })}
        </div>
      ))}
      <div className="shortcut-footer">
        <button
          onClick={() => {
            update({ shortcuts: defaultShortcuts(prefs.shortcutStyle) })
            setRecording(null)
            setNote('')
          }}
        >
          <RotateCcw size={11} /> {t('恢复')}
          {styleName} {t('预设')}
        </button>
      </div>
    </>
  )
}
