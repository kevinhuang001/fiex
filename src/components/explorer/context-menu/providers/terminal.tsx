import { TerminalSquare } from 'lucide-react'
import type { ContextMenuProvider } from '../types'

export const terminalProvider: ContextMenuProvider = {
  id: 'builtin:terminal',
  order: 60,
  matches: () => true,
  resolve: ({ terminal, x }) => [
    {
      label: 'Open in Terminal',
      icon: <TerminalSquare />,
      hint: '',
      disabled: x.directoryReadOnly,
      action: () => terminal(),
    },
  ],
}
