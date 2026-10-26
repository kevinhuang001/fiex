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
import type { AppController } from '@/hooks/useAppController'
import { call } from '@/lib/api'
import { CheckCircle2, LoaderCircle, X } from 'lucide-react'
export function ExplorerDialogs({
  left,
  preview,
  setPreview,
  navigatePreview,
  open,
  dialog,
  dialogBusy,
  conflict,
  setConflict,
  resolveConflict,
  busy,
  setDialog,
  setDialogBusy,
  setDialogError,
  dialogValue,
  x,
  setDialogValue,
  dialogError,
  trashIssue,
  setTrashIssue,
  openTrash,
  info,
  setInfo,
  tagOpen,
  tagInitial,
  setTagOpen,
  applyTags,
  connectionOpen,
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
  toast,
  setToast,
}: Pick<
  AppController,
  | 'left'
  | 'preview'
  | 'setPreview'
  | 'navigatePreview'
  | 'open'
  | 'dialog'
  | 'dialogBusy'
  | 'conflict'
  | 'setConflict'
  | 'resolveConflict'
  | 'busy'
  | 'setDialog'
  | 'setDialogBusy'
  | 'setDialogError'
  | 'dialogValue'
  | 'x'
  | 'setDialogValue'
  | 'dialogError'
  | 'trashIssue'
  | 'setTrashIssue'
  | 'openTrash'
  | 'info'
  | 'setInfo'
  | 'tagOpen'
  | 'tagInitial'
  | 'setTagOpen'
  | 'applyTags'
  | 'connectionOpen'
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
  | 'toast'
  | 'setToast'
>) {
  return (
    <>
      <PluginPanelHost />
      <FilePickerDialog />
      <ConflictDialog
        request={conflict}
        busy={Boolean(busy)}
        onClose={() => setConflict(null)}
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
              setDialogError(String(err))
              x.reload()
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
          {dialogError && (
            <p className="error" role="alert">
              {dialogError}
            </p>
          )}
          <div className="dialog-actions">
            <Button
              type="button"
              variant="ghost"
              disabled={dialogBusy}
              onClick={() => setDialog(null)}
            >
              取消
            </Button>
            <Button
              type="submit"
              variant={dialog?.danger ? 'destructive' : 'default'}
              disabled={dialogBusy || (!!dialog?.label && !dialogValue.trim())}
            >
              {dialogBusy && <LoaderCircle className="spin" />}
              {dialogBusy ? '正在处理…' : dialog?.submit}
            </Button>
          </div>
        </form>
      </Modal>

      <Modal open={trashIssue} onClose={() => setTrashIssue(false)} title="无法浏览废纸篓">
        <div className="dialog-actions">
          <Button variant="ghost" onClick={() => setTrashIssue(false)}>
            取消
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              setTrashIssue(false)
              void call('open_trash').catch((error) => x.setError(String(error)))
            }}
          >
            在 Finder 中打开
          </Button>
          <Button
            variant="secondary"
            onClick={() => {
              setTrashIssue(false)
              setTimeout(() => void openTrash(), 300)
            }}
          >
            重试
          </Button>
          <Button
            onClick={() => {
              setTrashIssue(false)
              void call('open_full_disk_access').catch((error) => x.setError(String(error)))
            }}
          >
            打开权限设置
          </Button>
        </div>
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
        onClose={() => setConnectionOpen(false)}
        onSaved={(connection) => {
          setConnectionOpen(false)
          void x.refreshConnections()
          // A connection may open a pluginView instead of a VFS mount. Resolve
          // the destination first; the stored mountPath is not a VFS URI for
          // pluginView connections such as Email.
          void call<{
            kind: 'vfs' | 'pluginView' | 'terminal'
            path?: string
            connectionId?: string
          }>('connection_open', { id: connection.id })
            .then((target) => {
              x.navigate(
                target.kind === 'vfs'
                  ? target.path || connection.mountPath
                  : target.kind === 'pluginView'
                    ? 'fiex-plugin-view://' + connection.id
                    : 'fiex-terminal://' + (target.connectionId || connection.id),
              )
            })
            .catch((error) => x.setError(String(error)))
          notify(`已连接 ${connection.name}`)
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
      {toast && (
        <div className="toast" role="status">
          <CheckCircle2 size={16} />
          {toast}
          <button title="关闭通知" onClick={() => setToast('')}>
            <X size={13} />
          </button>
        </div>
      )}
    </>
  )
}
