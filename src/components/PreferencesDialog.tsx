import type { Ide, Location, Preferences, Terminal } from '@/lib/types'
import {
  Filter,
  Info,
  Keyboard,
  Palette,
  Puzzle,
  PanelLeft,
  Search,
  Tags,
  TerminalSquare,
  type LucideIcon,
} from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { AboutSettings } from './settings/AboutSettings'
import { AppearanceSettings } from './settings/AppearanceSettings'
import { IgnoreSettings } from './settings/IgnoreSettings'
import { IntegrationSettings } from './settings/IntegrationSettings'
import { ShortcutSettings } from './settings/ShortcutSettings'
import { SidebarSettings } from './settings/SidebarSettings'
import { TagSettings } from './settings/TagSettings'
import { Modal } from './ui/Modal'
import { PluginSettings } from './settings/PluginSettings'

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

type SectionId =
  'appearance' | 'shortcuts' | 'ignore' | 'sidebar' | 'tags' | 'terminal' | 'about' | 'plugins'

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
    id: 'sidebar',
    label: '侧边栏',
    icon: PanelLeft,
    color: '#5b9cf8',
    keywords: ['sidebar', '侧边栏', '常用位置', '收藏', '设备', '连接', '标签', '顺序'],
  },
  {
    id: 'tags',
    label: '标签',
    icon: Tags,
    color: '#ec4899',
    keywords: ['tag', 'tags', '标签', '颜色', '自定义'],
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
    label: '启动方式',
    icon: TerminalSquare,
    color: '#10b981',
    keywords: ['terminal', 'shell', 'ide', '终端', '命令行', '启动', '编辑器'],
  },
  {
    id: 'plugins',
    label: '插件',
    icon: Puzzle,
    color: '#64748b',
    keywords: ['插件', 'plugin', '扩展', '配置', '权限', 'OAuth'],
  },
  {
    id: 'about',
    label: '关于',
    icon: Info,
    color: '#64748b',
    keywords: ['about', 'version', 'license', '关于', '版本', '许可'],
  },
]

type SearchEntry = { section: SectionId; label: string; keywords: string[] }

const searchEntries: SearchEntry[] = [
  { section: 'appearance', label: '主题', keywords: ['theme', '主题', '皮肤'] },
  {
    section: 'appearance',
    label: '外观模式',
    keywords: ['dark', 'light', '深色', '浅色', '跟随系统'],
  },
  { section: 'appearance', label: '显示隐藏文件', keywords: ['hidden', '隐藏文件', '.ds_store'] },
  { section: 'appearance', label: '启动页', keywords: ['start', '启动', '首页'] },
  {
    section: 'appearance',
    label: '启动时恢复标签页',
    keywords: ['tab', 'restore', '恢复', '标签页', '会话'],
  },
  { section: 'appearance', label: '字体大小', keywords: ['font', '字号', '缩放', '大小'] },
  { section: 'sidebar', label: '区域顺序', keywords: ['sidebar', '顺序', '排序', '区域'] },
  { section: 'sidebar', label: '侧边栏分区', keywords: ['显示', '隐藏', '标签', '设备', '连接'] },
  { section: 'sidebar', label: '常用位置', keywords: ['places', '位置', '显示'] },
  { section: 'shortcuts', label: '快捷键方案', keywords: ['shortcut', 'preset', '方案', '按键'] },
  { section: 'shortcuts', label: '自定义快捷键', keywords: ['shortcut', '自定义', '按键'] },
  { section: 'tags', label: '标签列表', keywords: ['tag', '标签', '颜色'] },
  { section: 'tags', label: '添加标签', keywords: ['tag', '新建', '颜色'] },
  { section: 'ignore', label: '忽略规则', keywords: ['ignore', '忽略', 'node_modules'] },
  { section: 'terminal', label: '终端', keywords: ['terminal', 'shell', '命令行'] },
  { section: 'terminal', label: 'IDE', keywords: ['editor', '编辑器', '代码'] },
  { section: 'plugins', label: '已安装插件', keywords: ['plugin', '权限', '激活'] },
  { section: 'plugins', label: '获取插件', keywords: ['plugin', '商店', '更新'] },
  { section: 'about', label: '版本信息', keywords: ['version', '版本', '许可', 'license'] },
]

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
  }, [query])
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
  }, [query, results])

  useEffect(() => {
    if (query && visible.length && !visible.some((item) => item.id === section))
      setSection(visible[0].id)
  }, [query, visible, section])

  return (
    <>
      <Modal open={open} onClose={onClose} title="设置" className="settings-modal" bare>
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
            {query.trim() ? (
              <div className="settings-search-results" role="listbox" aria-label="设置搜索结果">
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
                {!results.length && <p>没有匹配的设置项</p>}
              </div>
            ) : (
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
            )}
          </aside>

          <section className="settings-content">
            {section === 'appearance' && <AppearanceSettings prefs={prefs} update={update} />}

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

            {section === 'plugins' && <PluginSettings />}

            {section === 'about' && <AboutSettings onError={onError} />}
            <footer className="settings-actions">
              <button className="button" onClick={onClose}>
                取消
              </button>
              <button
                className="button primary"
                onClick={() => {
                  savePreferences(prefs)
                  onClose()
                }}
              >
                确认
              </button>
            </footer>
          </section>
        </div>
      </Modal>
      <Modal
        open={integrationsOpen}
        onClose={() => setIntegrationsOpen(false)}
        title="支持的 IDE 和终端"
      >
        <div className="integration-help">
          <section>
            <h3>IDE</h3>
            <p>
              VS Code · VS Code Insiders · Cursor · Windsurf · Zed · Sublime Text · IntelliJ IDEA ·
              WebStorm · PyCharm · Android Studio · Xcode
            </p>
            <small>已检测：{ides.map((ide) => ide.name).join('、') || '无'}</small>
          </section>
          <section>
            <h3>终端</h3>
            <p>
              系统默认 · Terminal · iTerm2 · Warp · Alacritty · kitty · Ghostty · WezTerm · Tabby ·
              Windows Terminal · PowerShell 7 · PowerShell · Command Prompt · GNOME Terminal ·
              Konsole · Xfce Terminal · Console · XTerm
            </p>
            <small>已检测：{terminals.map((terminal) => terminal.name).join('、') || '无'}</small>
          </section>
        </div>
      </Modal>
    </>
  )
}
