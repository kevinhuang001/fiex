import { t } from '@/i18n'
import { call } from '@/lib/api'
import { basename, displayAddress, normalizeAddress, pathCrumbs } from '@/lib/files'
import { useEffect, useRef, useState } from 'react'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'

export function usePathEditor({
  x,
  displayName,
}: {
  x: Explorer
  displayName: (path: string) => string
}) {
  const [editingPath, setEditingPath] = useState(false)
  const [pathDraft, setPathDraft] = useState('')
  const [applicationCrumbs, setApplicationCrumbs] = useState<ReturnType<typeof pathCrumbs> | null>(
    null,
  )
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
  useEffect(() => {
    const root = 'applications://'
    if (!x.path.startsWith(root) || x.path === root) {
      setApplicationCrumbs(null)
      return
    }
    setApplicationCrumbs(null)
    let cancelled = false
    const resolveAncestors = async () => {
      let current = x.path
      const seen = new Set<string>()
      const descendants: ReturnType<typeof pathCrumbs> = []
      while (current !== root && descendants.length < 128 && !seen.has(current)) {
        seen.add(current)
        const [entry, parent] = await Promise.all([
          call('file_info', { path: current }),
          call('file_parent', { path: current }),
        ])
        descendants.unshift({ name: entry.name, path: current })
        current = parent
      }
      if (!cancelled && current === root) {
        setApplicationCrumbs([{ name: basename(root), path: root }, ...descendants])
      }
    }
    void resolveAncestors().catch(() => {
      if (!cancelled) setApplicationCrumbs(null)
    })
    return () => {
      cancelled = true
    }
  }, [x.path])
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
    crumbs = applicationCrumbs ?? pathCrumbs(x.path, displayName)
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
