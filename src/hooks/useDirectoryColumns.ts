import { reportError } from '@/lib/errorCenter'
import { useEffect, useRef, useState } from 'react'
import { call } from '@/lib/api'
import { normalizeListColumns, type ListColumns } from '@/lib/listColumns'

/** Explicit per-directory widths; global preferences supply the initial default. */
export function useDirectoryColumns(path: string, fallback: ListColumns) {
  const [state, setState] = useState<{ path: string; value: ListColumns } | null>(null)
  const serial = useRef(Promise.resolve())
  const edits = useRef(0)
  const keyFor = async (directory: string) => {
    const bytes = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(directory))
    return (
      'columns.' + [...new Uint8Array(bytes)].map((b) => b.toString(16).padStart(2, '0')).join('')
    )
  }
  useEffect(() => {
    let active = true
    const version = edits.current
    void keyFor(path)
      .then((key) => call<ListColumns | null>('state_get', { namespace: 'layout', key }))
      .then((value) => {
        if (active && version === edits.current)
          setState({ path, value: value ? normalizeListColumns(value) : fallback })
      })
      .catch(() => {
        if (active && version === edits.current) setState({ path, value: fallback })
      })
    return () => {
      active = false
    }
  }, [path])
  const update = (value: ListColumns) => {
    edits.current += 1
    setState({ path, value })
    const target = path
    serial.current = serial.current
      .catch(() => {})
      .then(async () => {
        const key = await keyFor(target)
        await call('state_put', { namespace: 'layout', key, value })
      })
      .catch((cause) => reportError(cause, 'state'))
  }
  return [state?.path === path ? state.value : fallback, update] as const
}
