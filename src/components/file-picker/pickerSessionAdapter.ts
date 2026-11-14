import { quietProgressChannel } from '@/hooks/useActivity'
import { call } from '@/lib/api'
import { reportError } from '@/lib/errorCenter'
import { settleFilePicker } from '@/lib/filePicker'
import { loadPickerSidebarData, pickerSidebarSnapshot } from '@/lib/filePickerData'
import { openPresentedDirectory } from '@/lib/listing'
import { browserPickerStorage } from '@/features/file-picker/services/preferences'
import type {
  PickerDirectoryServices,
  PickerOperationServices,
  PickerSessionEnvironment,
  PickerStateWrite,
} from '@/features/file-picker/types'
import type { FilePickerMode } from '@/lib/filePicker'
import type { Location } from '@/lib/types'

function writePickerState(write: PickerStateWrite) {
  switch (write.key) {
    case 'sort':
      return call('state_put', { namespace: 'picker', key: 'sort', value: write.value })
    case 'group':
      return call('state_put', { namespace: 'picker', key: 'group', value: write.value })
    case 'lastSaveDirectory':
      return call('state_put', {
        namespace: 'picker',
        key: 'lastSaveDirectory',
        value: write.value,
      })
    case 'lastOpenDirectory':
      return call('state_put', {
        namespace: 'picker',
        key: 'lastOpenDirectory',
        value: write.value,
      })
  }
}

const directoryServices: PickerDirectoryServices = {
  preferences: { storage: browserPickerStorage, writeState: writePickerState },
  readSettings: async (mode: FilePickerMode) => {
    const lastKey = mode === 'saveFile' ? 'lastSaveDirectory' : 'lastOpenDirectory'
    const [sort, group, directory] = await Promise.all([
      call('state_get', { namespace: 'picker', key: 'sort' }),
      call('state_get', { namespace: 'picker', key: 'group' }),
      call('state_get', { namespace: 'picker', key: lastKey }),
    ])
    return { sort, group, directory }
  },
  sidebarSnapshot: pickerSidebarSnapshot,
  loadSidebarData: loadPickerSidebarData,
  openDirectory: openPresentedDirectory,
  reportError,
}

const operationServices: PickerOperationServices = {
  complete: settleFilePicker,
  createFolder: (directory, name) => call('create_entry', { path: directory, name }),
  rename: (path, name) => call('rename_entry', { path, name }),
  recycle: (path) => call('recycle', { paths: [path], onProgress: quietProgressChannel() }),
  reportError,
}

export function createPickerSessionEnvironment(favorites: Location[]): PickerSessionEnvironment {
  return { favorites, directory: directoryServices, operations: operationServices }
}
