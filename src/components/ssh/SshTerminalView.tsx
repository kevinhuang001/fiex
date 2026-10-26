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

interface SshEvent {
  type: 'data' | 'exit' | 'error'
  data?: number[]
  code?: number | null
  message?: string
}

function terminalTheme() {
  const { mode, tokens } = themeSnapshot()
  return {
    background: tokens.surface || (mode === 'dark' ? '#1e1e1e' : '#ffffff'),
    foreground: tokens.text || (mode === 'dark' ? '#e6e6e6' : '#1f2328'),
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

/** Tiny interactive shell used only by the web demo. It never executes input. */
function attachDemoShell(terminal: { write: (value: string) => void }, cwd?: string) {
  const home = cwd && !cwd.includes('://') ? cwd : '/Users/kevin/Documents'
  let line = ''
  const prompt = '\x1b[32muser@fiex\x1b[0m:\x1b[34m~\x1b[0m$ '
  const write = (value: string) => terminal.write(value)
  write(
    '\x1b[1;36mFiex 终端\x1b[0m\r\n' +
      '\x1b[90mWeb demo shell — 桌面版会在这里启动真实本地 shell。输入 help 查看命令。\x1b[0m\r\n\r\n' +
      prompt,
  )
  const run = (command: string) => {
    const [name, ...args] = command.trim().split(/\s+/)
    const arg = args.join(' ')
    switch (name) {
      case '':
        break
      case 'help':
        write('\r\n可用命令：ls  pwd  echo  whoami  date  clear  help\r\n')
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
        write(`\r\ncommand not found: ${name}（这是 Web 演示终端）\r\n`)
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
}: {
  connectionId?: string
  cwd?: string
  shell?: string
  name: string
}) {
  const host = useRef<HTMLDivElement>(null)
  const [attempt, setAttempt] = useState(0)
  const [status, setStatus] = useState<'connecting' | 'open' | 'closed' | 'error' | 'demo'>(
    'connecting',
  )
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
      fontFamily:
        '"JetBrains Mono", "SFMono-Regular", Menlo, Consolas, "Liberation Mono", monospace',
      fontSize: 13,
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
    fit.fit()
    terminal.focus()

    const applyTheme = () => {
      terminal.options.theme = terminalTheme()
    }
    window.addEventListener('fiex:theme-changed', applyTheme)

    let channel: Channel<SshEvent> | null = null
    terminal.onData((data) => {
      if (sessionId)
        void call('ssh_write', { sessionId, data: Array.from(new TextEncoder().encode(data)) })
    })
    const resize = () => {
      fit.fit()
      if (sessionId)
        void call('ssh_resize', { sessionId, cols: terminal.cols, rows: terminal.rows }).catch(
          () => {},
        )
    }
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
        terminal.dispose()
      }
    }
    channel = new Channel<SshEvent>()
    channel.onmessage = (event) => {
      if (disposed) return
      if (event.type === 'data' && event.data) {
        terminal.write(new Uint8Array(event.data))
      } else if (event.type === 'error') {
        setError(event.message || 'SSH 会话失败')
        setStatus('error')
      } else if (event.type === 'exit') {
        setStatus('closed')
      }
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
        setStatus('open')
        resize()
      })
      .catch((cause) => {
        setError(String(cause))
        setStatus('error')
      })
    return () => {
      disposed = true
      observer.disconnect()
      window.removeEventListener('fiex:theme-changed', applyTheme)
      if (sessionId) void call('ssh_close', { sessionId }).catch(() => {})
      terminal.dispose()
    }
  }, [connectionId, cwd, shell, attempt])

  return (
    <div className="ssh-terminal-view" data-connection={connectionId}>
      <div className="ssh-terminal-host" ref={host} />
      {(status === 'connecting' || status === 'closed' || status === 'error') && (
        <div className="ssh-terminal-status">
          {status === 'connecting' ? (
            <>
              <LoaderCircle size={14} className="spin" />
              <span>正在连接 {name}…</span>
            </>
          ) : (
            <>
              <span>{status === 'error' ? error : 'SSH 会话已结束'}</span>
              <button type="button" onClick={() => setAttempt((value) => value + 1)}>
                <RotateCw size={13} /> 重新连接
              </button>
            </>
          )}
        </div>
      )}
    </div>
  )
}

// Backwards-compatible name for existing SSH call sites.
