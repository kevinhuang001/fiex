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
import { reportError } from '@/lib/errorCenter'
import { themeSnapshot } from '@/lib/themes'

import { terminalEvents, type TerminalEvent, type SessionStatus } from './sessionEvents'
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

/**
 * The terminal has no color settings of its own. Chrome colors come from the
 * active theme tokens and the ANSI palette from `--terminal-ansi-*` variables,
 * so a light/dark switch or a theme change recolors every terminal in place.
 */
function terminalTheme() {
  const { mode, tokens } = themeSnapshot()
  const isDark = mode === 'dark'
  const ansi = (name: string) => tokens[`terminal-ansi-${name}`] || undefined
  return {
    // Let the terminal surface, or the wallpaper behind it, show through.
    background: withAlpha(tokens.surface || (isDark ? '#22303c' : '#f8fafc'), 0.4),
    foreground: tokens.text || (isDark ? '#e6edf3' : '#1f2328'),
    cursor: tokens.accent || (isDark ? '#7aa2f7' : '#2f6fed'),
    cursorAccent: tokens.surface || (isDark ? '#1e1e1e' : '#ffffff'),
    selectionBackground: tokens.selected || (isDark ? '#3b4261' : '#cfe0ff'),
    black: ansi('black'),
    red: ansi('red'),
    green: ansi('green'),
    yellow: ansi('yellow'),
    blue: ansi('blue'),
    magenta: ansi('magenta'),
    cyan: ansi('cyan'),
    white: ansi('white'),
    brightBlack: ansi('bright-black'),
    brightRed: ansi('bright-red'),
    brightGreen: ansi('bright-green'),
    brightYellow: ansi('bright-yellow'),
    brightBlue: ansi('bright-blue'),
    brightMagenta: ansi('bright-magenta'),
    brightCyan: ansi('bright-cyan'),
    brightWhite: ansi('bright-white'),
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

/** Local shell terminal. Remote connections render their own plugin views. */
export function TerminalView({
  cwd,
  shell,
  name,
  terminalFont,
  terminalFontSize,
}: {
  cwd?: string
  shell?: string
  name: string
  /** A system font family chosen in Settings; Nerd Font names are supported. */
  terminalFont?: string
  terminalFontSize?: number
}) {
  const host = useRef<HTMLDivElement>(null)
  const terminalRef = useRef<Terminal | null>(null)
  const resizeRef = useRef<(() => void) | null>(null)
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
      theme: terminalTheme(),
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
      terminal.options.theme = terminalTheme()
    }
    window.addEventListener('fiex:theme-changed', applyTheme)

    let channel: Channel<TerminalEvent> | null = null
    terminal.onData((data) => {
      if (sessionId)
        void call('terminal_write', {
          sessionId,
          data: Array.from(new TextEncoder().encode(data)),
        }).catch((cause) => {
          if (!disposed) {
            setError(String(cause))
            reportError(cause, 'terminal:write')
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
        void call('terminal_resize', { sessionId, cols: terminal.cols, rows: terminal.rows }).catch(
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
    channel = new Channel<TerminalEvent>()
    const events = terminalEvents(
      (data) => terminal.write(data),
      setStatus,
      (message) => {
        const detail = message || t('终端会话表不可用')
        setError(detail)
        reportError(detail, 'terminal:session')
      },
    )
    channel.onmessage = (event) => {
      if (!disposed) events.receive(event)
    }
    const opening = call<string>('local_terminal_open', {
      cwd: cwd || null,
      shell: shell || null,
      cols: terminal.cols,
      rows: terminal.rows,
      onEvent: channel,
    })
    opening
      .then((id) => {
        if (disposed) {
          void call('terminal_close', { sessionId: id })
          return
        }
        sessionId = id
        // The local shell has started after the PTY is created.
        resize()
      })
      .catch((cause) => {
        if (disposed || events.ended()) return
        setError(String(cause))
        reportError(cause, 'terminal:open')
        setStatus('error')
      })
    return () => {
      disposed = true
      observer.disconnect()
      window.removeEventListener('fiex:theme-changed', applyTheme)
      if (sessionId) void call('terminal_close', { sessionId }).catch(() => {})
      terminalRef.current = null
      resizeRef.current = null
      terminal.dispose()
    }
  }, [cwd, shell, attempt])

  useEffect(() => {
    const terminal = terminalRef.current
    if (!terminal) return
    terminal.options.fontFamily = terminalFontFamily(terminalFont)
    terminal.options.fontSize = terminalFontSize ?? 13
    terminal.options.theme = terminalTheme()
    const frame = requestAnimationFrame(() => resizeRef.current?.())
    return () => cancelAnimationFrame(frame)
  }, [terminalFont, terminalFontSize])

  return (
    <div className="local-terminal-view">
      <div className="local-terminal-host" ref={host} />
      {(status === 'connecting' || status === 'closed' || status === 'error') && (
        <div className="local-terminal-status">
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
              <span>{status === 'error' ? t('连接失败') : t('SSH 会话已结束')}</span>
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
