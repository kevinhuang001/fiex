import { t } from '@/i18n'
import { useEffect, useRef, useState } from 'react'
import { Channel } from '@tauri-apps/api/core'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { Unicode11Addon } from '@xterm/addon-unicode11'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import { LoaderCircle, RotateCw } from 'lucide-react'
import { call, native } from '@/lib/api'
import { themeSnapshot } from '@/lib/themes'

import { terminalEvents, type SshEvent, type SessionStatus } from './sessionEvents'
import { attachDemoShell } from './terminalDemoShell'

function withAlpha(color: string, alpha: number) {
  const value = color.trim()
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})(?:[0-9a-f]{2})?$/i.exec(value)
  if (hex) {
    const digits =
      hex[1].length === 3
        ? hex[1]
            .split('')
            .map((part) => part + part)
            .join('')
        : hex[1]
    const [r, g, b] = [0, 2, 4].map((index) => parseInt(digits.slice(index, index + 2), 16))
    return `rgba(${r}, ${g}, ${b}, ${alpha})`
  }
  const rgb = /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)/i.exec(value)
  return rgb ? `rgba(${rgb[1]}, ${rgb[2]}, ${rgb[3]}, ${alpha})` : value
}

function terminalTheme(color = '') {
  const { mode, tokens } = themeSnapshot()
  const surface = tokens.surface || (mode === 'dark' ? '#22303c' : '#f8fafc')
  return {
    // Let the terminal surface, or the wallpaper behind it, show through.
    background: withAlpha(surface, 0.4),
    foreground: color || tokens.text || (mode === 'dark' ? '#e6e6e6' : '#1f2328'),
    cursor: tokens.accent || (mode === 'dark' ? '#7aa2f7' : '#2f6fed'),
    cursorAccent: tokens.surface || (mode === 'dark' ? '#1e1e1e' : '#ffffff'),
    selectionBackground: tokens.selected || (mode === 'dark' ? '#3b4261' : '#cfe0ff'),
    black: mode === 'dark' ? '#15161e' : '#24292f',
    red: tokens.danger || '#e5534b',
    green: tokens.success || '#2da44e',
    yellow: tokens.warning || '#bf8700',
    blue: tokens.accent || '#2f6fed',
    magenta: '#a475f9',
    cyan: '#39c5cf',
    white: tokens.text || '#e6edf3',
    brightBlack: tokens.muted || '#6e7781',
    brightRed: '#ff7b72',
    brightGreen: '#56d364',
    brightYellow: '#e3b341',
    brightBlue: '#79c0ff',
    brightMagenta: '#d2a8ff',
    brightCyan: '#56d4dd',
    brightWhite: '#ffffff',
  }
}

function terminalFontFamily(terminalFont?: string) {
  return [
    terminalFont,
    'JetBrainsMono Nerd Font',
    'MesloLGS NF',
    'Hack Nerd Font',
    'FiraCode Nerd Font',
    'JetBrains Mono',
    'SFMono-Regular',
    'Menlo',
    'Consolas',
    'Liberation Mono',
    'monospace',
  ]
    .filter((value): value is string => Boolean(value))
    .map((value) => (value === 'monospace' ? value : `"${value}"`))
    .join(', ')
}

/** Built-in SSH terminal; uses the system OpenSSH client through a real PTY. */
export function TerminalView({
  connectionId,
  cwd,
  shell,
  name,
  terminalFont,
  terminalFontSize,
  terminalColor,
}: {
  connectionId?: string
  cwd?: string
  shell?: string
  name: string
  /** A system font family chosen in Settings; Nerd Font names are supported. */
  terminalFont?: string
  terminalFontSize?: number
  terminalColor?: string
}) {
  const host = useRef<HTMLDivElement>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const resizeRef = useRef<(() => void) | null>(null)
  const colorRef = useRef(terminalColor)
  colorRef.current = terminalColor
  const [attempt, setAttempt] = useState(0)
  const [status, setStatus] = useState<SessionStatus>('connecting')
  const [error, setError] = useState('')

  useEffect(() => {
    const element = host.current
    if (!element) return
    let disposed = false
    let sessionId = ''
    const terminal = new Terminal({
      allowProposedApi: true,
      // Let the translucent panel surface show through the cell background.
      allowTransparency: true,
      // A steady cursor avoids the unequal, very fast blink some WebViews show.
      cursorBlink: false,
      fontFamily: terminalFontFamily(terminalFont),
      fontSize: terminalFontSize ?? 13,
      lineHeight: 1.15,
      scrollback: 20000,
      theme: terminalTheme(colorRef.current),
      windowsPty: { backend: 'conpty' },
    })
    const fit = new FitAddon()
    const unicode = new Unicode11Addon()
    terminal.loadAddon(fit)
    terminal.loadAddon(unicode)
    terminal.loadAddon(
      new WebLinksAddon((event, uri) => {
        event.preventDefault()
        void call('open_external_url', { url: uri })
      }),
    )
    terminal.unicode.activeVersion = '11'
    terminal.open(element)
    terminalRef.current = terminal
    if (element.clientWidth > 1 && element.clientHeight > 1) fit.fit()
    terminal.focus()

    const applyTheme = () => {
      terminal.options.theme = terminalTheme(colorRef.current)
    }
    window.addEventListener('fiex:theme-changed', applyTheme)

    let channel: Channel<SshEvent> | null = null
    terminal.onData((data) => {
      if (sessionId)
        void call('ssh_write', {
          sessionId,
          data: Array.from(new TextEncoder().encode(data)),
        }).catch((cause) => {
          if (!disposed) {
            setError(String(cause))
            setStatus('error')
          }
        })
    })
    const resize = () => {
      // Hidden panes report 0×0; fitting there would shrink the PTY to two
      // columns until the tab becomes visible again.
      if (element.clientWidth < 2 || element.clientHeight < 2) return
      fit.fit()
      if (sessionId)
        void call('ssh_resize', { sessionId, cols: terminal.cols, rows: terminal.rows }).catch(
          () => {},
        )
    }
    resizeRef.current = resize
    const observer = new ResizeObserver(() => requestAnimationFrame(resize))
    observer.observe(element)
    void document.fonts?.ready.then(() => requestAnimationFrame(resize)).catch(() => {})
    terminal.attachCustomKeyEventHandler((event) => {
      if (event.type !== 'keydown') return true
      const copy = event.code === 'KeyC' && (event.metaKey || event.shiftKey)
      const paste = event.code === 'KeyV' && (event.metaKey || event.shiftKey)
      if (copy && terminal.hasSelection()) {
        void navigator.clipboard.writeText(terminal.getSelection())
        return false
      }
      if (paste) {
        void navigator.clipboard
          .readText()
          .then((text) => terminal.paste(text))
          .catch(() => {})
        return false
      }
      return true
    })
    setStatus('connecting')
    setError('')
    if (!native) {
      setStatus('demo')
      const disposeShell = attachDemoShell(terminal, cwd)
      return () => {
        disposed = true
        observer.disconnect()
        window.removeEventListener('fiex:theme-changed', applyTheme)
        disposeShell()
        terminalRef.current = null
        resizeRef.current = null
        terminal.dispose()
      }
    }
    channel = new Channel<SshEvent>()
    const events = terminalEvents(
      (data) => terminal.write(data),
      setStatus,
      (message) => setError(message || t('SSH 会话失败')),
    )
    channel.onmessage = (event) => {
      if (!disposed) events.receive(event)
    }
    const opening = connectionId
      ? call<string>('ssh_open', {
          connectionId,
          cols: terminal.cols,
          rows: terminal.rows,
          onEvent: channel,
        })
      : call<string>('local_terminal_open', {
          cwd: cwd || null,
          shell: shell || null,
          cols: terminal.cols,
          rows: terminal.rows,
          onEvent: channel,
        })
    opening
      .then((id) => {
        if (disposed) {
          void call('ssh_close', { sessionId: id })
          return
        }
        sessionId = id
        // The ID only acknowledges startup. SSH becomes ready after the
        // server accepts authentication and starts its shell.
        resize()
      })
      .catch((cause) => {
        if (disposed || events.ended()) return
        setError(String(cause))
        setStatus('error')
      })
    return () => {
      disposed = true
      observer.disconnect()
      window.removeEventListener('fiex:theme-changed', applyTheme)
      if (sessionId) void call('ssh_close', { sessionId }).catch(() => {})
      terminalRef.current = null
      resizeRef.current = null
      terminal.dispose()
    }
  }, [connectionId, cwd, shell, attempt])

  useEffect(() => {
    const terminal = terminalRef.current
    if (!terminal) return
    terminal.options.fontFamily = terminalFontFamily(terminalFont)
    terminal.options.fontSize = terminalFontSize ?? 13
    terminal.options.theme = terminalTheme(terminalColor)
    const frame = requestAnimationFrame(() => resizeRef.current?.())
    return () => cancelAnimationFrame(frame)
  }, [terminalFont, terminalFontSize, terminalColor])

  return (
    <div className="ssh-terminal-view" data-connection={connectionId}>
      <div className="ssh-terminal-host" ref={host} />
      {(status === 'connecting' || status === 'closed' || status === 'error') && (
        <div className="ssh-terminal-status">
          {status === 'connecting' ? (
            <>
              <LoaderCircle size={14} className="spin" />
              <span>
                {t('正在连接')}
                {name}…
              </span>
            </>
          ) : (
            <>
              <span>{status === 'error' ? error : t('SSH 会话已结束')}</span>
              <button type="button" onClick={() => setAttempt((value) => value + 1)}>
                <RotateCw size={13} /> {t('重新连接')}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  )
}

// Backwards-compatible name for existing SSH call sites.
