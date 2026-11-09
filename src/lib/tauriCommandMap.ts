import type { JsonValue } from '../../sdk/ui/src/index'
import type { CoreMenu } from '@/components/explorer/context-menu/useContextMenu'
import type { PickerResponse } from '@/components/explorer/context-menu/OpenWithPicker'
import type { InfoFieldDelta, InfoOpenResult } from '@/components/FileInfoDialog'
import type { PluginMenu } from '@/components/plugins/PluginMenuNode'
import type { Catalog } from '@/components/settings/ProviderSettings'
import type { InstallPreview } from '@/components/settings/installer/types'
import type { LocalPreview } from '@/components/settings/plugins/PluginAlerts'
import type { LogRecord } from '@/components/settings/plugins/PluginLogViewer'
import type { ShellChoice } from '@/components/terminal/TerminalPanel'
import type { ConnectionMenu } from '@/components/sidebar/Connections'
import type { DropPlan } from './types'
import type { BeforeOpenOutcome } from './pluginHooks'
import type { PreviewCandidate } from './preview'
import type { ResolvedPreview } from '@/components/preview/types'
import type { ListColumns } from './listColumns'
import type { Platform } from './platform'
import type { Theme } from './themes'
import type {
  Batch,
  Entry,
  EntryDecoration,
  Ide,
  Listing,
  Location,
  OperationResult,
  RemoteConnection,
  Terminal,
} from './types'
import type { MarketplacePlugin, PluginInfo } from './plugins'
import type { TauriCommandArgs } from './tauriCommandArgs'

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

/** Results owned by Tauri commands. Dynamic plugin and persisted JSON is
 * represented as JsonValue and decoded before it crosses the UI boundary. */
export interface TauriCommandResults {
  begin_operation: void
  cancel_operation: void
  cancel_search: void
  ci_log: void
  ci_reply: void
  connection_open: ConnectionOpenResult
  create_entry: string
  create_file_entry: string
  credential_backend: string
  credential_backend_set: void
  delete_permanently: number
  detect_ides: Ide[]
  detect_shells: ShellChoice[]
  detect_terminals: Terminal[]
  directory_open: void
  drag_files: void
  eject_device: void
  eject_usb: void
  empty_trash: number
  errors_pending: Array<{ id: number; source: string; code: string; message: string }>
  file_info: Entry
  file_info_cancel: void
  file_info_compute: InfoFieldDelta
  file_info_open: InfoOpenResult
  file_info_session_close: void
  file_parent: string
  open_full_disk_access: void
  pick_folder: string | null
  finish_drag: void
  initial_path: string | null
  internal_drop: DropPlan | null
  launch_path: string | null
  list_directory: Listing
  local_terminal_open: string
  locale_set: void
  locations: Location[]
  mount_device: string
  native_icon: string
  new_window: void
  open_external_url: void
  open_in_ide: void
  open_path: void
  open_terminal: void
  open_with_picker_candidates: PickerResponse
  open_with_picker_execute: void
  platform: Platform
  plugin_activate: void
  plugin_approve_install: void
  plugin_before_open: BeforeOpenOutcome
  plugin_cancel_install: void
  plugin_cancel_task: void
  plugin_confirm_install: string
  plugin_connection_menu_execute: JsonValue
  plugin_connection_menu_resolve: ConnectionMenu[]
  plugin_context_directory: string
  plugin_context_menu_children: { items: JsonValue[] }
  plugin_context_menu_execute: JsonValue
  plugin_context_menu_resolve: CoreMenu[] | PluginMenu[]
  plugin_decorations: EntryDecoration[]
  plugin_describe_resources: import('../../sdk/ui/src/index').ResourceContext[]
  plugin_file_dialog: string | null
  plugin_files_dialog: string[] | null
  plugin_invoke: JsonValue
  plugin_logs: { entries: LogRecord[]; cursor: number }
  plugin_logs_clear: void
  plugin_network_domains: string[]
  plugin_marketplace: MarketplacePlugin[]
  plugin_panel_asset: string
  plugin_panel_html: string
  plugin_permission_respond: void
  plugin_permission_allowed: boolean
  plugin_prepare_install: InstallPreview
  plugin_prepare_local_install: LocalPreview
  plugin_release_assets: string[]
  plugin_secret_available: boolean
  plugin_secret_set: void
  plugin_set_command: void
  plugin_set_enabled: void
  plugin_set_network_all: void
  plugin_set_network_domain: void
  plugin_set_permission: void
  plugin_settings: Record<string, JsonValue>
  plugin_storage_get: JsonValue | null
  plugin_storage_set: void
  plugin_storage_delete: void
  plugin_task: JsonValue | null
  plugin_status: Array<[string, boolean]>
  plugin_themes: Theme[]
  plugin_toolbar_invoke: JsonValue
  plugin_uninstall: void
  plugin_view_asset: string
  plugins_list: PluginInfo[]
  preview_candidates: PreviewCandidate[]
  preview_loading_asset: string | null
  preview_open: ResolvedPreview
  provider_catalog: { features: Catalog }
  provider_preferences: import('@/components/settings/ProviderSettings').ProviderPreferences
  search_providers: JsonValue[]
  read_preview: string
  recent_navigation_target: { parent: string; target: string }
  recycle: OperationResult
  register_drag: string[]
  remote_cached: boolean
  remote_connection_config: {
    id: string | null
    name: string
    kind: string
    pluginId: string | null
    settings: Record<string, string | number | boolean>
  }
  remote_connections: RemoteConnection[]
  remote_cache: void
  remote_download: string
  remote_download_dir: void
  remote_open: void
  remote_resolve: void
  remote_set_sync_interval: void
  remote_sync: void
  remote_uncache: void
  remote_unpin: void
  remove_remote_connection: void
  rename_entry: string
  reset_provider_preference: import('@/components/settings/ProviderSettings').ProviderPreferences
  restore_from_trash: number
  save_remote_connection: RemoteConnection
  search_files: Batch
  set_provider_preference: import('@/components/settings/ProviderSettings').ProviderPreferences
  set_tags: void
  state_get: JsonValue | null
  state_put: void
  state_remove: void
  system_fonts: string[]
  system_proxy_info: { enabled: boolean; source: string; proxy: string | null }
  terminal_close: void
  terminal_resize: void
  terminal_write: void
  transfer: OperationResult
  trash_path: string | null
  trash_restore_needs_folder: boolean
  ui_error_report: void
  undo: void
  unwatch_directory: void
  vfs_retain_views: void
  watch_directory: void
  update_connection_session: RemoteConnection
  relaunch_elevated: void
}

export type TauriCommand = keyof TauriCommandResults & keyof TauriCommandArgs
export type TauriCommandResult<C extends TauriCommand> = TauriCommandResults[C]
export type { TauriCommandArgs } from './tauriCommandArgs'

export interface PickerSortState {
  key: 'name' | 'modified' | 'size' | 'kind'
  direction: 'asc' | 'desc'
}

export type StateGetResult<A extends StateGetArgs> = A extends { namespace: 'layout' }
  ? ListColumns | null
  : A extends { namespace: 'explorer'; key: 'locations' }
    ? Location[] | null
    : A extends { namespace: 'explorer'; key: 'lastPath' }
      ? string | null
      : A extends { namespace: 'explorer'; key: 'preferences' }
        ? JsonValue
        : A extends { namespace: 'picker'; key: 'sort' }
          ? PickerSortState | null
          : A extends { namespace: 'picker'; key: 'group' }
            ? 'none' | 'kind' | 'modified' | 'size' | null
            : string | null
