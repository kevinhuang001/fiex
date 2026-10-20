import { useEffect, useState } from 'react'
import {
  Sun,
  Eye,
  SlidersHorizontal,
  TerminalSquare,
  ChevronDown,
  ExternalLink,
  Keyboard,
  RotateCcw,
  X,
} from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { Menu, Choice } from './Menus'
import { call } from '@/lib/api'
import {
  SHORTCUT_ACTIONS,
  SHORTCUT_GROUPS,
  SHORTCUT_LABELS,
  SHORTCUT_STYLES,
  comboFromEvent,
  defaultShortcuts,
  formatShortcut,
  sameShortcuts,
  type ShortcutAction,
  type ShortcutMap,
} from '@/lib/shortcuts'
import type { Preferences, Terminal } from '@/lib/types'

interface Props {
  open: boolean
  onClose: () => void
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  terminals: Terminal[]
  onError: (s: string) => void
}

export function PreferencesDialog({ open, onClose, prefs, update, terminals, onError }: Props) {
  const [recording, setRecording] = useState<{ action: ShortcutAction; index: number } | null>(null)
  const [note, setNote] = useState('')

  useEffect(() => {
    if (!open) {
      setRecording(null)
      setNote('')
    }
  }, [open])

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
        ? `已从「${SHORTCUT_LABELS[conflict]}」移除 ${formatShortcut(combo, prefs.shortcutStyle)}。`
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

  const custom = !sameShortcuts(prefs.shortcuts, defaultShortcuts(prefs.shortcutStyle))

  return (
    <Modal open={open} onClose={onClose} title="设置" className="settings-modal">
      <div className="settings">
        <div className="setting-row">
          <span>
            <Sun size={17} />
            外观
          </span>
          <div className="segmented">
            {(['system', 'light', 'dark'] as const).map((t, i) => (
              <button key={t} aria-pressed={prefs.theme === t} onClick={() => update({ theme: t })}>
                {['跟随系统', '浅色', '深色'][i]}
              </button>
            ))}
          </div>
        </div>
        <div className="setting-row">
          <span>
            <Eye size={17} />
            显示隐藏文件
          </span>
          <button
            role="switch"
            aria-checked={prefs.hidden}
            className="switch"
            onClick={() => update({ hidden: !prefs.hidden })}
          >
            <i />
          </button>
        </div>
        <div className="setting-row">
          <span>
            <SlidersHorizontal size={17} />
            图标大小
          </span>
          <input
            aria-label="图标大小"
            type="range"
            min="48"
            max="136"
            step="8"
            value={prefs.iconSize}
            onChange={(e) => update({ iconSize: +e.target.value })}
          />
        </div>
        <div className="setting-row">
          <span>
            <TerminalSquare size={17} />
            默认终端
          </span>
          <Menu label="默认终端" icon={<ChevronDown />}>
            {terminals.map((t) => (
              <Choice
                key={t.id}
                checked={prefs.terminal === t.id}
                onClick={() => update({ terminal: t.id })}
              >
                {t.name}
              </Choice>
            ))}
          </Menu>
        </div>
        <p className="settings-note">
          {terminals.find((t) => t.id === prefs.terminal)?.name || '系统默认终端'}
        </p>

        <div className="shortcut-editor">
          <div className="setting-row">
            <span>
              <Keyboard size={17} />
              快捷键风格
            </span>
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
          <p className="settings-note">切换风格会重置全部快捷键，也可以在下方单独修改。</p>
          {note && (
            <p className="shortcut-help error" role="status">
              {note}
            </p>
          )}
          {SHORTCUT_GROUPS.map((group) => (
            <div className="shortcut-group" key={group.title}>
              <h4>{group.title}</h4>
              {group.actions.map((action) => {
                const bindings = prefs.shortcuts[action] || []
                const adding = recording?.action === action && recording.index >= bindings.length
                return (
                  <div className="shortcut-row" key={action}>
                    <span>{SHORTCUT_LABELS[action]}</span>
                    <div className="shortcut-keys">
                      {bindings.map((combo, index) => {
                        const active = recording?.action === action && recording.index === index
                        return (
                          <span
                            className={`shortcut-pill ${active ? 'recording' : ''}`}
                            key={index}
                          >
                            <button
                              aria-label={`设置 ${SHORTCUT_LABELS[action]} 快捷键`}
                              onClick={() => {
                                setRecording({ action, index })
                                setNote('')
                              }}
                            >
                              {active
                                ? '按下快捷键…'
                                : combo
                                  ? formatShortcut(combo, prefs.shortcutStyle)
                                  : '未设置'}
                            </button>
                            <button
                              className="shortcut-remove"
                              aria-label={`移除 ${SHORTCUT_LABELS[action]} 快捷键`}
                              onClick={() => remove(action, index)}
                            >
                              <X size={10} />
                            </button>
                          </span>
                        )
                      })}
                      {adding && (
                        <span className="shortcut-pill recording">
                          <button>按下快捷键…</button>
                        </span>
                      )}
                      <button
                        className="shortcut-add"
                        title="添加快捷键"
                        aria-label={`添加 ${SHORTCUT_LABELS[action]} 快捷键`}
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
          <p className="shortcut-help">
            点击快捷键后按下新的组合键；Backspace 清除，Escape 取消。多个快捷键会依次显示在菜单中。
          </p>
          <div className="shortcut-footer">
            <button
              onClick={() => {
                update({ shortcuts: defaultShortcuts(prefs.shortcutStyle) })
                setRecording(null)
                setNote('')
              }}
            >
              <RotateCcw size={11} /> 恢复{' '}
              {SHORTCUT_STYLES.find((s) => s.id === prefs.shortcutStyle)?.name} 预设
              {custom ? '（当前已自定义）' : ''}
            </button>
          </div>
        </div>

        <div className="about">
          <img src="/fiex.svg" alt="" />
          <strong>
            Fiex <small>0.2.0</small>
          </strong>
          <span>File explorer, with room to breathe.</span>
          <Button
            variant="secondary"
            onClick={() =>
              void call('open_path', { path: 'https://github.com/kevinhuang001/fiex' }).catch((e) =>
                onError(String(e)),
              )
            }
          >
            GitHub
            <ExternalLink />
          </Button>
        </div>
      </div>
    </Modal>
  )
}
