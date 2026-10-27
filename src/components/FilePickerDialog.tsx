import { t } from '@/i18n'
import { useSyncExternalStore } from 'react'
import { createPortal } from 'react-dom'
import { overlayRoot } from '@/lib/portal'
import { Modal } from '@/components/ui/Modal'
import { currentFilePicker, subscribeFilePicker, type FilePickerRequest } from '@/lib/filePicker'

import { PickerDialogs } from './file-picker/PickerDialogs'
import { PickerEntries } from './file-picker/PickerEntries'
import { PickerFooter } from './file-picker/PickerFooter'
import { PickerMenu } from './file-picker/PickerMenu'
import { PickerSidebar } from './file-picker/PickerSidebar'
import { PickerToolbar } from './file-picker/PickerToolbar'
import { usePicker } from './file-picker/usePicker'

export function FilePickerDialog() {
  const pending = useSyncExternalStore(subscribeFilePicker, currentFilePicker, () => null)
  if (!pending) return null
  return (
    <Picker
      key={`${pending.request.mode}:${pending.request.directory || ''}`}
      request={pending.request}
    />
  )
}

function Picker({ request }: { request: FilePickerRequest }) {
  const picker = usePicker(request)
  return (
    <Modal
      open
      onClose={picker.cancel}
      title={
        request.mode === 'saveFile'
          ? t('保存文件')
          : request.mode === 'openDirectory'
            ? t('选择文件夹')
            : request.mode === 'openFiles'
              ? t('选择文件')
              : t('打开文件')
      }
      wide
      className="file-picker"
    >
      <div className="file-picker-body">
        <PickerSidebar
          directory={picker.directory}
          places={picker.places}
          favorites={picker.favorites}
          connections={picker.connections}
          disks={picker.disks}
          navigate={picker.navigate}
        />
        <section className="file-picker-main">
          <PickerToolbar
            directory={picker.directory}
            canBack={picker.index > 0}
            canForward={picker.index < picker.history.length - 1}
            navigate={picker.navigate}
            onBack={picker.back}
            onForward={picker.forward}
            onUp={picker.up}
            onMore={picker.openBlankMenu}
            filter={picker.filter}
            onFilter={picker.setFilter}
          />
          <PickerEntries
            view={picker.view}
            error={picker.error}
            visible={picker.visible}
            grouped={picker.grouped}
            selected={picker.selected}
            onToggle={picker.toggle}
            onOpen={picker.openEntry}
            onContextMenu={picker.contextMenu}
          />
        </section>
      </div>
      {picker.menu &&
        createPortal(
          <PickerMenu
            menu={picker.menu}
            sort={picker.sort}
            group={picker.group}
            view={picker.view}
            position={picker.menuPosition}
            menuRef={picker.menuRef}
            actions={{
              open: picker.openEntry,
              rename: picker.requestRename,
              trash: picker.requestTrash,
              newFolder: picker.requestNewFolder,
              refresh: picker.refresh,
              toggleView: picker.toggleView,
              sortBy: picker.sortBy,
              groupBy: picker.groupBy,
              close: picker.closeMenu,
            }}
          />,
          overlayRoot() ?? document.body,
        )}
      <PickerDialogs
        dialog={picker.dialog}
        replace={picker.replace}
        saveName={picker.saveName}
        busy={picker.busy}
        inputRef={picker.dialogInput}
        actions={{
          closeDialog: picker.closeDialog,
          setDialogValue: picker.setDialogValue,
          createFolder: picker.createFolder,
          rename: picker.renameEntry,
          trash: picker.trashEntry,
          confirmReplace: picker.confirmReplace,
          cancelReplace: picker.cancelReplace,
        }}
      />
      <PickerFooter
        mode={request.mode}
        saveName={picker.saveName}
        onSaveName={picker.setSaveName}
        nameInput={picker.nameInput}
        selectedEntries={picker.selectedEntries}
        canConfirm={picker.canConfirm}
        onCancel={picker.cancel}
        onConfirm={picker.confirm}
      />
    </Modal>
  )
}
