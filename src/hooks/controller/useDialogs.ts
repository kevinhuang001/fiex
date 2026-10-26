import type { Explorer } from '@/hooks/useExplorer'
import type { DialogState } from '@/lib/dialogs'
import type { Entry, Tag, RemoteConnection } from '@/lib/types'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

/** Dialog and overlay state: the modal dialog with its draft value/error/busy
 *  flags, the preview/info/settings/tag/connection overlays, the context menu,
 *  the toast, and preview navigation across the current listing. */
export function useDialogs(x: Explorer) {
  const [sidebar, setSidebar] = useState(true)
  const [preview, setPreview] = useState<Entry | null>(null)
  const [settings, setSettings] = useState(false)
  const [pluginManager, setPluginManager] = useState(false)
  const [info, setInfo] = useState<Entry | null>(null)
  const [dialog, setDialog] = useState<DialogState | null>(null)
  const [dialogValue, setDialogValue] = useState('')
  const [dialogError, setDialogError] = useState('')
  const [dialogBusy, setDialogBusy] = useState(false)
  const [toast, setToast] = useState('')
  const [trashIssue, setTrashIssue] = useState(false)
  const [tagOpen, setTagOpen] = useState(false)
  const [connectionOpen, setConnectionOpen] = useState(false)
  const [connectionToEdit, setConnectionToEdit] = useState<RemoteConnection | null>(null)
  const [tagPaths, setTagPaths] = useState<string[]>([])
  const [tagInitial, setTagInitial] = useState<Tag[]>([])
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry } | null>(null)
  const searchInput = useRef<HTMLInputElement>(null)
  const chosen = useMemo(() => {
    const selected = new Set(x.selected)
    return x.entries.filter((entry) => selected.has(entry.path))
  }, [x.entries, x.selected])
  const first = chosen[0]
  const notify = useCallback((s: string) => setToast(s), [])
  useEffect(() => {
    if (!toast) return
    const timer = setTimeout(() => setToast(''), 5000)
    return () => clearTimeout(timer)
  }, [toast])
  useEffect(() => {
    if (dialog) {
      setDialogValue(dialog.value || '')
      setDialogError('')
    }
  }, [dialog])
  const navigatePreview = (delta: number) => {
    if (!preview) return
    const i = x.entries.findIndex((e) => e.path === preview.path)
    const next = x.entries[(i + delta + x.entries.length) % x.entries.length]
    if (next) {
      setPreview(next)
      x.setSelected([next.path])
    }
  }
  return {
    sidebar,
    setSidebar,
    preview,
    setPreview,
    settings,
    setSettings,
    pluginManager,
    setPluginManager,
    info,
    setInfo,
    dialog,
    setDialog,
    dialogValue,
    setDialogValue,
    dialogError,
    setDialogError,
    dialogBusy,
    setDialogBusy,
    toast,
    setToast,
    trashIssue,
    setTrashIssue,
    tagOpen,
    setTagOpen,
    connectionOpen,
    setConnectionOpen,
    connectionToEdit,
    setConnectionToEdit,
    tagPaths,
    setTagPaths,
    tagInitial,
    setTagInitial,
    menu,
    setMenu,
    searchInput,
    chosen,
    first,
    notify,
    navigatePreview,
  }
}
