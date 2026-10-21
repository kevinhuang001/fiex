import { call } from '@/lib/api'
import { join } from '@/lib/files'
import type { Preferences } from '@/lib/types'
import { useEffect, useRef, useState } from 'react'
import type { Explorer } from './useExplorer'

export function usePathEditor({
  x,
  prefs,
  displayName,
}: {
  x: Explorer
  prefs: Preferences
  displayName: (path: string) => string
}) {
  const [editingPath, setEditingPath] = useState(false)
  const [pathDraft, setPathDraft] = useState('')
  const pathInput = useRef<HTMLInputElement>(null)
  const startPathEdit = () => {
    setPathDraft(x.path)
    setEditingPath(true)
  }
  useEffect(() => {
    if (!editingPath) return
    const input = pathInput.current
    input?.focus()
    input?.select()
  }, [editingPath])
  const submitPath = async (event: React.FormEvent) => {
    event.preventDefault()
    const target = pathDraft.trim()
    if (!target) return
    try {
      const { path } = await call<{ path: string }>('list_directory', {
        path: target,
        hidden: prefs.hidden,
        ignores: prefs.displayIgnore,
      })
      setEditingPath(false)
      x.navigate(path)
    } catch (error) {
      x.setError(String(error))
    }
  }
  const pathParts = x.path.split(/[\\/]/).filter(Boolean)
  let accumulated = x.path.startsWith('/') ? '' : x.path.startsWith('\\') ? '\\\\' : ''
  const crumbs = pathParts.map((name) => {
    accumulated = accumulated
      ? join(accumulated, name)
      : x.path.startsWith('/')
        ? '/' + name
        : name + '\\'
    return { name: displayName(accumulated) || name, path: accumulated }
  })
  return {
    editingPath,
    pathDraft,
    setPathDraft,
    pathInput,
    setEditingPath,
    startPathEdit,
    submitPath,
    crumbs,
  }
}
