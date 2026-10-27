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

function terminalTheme(color = '') {
  const { mode, tokens } = themeSnapshot()
  return {
    background: tokens.surface || (mode === 'dark' ? '#1e1e1e' : '#ffffff'),
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

/** Tiny interactive shell used only by the web demo. It never executes input. */
function attachDemoShell(terminal: { write: (value: string) => void }, cwd?: string) {
  const home = cwd && !cwd.includes('://') ? cwd : '/Users/kevin/Documents'
  let line = ''
  const prompt = '\x1b[32muser@fiex\x1b[0m:\x1b[34m~\x1b[0m$ '
  const write = (value: string) => terminal.write(value)
  write(
    t('\u001b[1;36mFiex 终端\u001b[0m\r\n') +
      t(
        '\u001b[90mWeb demo shell — 桌面版会在这里启动真实本地 shell。输入 help 查看命令。\u001b[0m\r\n\r\n',
      ) +
      prompt,
  )
  const run = (command: string) => {
    const [name, ...args] = command.trim().split(/\s+/)
    const arg = args.join(' ')
    switch (name) {
      case '':
        break
      case 'help':
        write(t('\r\n可用命令：ls  pwd  echo  whoami  date  clear  help\r\n'))
        break
      case 'ls':
        write('\r\nDocuments  Downloads  Music  Pictures  README.md  package.json\r\n')
        break
      case 'pwd':
        write(`\r\n${home}\r\n`)
        break
      case 'echo':
        write(`\r\n${arg}\r\n`)
        break
      case 'whoami':
        write('\r\nuser\r\n')
        break
      case 'date':
        write(`\r\n${new Date().toString()}\r\n`)
        break
      case 'clear':
        write('\x1b[2J\x1b[H')
        return
      default:
        write(t('\r\ncommand not found: {0}（这是 Web 演示终端）\r\n', name))
    }
  }
  const disposable = (
    terminal as unknown as {
      onData: (listener: (data: string) => void) => { dispose: () => void }
    }
  ).onData((data) => {
    for (const character of data) {
      if (character === '\r' || character === '\n') {
        write('\r\n')
        run(line)
        line = ''
        write(prompt)
      } else if (character === '\x7f') {
        if (line) {
          line = line.slice(0, -1)
          write('\b \b')
        }
      } else if (character === '\x03') {
        write('^C\r\n' + prompt)
        line = ''
      } else if (character >= ' ') {
        line += character
        write(character)
      }
    }
  })
  return () => disposable.dispose()
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
    fit.fit()
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
      fit.fit()
      if (sessionId)
        void call('ssh_resize', { sessionId, cols: terminal.cols, rows: terminal.rows }).catch(
          () => {},
        )
    }
    resizeRef.current = resize
    const observer = new ResizeObserver(() => requestAnimationFrame(resize))
    observer.observe(element)
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
