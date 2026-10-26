import { useEffect, useRef, useState } from 'react'
import { ChevronDown, Plus, SquareTerminal, X } from 'lucide-react'
import { TerminalView } from '@/components/ssh/SshTerminalView'
import { call } from '@/lib/api'

interface TerminalTab {
  id: string
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
export function TerminalPanel() {
  const [tabs, setTabs] = useState<TerminalTab[]>([])
  const [active, setActive] = useState('')
  const [open, setOpen] = useState(false)
  const [height, setHeight] = useState(260)
  const [shells, setShells] = useState<ShellChoice[]>([])
  const [shellMenu, setShellMenu] = useState(false)
  const lastCwd = useRef('')
  const panel = useRef<HTMLElement>(null)
  const shellMenuRef = useRef<HTMLDivElement>(null)
  const dragTab = useRef<number | null>(null)

  useEffect(() => {
    void call<ShellChoice[]>('detect_shells')
      .then((items) => setShells(Array.isArray(items) ? items : []))
      .catch(() => {})
  }, [])
  useEffect(() => {
    if (!shellMenu) return
    const close = (event: MouseEvent) => {
      if (!shellMenuRef.current?.contains(event.target as Node)) setShellMenu(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [shellMenu])

  useEffect(() => {
    const handler = (event: Event) => {
      const detail = (event as CustomEvent<{ cwd?: string }>).detail
      const cwd = detail?.cwd || lastCwd.current
      if (cwd) lastCwd.current = cwd
      const id = crypto.randomUUID()
      setTabs((values) => [
        ...values,
        { id, cwd, shell: shells[0]?.path, shellName: shells[0]?.name },
      ])
      setActive(id)
      setOpen(true)
    }
    window.addEventListener('fiex:open-builtin-terminal', handler)
    return () => window.removeEventListener('fiex:open-builtin-terminal', handler)
  }, [])

  useEffect(() => {
    const show = (event: Event) => {
      const detail = (event as CustomEvent<{ cwd?: string }>).detail
      const cwd = detail?.cwd || lastCwd.current
      if (cwd) lastCwd.current = cwd
      if (tabs.length) {
        setOpen(true)
        if (!active || !tabs.some((tab) => tab.id === active)) setActive(tabs[tabs.length - 1].id)
        return
      }
      const id = crypto.randomUUID()
      setTabs([{ id, cwd, shell: shells[0]?.path, shellName: shells[0]?.name }])
      setActive(id)
      setOpen(true)
    }
    window.addEventListener('fiex:show-terminal-panel', show)
    return () => window.removeEventListener('fiex:show-terminal-panel', show)
  }, [tabs, active, shells])

  if (!tabs.length) return null
  const resizeStart = (event: React.MouseEvent) => {
    event.preventDefault()
    const bottom = panel.current?.getBoundingClientRect().bottom ?? window.innerHeight
    const move = (moveEvent: MouseEvent) => {
      const next = bottom - moveEvent.clientY
      setHeight(Math.min(Math.max(140, window.innerHeight * 0.72), Math.max(120, next)))
    }
    const stop = () => {
      window.removeEventListener('mousemove', move)
      window.removeEventListener('mouseup', stop)
    }
    window.addEventListener('mousemove', move)
    window.addEventListener('mouseup', stop)
  }
  const moveTab = (to: number) => {
    const from = dragTab.current
    dragTab.current = null
    if (from === null || from === to) return
    setTabs((values) => {
      const next = [...values]
      const [moved] = next.splice(from, 1)
      next.splice(to, 0, moved)
      return next
    })
  }
  const add = (shell?: ShellChoice) => {
    const id = crypto.randomUUID()
    setTabs((values) => [
      ...values,
      { id, cwd: lastCwd.current, shell: shell?.path, shellName: shell?.name },
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
      aria-label="内置终端"
    >
      <div
        className="terminal-panel-resizer"
        role="separator"
        aria-orientation="horizontal"
        aria-label="调整终端高度"
        onMouseDown={resizeStart}
      />
      <header className="terminal-panel-bar">
        <div className="terminal-panel-tabs" role="tablist" aria-label="终端标签">
          {tabs.map((tab, index) => (
            <div
              className={`terminal-panel-tab ${tab.id === active ? 'active' : ''}`}
              key={tab.id}
              draggable
              onDragStart={() => {
                dragTab.current = index
              }}
              onDragOver={(event) => event.preventDefault()}
              onDrop={() => moveTab(index)}
              onDragEnd={() => {
                dragTab.current = null
              }}
            >
              <button
                type="button"
                role="tab"
                aria-selected={tab.id === active}
                onClick={() => {
                  setActive(tab.id)
                  setOpen(true)
                }}
              >
                <SquareTerminal size={12} />
                {tab.shellName || '终端'} {index + 1}
              </button>
              <button
                type="button"
                className="terminal-panel-close"
                title="关闭终端"
                aria-label={`关闭终端 ${index + 1}`}
                onClick={() => close(tab.id)}
              >
                <X size={12} />
              </button>
            </div>
          ))}
        </div>
        <div className="terminal-panel-actions" ref={shellMenuRef}>
          <div className="terminal-shell-picker">
            <button
              type="button"
              title="新建终端"
              aria-label="新建终端"
              aria-expanded={shellMenu}
              onClick={() => setShellMenu((value) => !value)}
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
                    默认 Shell
                  </button>
                )}
              </div>
            )}
          </div>
          <button
            type="button"
            title="收起终端"
            aria-label="收起终端"
            onClick={() => setOpen(false)}
          >
            <ChevronDown size={14} />
          </button>
        </div>
      </header>
      <div className="terminal-panel-body">
        {tabs.map((tab, index) => (
          <div className="terminal-panel-pane" key={tab.id} hidden={tab.id !== active || !open}>
            <TerminalView
              cwd={tab.cwd}
              shell={tab.shell}
              name={`终端 ${index + 1}${tab.shellName ? ` · ${tab.shellName}` : ''}`}
            />
          </div>
        ))}
      </div>
    </section>
  )
}
