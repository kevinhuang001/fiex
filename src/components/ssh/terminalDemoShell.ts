import { t } from '@/i18n'

/** Tiny interactive shell used only by the web demo. It never executes input. */
export function attachDemoShell(terminal: { write: (value: string) => void }, cwd?: string) {
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
