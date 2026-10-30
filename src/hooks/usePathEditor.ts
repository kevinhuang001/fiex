import { t } from '@/i18n'
import { displayAddress, normalizeAddress, pathCrumbs } from '@/lib/files'
import { useEffect, useRef, useState } from 'react'
import type { Explorer } from './useExplorer'

export function usePathEditor({
  x,
  displayName,
}: {
  x: Explorer
  displayName: (path: string) => string
}) {
  const [editingPath, setEditingPath] = useState(false)
  const [pathDraft, setPathDraft] = useState('')
  const pathInput = useRef<HTMLInputElement>(null)
  const startPathEdit = () => {
    setPathDraft(displayAddress(x.path))
    setEditingPath(true)
  }
  useEffect(() => {
    if (!editingPath) return
    const input = pathInput.current
    input?.focus()
    input?.select()
  }, [editingPath])
  const submitPath = (event: React.FormEvent) => {
    event.preventDefault()
    const target = normalizeAddress(pathDraft)
    if (!target) return
    // Navigation goes through the same cached directory stream as the
    // sidebar and history, so an already-cached path paints immediately.
    setEditingPath(false)
    x.navigate(target)
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
