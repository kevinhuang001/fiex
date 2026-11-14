import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import { t } from '@/i18n'
import type { Entry, Ide, Preferences } from '@/lib/types'
import { finishActivity } from '@/hooks/useActivity'
import { useFileOperations } from '@/hooks/useFileOperations'
import { usePathEditor } from '@/hooks/usePathEditor'
import { useWorkspaceActions } from './useWorkspaceActions'
import { useConnectionActions } from './useConnectionActions'
import { useDropTargets } from './useDropTargets'
import { useOpenActions } from './useOpenActions'
import { useShortcuts } from './useShortcuts'
import type { useDialogs } from './useDialogs'
import type { useWorkspace } from '@/hooks/useWorkspace'

type Dialogs = ReturnType<typeof useDialogs>
type Workspace = ReturnType<typeof useWorkspace>

interface ControllerActionOptions {
  x: Explorer
  left: Explorer
  right: Explorer
  prefs: Preferences
  update: (patch: Partial<Preferences>) => void
  workspace: Pick<Workspace, 'add'>
  notify: Dialogs['notify']
  first: Entry | undefined
  ides: Ide[]
  trashPath: string | null
  dialogs: Dialogs
  focusSearch: () => void
  newWorkspaceTab: () => void
  closeWorkspaceTab: (index: number) => void
  split: boolean
}

/** Combines file, connection, navigation and shortcut actions for the app shell. */
export function useAppControllerActions({
  x,
  left,
  right,
  prefs,
  update,
  workspace,
  notify,
  first,
  ides,
  trashPath,
  dialogs,
  focusSearch,
  newWorkspaceTab,
  closeWorkspaceTab,
  split,
}: ControllerActionOptions) {
  const files = useFileOperations({ x, notify, setDialog: dialogs.setDialog, trashPath })
  const cancelConflict = () => {
    if (files.conflict?.activityId) finishActivity(files.conflict.activityId, t('已取消'))
    files.setConflict(null)
  }
  const connection = useConnectionActions({
    x,
    left,
    right,
    run: files.run,
    notify,
    setDialog: dialogs.setDialog,
    trashPath,
  })
  const opening = useOpenActions({
    x,
    prefs,
    run: files.run,
    notify,
    first,
    ides,
    isRemotePath: connection.isRemotePath,
    tagPaths: dialogs.tagPaths,
    setTagPaths: dialogs.setTagPaths,
    setTagInitial: dialogs.setTagInitial,
    setTagOpen: dialogs.setTagOpen,
    setPreview: dialogs.setPreview,
    trashPath,
  })
  const path = usePathEditor({ x: left, displayName: connection.displayName })
  const workspaceActions = useWorkspaceActions({
    x,
    prefs,
    update,
    workspace,
    notify,
    first,
    left,
    right,
    split,
  })
  const { hint } = useShortcuts({
    prefs,
    x,
    left,
    first,
    dialog: dialogs.dialog,
    conflict: files.conflict,
    settings: dialogs.settings,
    pluginManager: dialogs.pluginManager,
    preview: dialogs.preview,
    info: dialogs.info,
    tagOpen: dialogs.tagOpen,
    connectionOpen: dialogs.connectionOpen,
    focusSearch,
    startPathEdit: path.startPathEdit,
    copy: files.copy,
    clipboard: files.clipboard,
    transfer: files.transfer,
    undo: files.undo,
    create: files.create,
    createFile: files.createFile,
    newWorkspaceTab,
    closeWorkspaceTab,
    duplicate: files.duplicate,
    previewEntry: opening.previewEntry,
    open: opening.open,
    recycle: files.recycle,
    refreshAll: workspaceActions.refreshAll,
    setSidebar: dialogs.setSidebar,
    setInfo: dialogs.setInfo,
    setSettings: dialogs.setSettings,
    setMenu: dialogs.setMenu,
  })
  const { dragging } = useDropTargets({
    x,
    conflict: files.conflict,
    transfer: files.transfer,
    bookmarkPaths: workspaceActions.bookmarkPaths,
    addWorkspace: workspaceActions.addWorkspace,
  })

  return {
    ...files,
    ...connection,
    ...opening,
    ...path,
    ...workspaceActions,
    cancelConflict,
    hint,
    dragging,
  }
}
