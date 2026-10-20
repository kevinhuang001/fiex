import { useEffect, useRef, useState } from 'react'
import type { Entry } from '@/lib/types'

interface Props {
  entry: Entry
  className?: string
  onCommit: (name: string) => Promise<boolean>
  onCancel: () => void
}

/**
 * Finder-style inline rename: the filename itself becomes an input, the stem
 * is selected, Return commits and Escape cancels. Focus loss commits too.
 */
export function InlineRename({ entry, className, onCommit, onCancel }: Props) {
  const [value, setValue] = useState(entry.name)
  const [busy, setBusy] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  const settled = useRef(false)

  useEffect(() => {
    const element = input.current
    if (!element) return
    element.focus()
    const dot = entry.name.lastIndexOf('.')
    const end = dot > 0 ? dot : entry.name.length
    requestAnimationFrame(() => element.setSelectionRange(0, end))
  }, [entry.path, entry.name])

  const finish = async (cancel: boolean) => {
    if (settled.current) return
    settled.current = true
    if (cancel) {
      onCancel()
      return
    }
    const next = value.trim()
    if (!next || next === entry.name) {
      onCancel()
      return
    }
    setBusy(true)
    const renamed = await onCommit(next)
    setBusy(false)
    if (!renamed) {
      settled.current = false
      input.current?.focus()
      input.current?.select()
    }
  }

  return (
    <input
      ref={input}
      className={`inline-rename ${className || ''}`}
      value={value}
      disabled={busy}
      aria-label={`重命名 ${entry.name}`}
      spellCheck={false}
      onClick={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
      onChange={(event) => setValue(event.target.value)}
      onBlur={() => void finish(false)}
      onKeyDown={(event) => {
        event.stopPropagation()
        if (event.nativeEvent.isComposing) return
        if (event.key === 'Enter') {
          event.preventDefault()
          void finish(false)
        } else if (event.key === 'Escape') {
          event.preventDefault()
          void finish(true)
        }
      }}
    />
  )
}
