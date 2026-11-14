import { t } from '@/i18n'
import { requestDirectoryRefresh } from '@/lib/listing'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { PluginPanelHost } from '@/components/plugins/PluginPanelHost'
import { FilePickerDialog } from '@/components/FilePickerDialog'
import { ConflictDialog } from '@/components/ConflictDialog'
import { ConnectionDialog } from '@/components/ConnectionDialog'
import { FileInfoDialog } from '@/components/FileInfoDialog'
import { lazy, Suspense } from 'react'
const PreferencesDialog = lazy(() =>
  import('@/components/PreferencesDialog').then((m) => ({ default: m.PreferencesDialog })),
)
const PluginManagerDialog = lazy(() =>
  import('@/components/settings/PluginManagerDialog').then((m) => ({
    default: m.PluginManagerDialog,
  })),
)
import { Preview } from '@/components/Preview'
import { TagDialog } from '@/components/TagDialog'
import { Button } from '@/components/ui/button'
import { Modal } from '@/components/ui/Modal'
import type { Explorer } from '@/features/explorer/hooks/useExplorer'
import type {
  ConflictResolution,
  Entry,
  Ide,
  Preferences,
  RemoteConnection,
  Tag,
  Terminal,
  TransferConflict,
} from '@/lib/types'
import type { DialogState } from '@/lib/dialogs'
import type { Dispatch, SetStateAction } from 'react'
import { LoaderCircle } from 'lucide-react'
export function ExplorerDialogs({
  left,
  preview,
  setPreview,
  navigatePreview,
  open,
  dialog,
  dialogBusy,
  conflict,
  cancelConflict,
  resolveConflict,
  busy,
  setDialog,
  setDialogBusy,
  setDialogError,
  dialogValue,
  x,
  setDialogValue,
  info,
  setInfo,
  tagOpen,
  tagInitial,
  setTagOpen,
  applyTags,
  connectionOpen,
  connectionToEdit,
  setConnectionOpen,
  notify,
  settings,
  setSettings,
  pluginManager,
  setPluginManager,
  prefs,
  update,
  terminals,
  ides,
export interface ExplorerDialogsProps {
  left: Pick<Explorer, 'locations'>
  x: Explorer
  preview: Entry | null
  setPreview: Dispatch<SetStateAction<Entry | null>>
  navigatePreview: (delta: number) => void
  open: (entry: Entry) => void
  dialog: DialogState | null
  dialogBusy: boolean
  conflict: { items: TransferConflict[]; moving: boolean } | null
  cancelConflict: () => void
  resolveConflict: (resolutions: ConflictResolution[]) => Promise<void>
  busy: string
  setDialog: Dispatch<SetStateAction<DialogState | null>>
  setDialogBusy: Dispatch<SetStateAction<boolean>>
  setDialogError: (error: string) => void
  dialogValue: string
  setDialogValue: Dispatch<SetStateAction<string>>
  info: Entry | null
  setInfo: (entry: Entry | null) => void
  tagOpen: boolean
  tagInitial: Tag[]
  setTagOpen: Dispatch<SetStateAction<boolean>>
  applyTags: (tags: Tag[]) => Promise<void>
  connectionOpen: boolean
  connectionToEdit: RemoteConnection | null
  setConnectionOpen: Dispatch<SetStateAction<boolean>>
  notify: (message: string) => void
  settings: boolean
  setSettings: Dispatch<SetStateAction<boolean>>
  pluginManager: boolean
  setPluginManager: Dispatch<SetStateAction<boolean>>
  prefs: Preferences
  update: (patch: Partial<Preferences>) => void
  terminals: Terminal[]
  ides: Ide[]
}

}: ExplorerDialogsProps) {  | 'left'
  | 'preview'
  | 'setPreview'
  | 'navigatePreview'
  | 'open'
  | 'dialog'
  | 'dialogBusy'
  | 'conflict'
  | 'cancelConflict'
  | 'resolveConflict'
  | 'busy'
  | 'setDialog'
  | 'setDialogBusy'
  | 'setDialogError'
  | 'dialogValue'
  | 'x'
  | 'setDialogValue'
  | 'info'
  | 'setInfo'
  | 'tagOpen'
  | 'tagInitial'
  | 'setTagOpen'
  | 'applyTags'
  | 'connectionOpen'
  | 'connectionToEdit'
  | 'setConnectionOpen'
  | 'notify'
  | 'settings'
  | 'setSettings'
  | 'pluginManager'
  | 'setPluginManager'
  | 'prefs'
  | 'update'
  | 'terminals'
  | 'ides'
>) {
  return (
    <>
      <PluginPanelHost />
      <FilePickerDialog />
      <ConflictDialog
        request={conflict}
        busy={Boolean(busy)}
        onClose={cancelConflict}
        onResolve={resolveConflict}
      />
      <Preview
        entry={preview}
        onClose={() => setPreview(null)}
        onNavigate={navigatePreview}
        onOpen={open}
      />

      <Modal
        open={!!dialog}
        onClose={() => {
          if (!dialogBusy) setDialog(null)
        }}
        title={dialog?.title || ''}
      >
        <form
          onSubmit={async (e) => {
            e.preventDefault()
            if (!dialog || dialogBusy) return
            setDialogBusy(true)
            setDialogError('')
            try {
              await dialog.action(dialogValue)
              setDialog(null)
            } catch (err) {
              reportError(err, 'explorer:dialog')
              requestDirectoryRefresh([x.path])
            } finally {
              setDialogBusy(false)
            }
          }}
        >
          {dialog?.label && (
            <label className="field-label">
              {dialog.label}
              <input
                className="text-field"
                autoFocus
                value={dialogValue}
                onFocus={(e) => e.target.select()}
                onChange={(e) => setDialogValue(e.target.value)}
              />
            </label>
          )}
          <div className="dialog-actions">
            <Button
              type="button"
              variant="ghost"
              disabled={dialogBusy}
              onClick={() => setDialog(null)}
            >
              {t('取消')}
            </Button>
            <Button
              type="submit"
              variant={dialog?.danger ? 'destructive' : 'default'}
              disabled={dialogBusy || (!!dialog?.label && !dialogValue.trim())}
            >
              {dialogBusy && <LoaderCircle className="spin" />}
              {dialogBusy ? t('正在处理…') : dialog?.submit}
            </Button>
          </div>
        </form>
      </Modal>

      <FileInfoDialog info={info} onClose={() => setInfo(null)} />

      <TagDialog
        open={tagOpen}
        initial={tagInitial}
        available={prefs.tags}
        onClose={() => setTagOpen(false)}
        onApply={applyTags}
      />

      <ConnectionDialog
        open={connectionOpen}
        connection={connectionToEdit}
        onClose={() => setConnectionOpen(false)}
        onSaved={(connection) => {
          setConnectionOpen(false)
          void x.refreshConnections()
          if (connectionToEdit) {
            notify(t('连接已保存，更改将在下次连接时生效'))
            return
          }
          // A connection may open a pluginView instead of a VFS mount. Resolve
          // the destination first; the stored mountPath is not a VFS URI for
          // pluginView connections such as Email.
          void call('connection_open', { id: connection.id })
            .then((target) => {
              x.navigate(
                target.kind === 'vfs'
                  ? target.path || connection.mountPath
                  : 'fiex-plugin-view://' + connection.id,
              )
            })
            .catch((error) => x.setError(String(error)))
          notify(t('已连接 {0}', connection.name))
        }}
      />

      {settings && (
        <Suspense fallback={null}>
          <PreferencesDialog
            open={settings}
            onClose={() => setSettings(false)}
            prefs={prefs}
            locations={left.locations}
            update={update}
            terminals={terminals}
            ides={ides}
            onError={x.setError}
          />
        </Suspense>
      )}
      {pluginManager && (
        <Suspense fallback={null}>
          <PluginManagerDialog open={pluginManager} onClose={() => setPluginManager(false)} />
        </Suspense>
      )}
    </>
  )
}
