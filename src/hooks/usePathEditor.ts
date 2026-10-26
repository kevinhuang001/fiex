import { t } from '@/i18n'
import { call } from '@/lib/api'
import { pathCrumbs } from '@/lib/files'
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
  let crumbs: ReturnType<typeof pathCrumbs>
  try {
    crumbs = pathCrumbs(x.path, displayName)
  } catch {
    crumbs = [{ name: t('无效地址'), path: x.path }]
  }
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
