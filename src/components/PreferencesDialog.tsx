import { t, getLocale } from '@/i18n'
import type { Ide, Location, Preferences, Terminal } from '@/lib/types'
import { Search } from 'lucide-react'
import { sections, searchEntries, type SectionId } from './settings/preferenceSections'
import { useEffect, useMemo, useState } from 'react'
import { AboutSettings } from './settings/AboutSettings'
import { AppearanceSettings } from './settings/AppearanceSettings'
import { BehaviorSettings } from './settings/BehaviorSettings'
import { IgnoreSettings } from './settings/IgnoreSettings'
import { IntegrationSettings } from './settings/IntegrationSettings'
import { ShortcutSettings } from './settings/ShortcutSettings'
import { SidebarSettings } from './settings/SidebarSettings'
import { TagSettings } from './settings/TagSettings'
import { Modal } from './ui/Modal'

interface Props {
  open: boolean
  onClose: () => void
  prefs: Preferences
  locations: Location[]
  update: (p: Partial<Preferences>) => void
  terminals: Terminal[]
  ides: Ide[]
  onError: (s: string) => void
}

export function PreferencesDialog({
  open,
  onClose,
  prefs: savedPrefs,
  locations,
  update: savePreferences,
  terminals,
  ides,
  onError,
}: Props) {
  const [prefs, setDraft] = useState(savedPrefs)
  const update = (patch: Partial<Preferences>) => setDraft((current) => ({ ...current, ...patch }))
  useEffect(() => {
    if (open) setDraft(savedPrefs)
  }, [open, savedPrefs])
  const [section, setSection] = useState<SectionId>('appearance')
  const [query, setQuery] = useState('')
  const [integrationsOpen, setIntegrationsOpen] = useState(false)

  useEffect(() => {
    if (!open) {
      setQuery('')
      setSection('appearance')
      return
    }
  }, [open])

  const results = useMemo(() => {
    const term = query.trim().toLowerCase()
    if (!term) return []
    return searchEntries.filter(
      (item) =>
        item.label.toLowerCase().includes(term) ||
        item.keywords.some((keyword) => keyword.toLowerCase().includes(term)),
    )
  }, [query, getLocale()])
  const visible = useMemo(() => {
    const term = query.trim().toLowerCase()
    if (!term) return sections
    const matched = new Set(results.map((item) => item.section))
    return sections.filter(
      (item) =>
        matched.has(item.id) ||
        item.label.toLowerCase().includes(term) ||
        item.keywords.some((keyword) => keyword.toLowerCase().includes(term)),
    )
  }, [query, results, getLocale()])

  useEffect(() => {
    if (query && visible.length && !visible.some((item) => item.id === section))
      setSection(visible[0].id)
  }, [query, visible, section])

  return (
    <>
      <Modal open={open} onClose={onClose} title={t('设置')} className="settings-modal" bare>
        <div className="settings">
          <aside className="settings-sidebar">
            <div className="settings-profile">
              <img src="/fiex.png" alt="" />
              <div>
                <strong>Fiex</strong>
              </div>
            </div>
            <label className="settings-search">
              <Search size={13} />
              <input
                aria-label={t('搜索设置')}
                placeholder={t('搜索设置')}
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
            {query.trim() ? (
              <div
                className="settings-search-results"
                role="listbox"
                aria-label={t('设置搜索结果')}
              >
                {results.map((item) => {
                  const target = sections.find((entry) => entry.id === item.section)
                  return (
                    <button
                      key={`${item.section}:${item.label}`}
                      role="option"
                      aria-selected="false"
                      onClick={() => {
                        setSection(item.section)
                        setQuery('')
                      }}
                    >
                      <span>{item.label}</span>
                      <small>{target?.label}</small>
                    </button>
                  )
                })}
                {!results.length && <p>{t('没有匹配的设置项')}</p>}
              </div>
            ) : (
              <nav className="settings-nav-list" aria-label={t('设置分类')}>
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
                {visible.length === 0 && <p className="settings-empty">{t('没有匹配的设置')}</p>}
              </nav>
            )}
          </aside>

          <section className="settings-content">
            {section === 'appearance' && <AppearanceSettings prefs={prefs} update={update} />}
            {section === 'behavior' && <BehaviorSettings prefs={prefs} update={update} />}

            {section === 'shortcuts' && <ShortcutSettings prefs={prefs} update={update} />}

            {section === 'sidebar' && (
              <SidebarSettings prefs={prefs} update={update} locations={locations} />
            )}

            {section === 'tags' && <TagSettings prefs={prefs} update={update} />}

            {section === 'ignore' && <IgnoreSettings prefs={prefs} update={update} />}

            {section === 'terminal' && (
              <IntegrationSettings
                prefs={prefs}
                update={update}
                ides={ides}
                terminals={terminals}
                onHelp={() => setIntegrationsOpen(true)}
              />
            )}

            {section === 'about' && <AboutSettings onError={onError} />}
            <footer className="settings-actions">
              <button className="button" onClick={onClose}>
                {t('取消')}
              </button>
              <button
                className="button primary"
                onClick={() => {
                  savePreferences(prefs)
                  onClose()
                }}
              >
                {t('确认')}
              </button>
            </footer>
          </section>
        </div>
      </Modal>
      <Modal
        open={integrationsOpen}
        onClose={() => setIntegrationsOpen(false)}
        title={t('支持的 IDE 和终端')}
      >
        <div className="integration-help">
          <section>
            <h3>IDE</h3>
            <p>
              VS Code · VS Code Insiders · Cursor · Windsurf · Zed · Sublime Text · IntelliJ IDEA ·
              WebStorm · PyCharm · Android Studio · Xcode
            </p>
            <small>
              {t('已检测：')}
              {ides.map((ide) => ide.name).join('、') || t('无')}
            </small>
          </section>
          <section>
            <h3>{t('终端')}</h3>
            <p>
              {t(
                '系统默认 · Terminal · iTerm2 · Warp · Alacritty · kitty · Ghostty · WezTerm · Tabby · Windows Terminal · PowerShell 7 · PowerShell · Command Prompt · GNOME Terminal · Konsole · Xfce Terminal · Console · XTerm',
              )}
            </p>
            <small>
              {t('已检测：')}
              {terminals.map((terminal) => terminal.name).join('、') || t('无')}
            </small>
          </section>
        </div>
      </Modal>
    </>
  )
}
