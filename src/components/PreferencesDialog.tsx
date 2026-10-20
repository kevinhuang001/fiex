import { useEffect, useMemo, useState } from 'react'
import {
  Check,
  ExternalLink,
  Eye,
  Filter,
  Info,
  Keyboard,
  Palette,
  Plus,
  RotateCcw,
  Scale,
  Search,
  SlidersHorizontal,
  Sun,
  TerminalSquare,
  X,
  type LucideIcon,
} from 'lucide-react'
import { Modal } from './ui/Modal'
import { Button } from './ui/button'
import { call } from '@/lib/api'
import { defaultDisplayIgnore, defaultOperationIgnore } from '@/lib/ignores'
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
import type { Preferences, Terminal } from '@/lib/types'

interface Props {
  open: boolean
  onClose: () => void
  prefs: Preferences
  update: (p: Partial<Preferences>) => void
  terminals: Terminal[]
  onError: (s: string) => void
}

type SectionId = 'appearance' | 'shortcuts' | 'ignore' | 'terminal' | 'about'

interface Section {
  id: SectionId
  label: string
  icon: LucideIcon
  color: string
  keywords: string[]
}

const sections: Section[] = [
  {
    id: 'appearance',
    label: '外观',
    icon: Palette,
    color: '#3b82f6',
    keywords: ['theme', 'dark', 'light', '主题', '深色', '浅色', '图标'],
  },
  {
    id: 'shortcuts',
    label: '快捷键',
    icon: Keyboard,
    color: '#8b5cf6',
    keywords: ['keyboard', 'shortcut', '快捷键', '按键'],
  },
  {
    id: 'ignore',
    label: '忽略',
    icon: Filter,
    color: '#f59e0b',
    keywords: ['ignore', '忽略', 'node_modules', '.ds_store', '压缩', '复制'],
  },
  {
    id: 'terminal',
    label: '终端',
    icon: TerminalSquare,
    color: '#10b981',
    keywords: ['terminal', 'shell', '终端', '命令行'],
  },
  {
    id: 'about',
    label: '关于',
    icon: Info,
    color: '#64748b',
    keywords: ['about', 'version', 'license', '关于', '版本', '许可'],
  },
]

function IgnoreEditor({
  title,
  patterns,
  onChange,
  onReset,
}: {
  title: string
  patterns: string[]
  onChange: (patterns: string[]) => void
  onReset: () => void
}) {
  const [draft, setDraft] = useState('')
  const add = () => {
    const pattern = draft.trim()
    if (!pattern) return
    if (!patterns.includes(pattern)) onChange([...patterns, pattern])
    setDraft('')
  }
  return (
    <div className="settings-card ignore-card">
      <div className="settings-card-header">
        <span>
          {title}
          <small className="ignore-count">{patterns.length}</small>
        </span>
        <button className="settings-reset" onClick={onReset}>
          <RotateCcw size={11} />
          恢复预设
        </button>
      </div>
      <div className="ignore-chips">
        {patterns.map((pattern) => (
          <span className="ignore-chip" key={pattern}>
            <code>{pattern}</code>
            <button
              aria-label={`删除 ${pattern}`}
              onClick={() => onChange(patterns.filter((item) => item !== pattern))}
            >
              <X size={10} />
            </button>
          </span>
        ))}
      </div>
      <div className="ignore-add">
        <input
          value={draft}
          placeholder="添加规则，例如 node_modules 或 build/*"
          aria-label={`添加${title}规则`}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              add()
            }
          }}
        />
        <button aria-label={`添加${title}规则`} disabled={!draft.trim()} onClick={add}>
          <Plus size={14} />
        </button>
      </div>
    </div>
  )
}

export function PreferencesDialog({ open, onClose, prefs, update, terminals, onError }: Props) {
  const [section, setSection] = useState<SectionId>('appearance')
  const [query, setQuery] = useState('')
  const [recording, setRecording] = useState<{ action: ShortcutAction; index: number } | null>(null)
  const [note, setNote] = useState('')

  useEffect(() => {
    if (!open) {
      setRecording(null)
      setNote('')
      setQuery('')
      setSection('appearance')
    }
  }, [open])

  const visible = useMemo(() => {
    const term = query.trim().toLowerCase()
    if (!term) return sections
    return sections.filter(
      (item) =>
        item.label.toLowerCase().includes(term) ||
        item.keywords.some((keyword) => keyword.toLowerCase().includes(term)),
    )
  }, [query])

  useEffect(() => {
    if (query && visible.length && !visible.some((item) => item.id === section))
      setSection(visible[0].id)
  }, [query, visible, section])

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

  const openLink = (url: string) =>
    void call('open_path', { path: url }).catch((error) => onError(String(error)))

  const styleName = SHORTCUT_STYLES.find((item) => item.id === prefs.shortcutStyle)?.name

  return (
    <Modal open={open} onClose={onClose} title="设置" className="settings-modal" bare>
      <div className="settings">
        <aside className="settings-sidebar">
          <div className="settings-profile">
            <img src="/fiex.png" alt="" />
            <div>
              <strong>Fiex</strong>
              <span>文件浏览器</span>
            </div>
          </div>
          <label className="settings-search">
            <Search size={13} />
            <input
              aria-label="搜索设置"
              placeholder="搜索设置"
              value={query}
              autoFocus
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape') {
                  event.stopPropagation()
                  setQuery('')
                }
              }}
            />
          </label>
          <nav className="settings-nav-list" aria-label="设置分类">
            {visible.map((item) => {
              const Icon = item.icon
              return (
                <button
                  key={item.id}
                  className={`settings-nav ${section === item.id ? 'active' : ''}`}
                  aria-current={section === item.id ? 'page' : undefined}
                  onClick={() => setSection(item.id)}
                >
                  <span
                    className="settings-nav-icon"
                    style={{ background: section === item.id ? '#ffffff2e' : item.color }}
                  >
                    <Icon size={15} />
                  </span>
                  <span className="settings-nav-label">{item.label}</span>
                </button>
              )
            })}
            {visible.length === 0 && <p className="settings-empty">没有匹配的设置</p>}
          </nav>
          <div className="settings-sidebar-footer">
            <span>Fiex</span>
            <span>0.2.4</span>
          </div>
        </aside>

        <section className="settings-content">
          {section === 'appearance' && (
            <>
              <div className="settings-card">
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span className="settings-row-icon" style={{ background: '#3b82f6' }}>
                      <Sun size={15} />
                    </span>
                    外观
                  </div>
                  <div className="settings-row-control">
                    <div className="segmented">
                      {(['system', 'light', 'dark'] as const).map((theme, index) => (
                        <button
                          key={theme}
                          aria-pressed={prefs.theme === theme}
                          onClick={() => update({ theme })}
                        >
                          {['跟随系统', '浅色', '深色'][index]}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span className="settings-row-icon" style={{ background: '#64748b' }}>
                      <Eye size={15} />
                    </span>
                    显示隐藏文件
                  </div>
                  <button
                    role="switch"
                    aria-checked={prefs.hidden}
                    className="switch"
                    onClick={() => update({ hidden: !prefs.hidden })}
                  >
                    <i />
                  </button>
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span className="settings-row-icon" style={{ background: '#6366f1' }}>
                      <SlidersHorizontal size={15} />
                    </span>
                    字体大小
                  </div>
                  <div className="settings-row-control settings-slider">
                    <input
                      aria-label="字体大小"
                      type="range"
                      min="8"
                      max="18"
                      step="1"
                      value={prefs.fontSize}
                      onChange={(event) => update({ fontSize: +event.target.value })}
                    />
                    <span>{prefs.fontSize}px</span>
                  </div>
                </div>
              </div>
            </>
          )}

          {section === 'shortcuts' && (
            <>
              <div className="settings-card">
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span className="settings-row-icon" style={{ background: '#8b5cf6' }}>
                      <Keyboard size={15} />
                    </span>
                    快捷键风格
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
                    const adding =
                      recording?.action === action && recording.index >= bindings.length
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
                                  aria-label={`设置 ${SHORTCUT_LABELS[action]} 快捷键`}
                                  onClick={() => {
                                    setRecording({ action, index })
                                    setNote('')
                                  }}
                                >
                                  {activeRecording
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
              <div className="shortcut-footer">
                <button
                  onClick={() => {
                    update({ shortcuts: defaultShortcuts(prefs.shortcutStyle) })
                    setRecording(null)
                    setNote('')
                  }}
                >
                  <RotateCcw size={11} /> 恢复 {styleName} 预设
                </button>
              </div>
            </>
          )}

          {section === 'ignore' && (
            <>
              {(
                [
                  {
                    key: 'displayIgnore',
                    title: '显示忽略',
                    reset: defaultDisplayIgnore,
                  },
                  {
                    key: 'compressIgnore',
                    title: '压缩忽略',
                    reset: defaultOperationIgnore,
                  },
                  {
                    key: 'copyIgnore',
                    title: '复制 / 移动忽略',
                    reset: defaultOperationIgnore,
                  },
                ] as const
              ).map(({ key, title, reset }) => (
                <IgnoreEditor
                  key={key}
                  title={title}
                  patterns={prefs[key]}
                  onChange={(patterns) => update({ [key]: patterns } as Partial<Preferences>)}
                  onReset={() => update({ [key]: reset() } as Partial<Preferences>)}
                />
              ))}
            </>
          )}

          {section === 'terminal' && (
            <>
              <div className="settings-card">
                {terminals.length ? (
                  terminals.map((terminal) => (
                    <button
                      key={terminal.id}
                      className={`settings-choice ${
                        prefs.terminal === terminal.id ? 'selected' : ''
                      }`}
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
          )}

          {section === 'about' && (
            <>
              <div className="settings-about">
                <img src="/fiex.png" alt="" />
                <h3>Fiex</h3>
                <p>File explorer, with room to breathe.</p>
                <Button
                  variant="secondary"
                  onClick={() => openLink('https://github.com/kevinhuang001/fiex')}
                >
                  GitHub
                  <ExternalLink />
                </Button>
              </div>
              <div className="settings-card">
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span className="settings-row-icon" style={{ background: '#64748b' }}>
                      <Info size={15} />
                    </span>
                    版本
                  </div>
                  <span className="settings-value">0.2.4</span>
                </div>
                <button
                  className="settings-row settings-row-button"
                  onClick={() =>
                    openLink('https://github.com/kevinhuang001/fiex/blob/main/THIRD-PARTY.md')
                  }
                >
                  <div className="settings-row-label">
                    <span className="settings-row-icon" style={{ background: '#10b981' }}>
                      <Scale size={15} />
                    </span>
                    开源许可
                  </div>
                  <ExternalLink size={14} />
                </button>
              </div>
            </>
          )}
        </section>
      </div>
    </Modal>
  )
}
