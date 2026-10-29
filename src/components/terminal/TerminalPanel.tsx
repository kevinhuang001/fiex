import { TerminalAppearanceMenu } from './TerminalAppearanceMenu'
import { t } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { ChevronDown, Plus, Settings2 } from 'lucide-react'
import { TerminalView } from '@/components/terminal/TerminalView'
import { TerminalTabs } from './TerminalTabs'
import { call } from '@/lib/api'
import type { Preferences } from '@/lib/types'
import { uiScale } from '@/lib/uiScale'

interface TerminalTab {
  id: string
  number: number
  cwd: string
  shell?: string
  shellName?: string
}

interface ShellChoice {
  id: string
  name: string
  path: string
}

/** VS Code-style integrated terminal panel. Tabs stay mounted while the panel
 * is collapsed or another terminal is active. */
export function TerminalPanel({
  terminalFont,
  terminalFontSize,
  onSettingsChange,
}: {
  terminalFont?: string
  terminalFontSize?: number
  onSettingsChange: (patch: Partial<Preferences>) => void
}) {
  const [tabs, setTabs] = useState<TerminalTab[]>([])
  const [active, setActive] = useState('')
  const [open, setOpen] = useState(false)
  const [height, setHeight] = useState(260)
  const [shells, setShells] = useState<ShellChoice[]>([])
  const [shellMenu, setShellMenu] = useState(false)
  const [settingsMenu, setSettingsMenu] = useState(false)
  const [fonts, setFonts] = useState<string[]>([])
  const lastCwd = useRef('')
  const nextNumber = useRef(1)
  const panel = useRef<HTMLElement>(null)
  const shellMenuRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!settingsMenu || fonts.length) return
    void call<string[]>('system_fonts')
      .then((items) => setFonts(Array.isArray(items) ? items : []))
      .catch(() => {})
  }, [settingsMenu, fonts.length])

  useEffect(() => {
    void call<ShellChoice[]>('detect_shells')
      .then((items) => setShells(Array.isArray(items) ? items : []))
      .catch(() => {})
  }, [])
  useEffect(() => {
    if (!shellMenu && !settingsMenu) return
    const close = (event: MouseEvent) => {
      if (!shellMenuRef.current?.contains(event.target as Node)) {
        setShellMenu(false)
        setSettingsMenu(false)
      }
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [shellMenu, settingsMenu])

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<{ cwd?: string }>).detail
      const cwd = detail?.cwd || lastCwd.current
      if (cwd) lastCwd.current = cwd
      const id = crypto.randomUUID()
      const number = nextNumber.current++
      setTabs((values) => [
        ...values,
        { id, number, cwd, shell: shells[0]?.path, shellName: shells[0]?.name },
      ])
      setActive(id)
      setOpen(true)
    }
    window.addEventListener('fiex:open-builtin-terminal', handler)
    return () => window.removeEventListener('fiex:open-builtin-terminal', handler)
  }, [shells])

  useEffect(() => {
    const show = (event: Event) => {
      const detail = (event as CustomEvent<{ cwd?: string }>).detail
      const cwd = detail?.cwd || lastCwd.current
      if (cwd) lastCwd.current = cwd
      if (tabs.length) {
        setOpen((value) => !value)
        setShellMenu(false)
        setSettingsMenu(false)
        if (!active || !tabs.some((tab) => tab.id === active)) setActive(tabs[tabs.length - 1].id)
        return
      }
      const id = crypto.randomUUID()
      setTabs([
        {
          id,
          number: nextNumber.current++,
          cwd,
          shell: shells[0]?.path,
          shellName: shells[0]?.name,
        },
      ])
      setActive(id)
      setOpen(true)
    }
    window.addEventListener('fiex:show-terminal-panel', show)
    return () => window.removeEventListener('fiex:show-terminal-panel', show)
  }, [tabs, active, shells])

  if (!tabs.length) return null
  const resizeStart = (event: React.MouseEvent) => {
    event.preventDefault()
    const scale = uiScale()
    const bottom = panel.current?.getBoundingClientRect().bottom ?? window.innerHeight
    const limit = Math.max(140, (window.innerHeight / scale) * 0.72)
    let next = height
    const move = (moveEvent: MouseEvent) => {
      next = Math.min(limit, Math.max(120, (bottom - moveEvent.clientY) / scale))
      // Write the height directly while dragging: rerendering the tab strip and
      // every terminal view on each pointer event makes the resize feel sticky.
      if (panel.current) panel.current.style.height = `${next}px`
    }
    const stop = () => {
      window.removeEventListener('mousemove', move)
      window.removeEventListener('mouseup', stop)
      document.body.classList.remove('resizing-terminal')
      setHeight(next)
    }
    document.body.classList.add('resizing-terminal')
    window.addEventListener('mousemove', move)
    window.addEventListener('mouseup', stop)
  }
  const moveTab = (from: number, to: number) => {
    if (from === to) return
    setTabs((values) => {
      const next = [...values]
      const [moved] = next.splice(from, 1)
      next.splice(to, 0, moved)
      return next
    })
  }
  const add = (shell?: ShellChoice) => {
    const id = crypto.randomUUID()
    const number = nextNumber.current++
    setTabs((values) => [
      ...values,
      { id, number, cwd: lastCwd.current, shell: shell?.path, shellName: shell?.name },
    ])
    setActive(id)
    setOpen(true)
    setShellMenu(false)
  }
  const close = (id: string) => {
    setTabs((values) => {
      const index = values.findIndex((tab) => tab.id === id)
      const next = values.filter((tab) => tab.id !== id)
      if (active === id) setActive(next[Math.max(0, index - 1)]?.id || '')
      return next
    })
  }

  return (
    <section
      ref={panel}
      className={`terminal-panel ${open ? '' : 'closed'}`}
      style={{ height: open ? height : undefined }}
      aria-label={t('内置终端')}
    >
      <div
        className="terminal-panel-resizer"
        role="separator"
        aria-orientation="horizontal"
        aria-label={t('调整终端高度')}
        onMouseDown={resizeStart}
      />
      <header className="terminal-panel-bar">
        <TerminalTabs
          tabs={tabs}
          active={active}
          onSelect={(id) => {
            setActive(id)
            setOpen(true)
          }}
          onClose={close}
          onMove={moveTab}
        />
        <div className="terminal-panel-actions" ref={shellMenuRef}>
          <div className="terminal-shell-picker">
            <button
              type="button"
              title={t('新建终端')}
              aria-label={t('新建终端')}
              aria-expanded={shellMenu}
              onClick={() => {
                setSettingsMenu(false)
                setShellMenu((value) => !value)
              }}
            >
              <Plus size={14} />
            </button>
            {shellMenu && (
              <div className="terminal-shell-menu" role="menu">
                {shells.length ? (
                  shells.map((shell) => (
                    <button key={shell.id} type="button" role="menuitem" onClick={() => add(shell)}>
                      {shell.name}
                    </button>
                  ))
                ) : (
                  <button type="button" role="menuitem" onClick={() => add()}>
                    {t('默认 Shell')}
                  </button>
                )}
              </div>
            )}
          </div>
          <div className="terminal-settings-picker">
            <button
              type="button"
              title={t('终端设置')}
              aria-label={t('终端设置')}
              aria-expanded={settingsMenu}
              onClick={() => {
                setShellMenu(false)
                setSettingsMenu((value) => !value)
              }}
            >
              <Settings2 size={14} />
            </button>
            {settingsMenu && (
              <TerminalAppearanceMenu
                terminalFont={terminalFont}
                terminalFontSize={terminalFontSize}
                fonts={fonts}
                onSettingsChange={onSettingsChange}
              />
            )}
          </div>
          <button
            type="button"
            title={t('收起终端')}
            aria-label={t('收起终端')}
            onClick={() => {
              setOpen(false)
              setSettingsMenu(false)
            }}
          >
            <ChevronDown size={14} />
          </button>
        </div>
      </header>
      <div className="terminal-panel-body">
        {tabs.map((tab) => (
          <div className="terminal-panel-pane" key={tab.id} hidden={tab.id !== active || !open}>
            <TerminalView
              cwd={tab.cwd}
              shell={tab.shell}
              terminalFont={terminalFont}
              terminalFontSize={terminalFontSize}
              name={t('终端 {0}{1}', tab.number, tab.shellName ? ` · ${tab.shellName}` : '')}
            />
          </div>
        ))}
      </div>
    </section>
  )
}
