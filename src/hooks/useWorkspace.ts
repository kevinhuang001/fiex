import { useCallback, useEffect, useState } from 'react'
import type { Entry } from '@/lib/types'

const KEY = 'fiex:workspace'

function load(): Entry[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) || '[]')
    return Array.isArray(parsed)
      ? parsed.filter((item) => item && typeof item.path === 'string')
      : []
  } catch {
    return []
  }
}

/// The workspace is only a view: dropping files here keeps references, never
/// copies. It survives restarts and can be cleared in one click.
export function useWorkspace() {
  const [items, setItems] = useState<Entry[]>(load)
  const [open, setOpen] = useState(() => load().length > 0)
  useEffect(() => {
    try {
      localStorage.setItem(KEY, JSON.stringify(items))
    } catch {
      // Storage is best-effort.
    }
  }, [items])
  const add = useCallback((entries: Entry[]) => {
    setItems((current) => {
      const seen = new Set(current.map((item) => item.path))
      const additions = entries.filter((entry) => !seen.has(entry.path))
      return additions.length ? [...current, ...additions] : current
    })
  }, [])
  const remove = useCallback((path: string) => {
    setItems((current) => current.filter((item) => item.path !== path))
  }, [])
  const clear = useCallback(() => setItems([]), [])
  return { items, open, setOpen, add, remove, clear }
}
