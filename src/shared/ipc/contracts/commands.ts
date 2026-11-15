import type { JsonValue } from '../../../../sdk/ui/src/index'

export type ConnectionOpenResult =
  | { kind: 'vfs'; path: string }
  | {
      kind: 'pluginView'
      pluginId: string
      connectionId: string
      assetPath: string
      state: JsonValue
      tabTitle?: string
    }

export type StateGetArgs =
  | { namespace: 'layout'; key: string }
  | { namespace: 'explorer'; key: 'locations' }
  | { namespace: 'explorer'; key: 'lastPath' }
  | { namespace: 'explorer'; key: 'preferences' }
  | { namespace: 'picker'; key: 'sort' }
  | { namespace: 'picker'; key: 'group' }
  | { namespace: 'picker'; key: 'lastOpenDirectory' | 'lastSaveDirectory' }

export interface PickerSortState {
  key: 'name' | 'modified' | 'size' | 'kind'
  direction: 'asc' | 'desc'
}
