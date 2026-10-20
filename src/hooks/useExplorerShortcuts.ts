import { useEffect } from 'react'
import { matchShortcut, type ShortcutAction, type ShortcutMap } from '@/lib/shortcuts'

interface Options {
  shortcuts: ShortcutMap
  blocked: boolean
  handlers: Record<ShortcutAction, (event: KeyboardEvent) => void>
  onEscape: () => void
}

/** Route shortcuts only when the focused control does not own the key. */
export function useExplorerShortcuts({ shortcuts, blocked, handlers, onEscape }: Options) {
  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      const target = event.target instanceof Element ? event.target : null
      if (
        blocked ||
        event.defaultPrevented ||
        event.isComposing ||
        target?.closest('[role=dialog],[role=menu]')
      )
        return
      if (
        (event.key === 'Enter' || event.key === ' ') &&
        target?.closest('button,a,select,[role=button],[role=tab]')
      )
        return
      const action = matchShortcut(event, shortcuts)
      if (
        target?.closest('input,textarea,select,[contenteditable=true]') &&
        !(
          (event.ctrlKey || event.metaKey) &&
          (action === 'goToFolder' ||
            action === 'search' ||
            action === 'settings' ||
            action === 'newTab')
        )
      )
        return
      if (action) {
        event.preventDefault()
        handlers[action](event)
      } else if (event.key === 'Escape') onEscape()
    }
    document.addEventListener('keydown', handle)
    return () => document.removeEventListener('keydown', handle)
  }, [shortcuts, blocked, handlers, onEscape])
}
