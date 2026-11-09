import type { Channel } from '@tauri-apps/api/core'
import type { DropPlan } from './types'
import type { ActivationConsent } from './pluginPermissions'
import type { OperationProgress, Tag } from './types'
import type { JsonValue, ResourceRef } from '../../sdk/ui/src/index'
import type { ConflictResolution } from './types'
import type { DirectoryViewUpdate } from './listing'
import type { StateGetArgs } from './tauriCommandMap'

type NoCommandArgs = { readonly __noArgs?: never }
export type JsonInput = JsonValue | object

export interface SearchCommandInput {
  pane: string
  root: string
  query: string
  hidden: boolean
  ignores: string[]
  mode?: 'name' | 'content'
  engine?: string
}

export interface RemoteConnectionInput {
  id?: string | null
  name: string
  kind: string
  pluginId?: string | null
  settings?: Record<string, JsonValue>
}

export interface PluginResourceBinding {
  ref: ResourceRef
  path: string
}

export interface PluginDecorationEntry {
  path: string
  name: string
  isDir: boolean
  extension?: string
  mime?: string
  allowedOps?: string[]
  tags?: Array<{ name: string; color: number; hex?: string }>
  sync?: string | null
  hasCache?: boolean
  pinned?: boolean
  uploaded?: boolean
  linkTarget?: string | null
}

export interface TauriCommandArgs {
  begin_operation: NoCommandArgs
  cancel_operation: NoCommandArgs
  cancel_search: { pane: string }
  ci_log: { message: string }
  ci_reply: { id: number; value: string }
  connection_open: { id: string }
  create_entry: { path: string; name: string }
  create_file_entry: { path: string; name: string }
  credential_backend: NoCommandArgs
  credential_backend_set: { backend: string }
  delete_permanently: { paths: string[] }
  detect_ides: NoCommandArgs
  detect_shells: NoCommandArgs
  detect_terminals: NoCommandArgs
  directory_open: {
    path: string
    hidden: boolean
    ignores: string[]
    onSnapshot: Channel<DirectoryViewUpdate>
  }
  drag_files: { paths: string[]; onEvent: Channel<string> }
  eject_device: { path: string }
  eject_usb: { device: string }
  empty_trash: NoCommandArgs
  errors_pending: { after?: number | null }
  file_info: { path: string }
  file_info_cancel: { sessionId: string; taskId: string }
  file_info_compute: {
    sessionId: string
    providerId: string
    fields: Array<{ sectionId: string; fieldId: string }>
    taskId: string
  }
  file_info_open: { path: string; onUpdate?: Channel<unknown> }
  file_info_session_close: { sessionId: string }
  file_parent: { path: string }
  finish_drag: NoCommandArgs
  initial_path: NoCommandArgs
  internal_drop: {
    paths: string[]
    consume: boolean
    captured?: DropPlan | null
  }
  launch_path: NoCommandArgs
  list_directory: { path: string; hidden: boolean; ignores: string[] }
  local_terminal_open: {
    cwd?: string | null
    shell?: string | null
    cols: number
    rows: number
    onEvent: Channel<import('@/components/terminal/sessionEvents').TerminalEvent>
  }
  locale_set: { locale: string }
  locations: NoCommandArgs
  mount_device: { device: string }
  native_icon: { path: string; size?: number | null }
  new_window: { path: string }
  open_external_url: { url: string }
  open_full_disk_access: NoCommandArgs
  open_in_ide: { path: string; id: string }
  open_path: { path: string }
  open_terminal: { path: string; id: string }
  open_with_picker_candidates: { path: string }
  open_with_picker_execute: { path: string; sessionId: string; itemId: string }
  pick_folder: { default?: string | null }
  platform: NoCommandArgs
  plugin_activate: { pluginId: string; consent: ActivationConsent }
  plugin_approve_install: { pluginId: string }
  plugin_before_open: { path: string }
  plugin_cancel_install: { ticket: string }
  plugin_cancel_task: { pluginId: string; taskId: string }
  plugin_confirm_install: { ticket: string; allowUntrusted?: boolean | null }
  plugin_connection_menu_execute: {
    connectionId: string
    providerId: string
    itemId: string
    menuSessionId?: string | null
  }
  plugin_connection_menu_resolve: { connectionId: string }
  plugin_context_directory: { pluginId: string; path: string }
  plugin_context_menu_children: { menuSessionId: string; parentId: string }
  plugin_context_menu_execute: {
    menuSessionId: string
    itemId: string
    onProgress?: Channel<OperationProgress>
  }
  plugin_context_menu_resolve: {
    directory: string
    paths: string[]
    source?: string | null
    sessionId?: string | null
  }
  plugin_decorations: {
    directory: string
    entries: PluginDecorationEntry[]
    includePlugins?: boolean | null
  }
  plugin_describe_resources: { pluginId: string; paths: string[] }
  plugin_file_dialog: {
    mode: 'openFile' | 'openDirectory' | 'saveFile'
    directory?: string | null
    suggestedName?: string | null
    extensions?: string[] | null
  }
  plugin_files_dialog: { directory?: string | null; extensions?: string[] | null }
  plugin_invoke: {
    pluginId: string
    method: string
    params: JsonInput
    directory?: string | null
    selectedPaths?: string[] | null
    previewSessionId?: string | null
    resourceBindings?: PluginResourceBinding[] | null
  }
  plugin_logs: { pluginId: string; after?: number | null }
  plugin_logs_clear: { pluginId: string }
  plugin_marketplace: { repository?: string | null }
  plugin_network_domains: { pluginId: string }
  plugin_panel_asset: { pluginId: string; panelId: string }
  plugin_panel_html: { pluginId: string; panelId: string }
  plugin_permission_allowed: { pluginId: string; permission: string }
  plugin_permission_respond: { requestId: string; allow: boolean }
  plugin_prepare_install: { repository: string; assetName: string }
  plugin_prepare_local_install: { path: string }
  plugin_release_assets: { repository: string }
  plugin_secret_set: { pluginId: string; scope: string; key: string; value: string }
  plugin_secret_available: NoCommandArgs
  plugin_set_command: { pluginId: string; command: string; granted: boolean }
  plugin_set_enabled: { pluginId: string; enabled: boolean }
  plugin_set_network_all: { pluginId: string; all: boolean }
  plugin_set_network_domain: { pluginId: string; domain: string; granted: boolean }
  plugin_set_permission: { pluginId: string; permission: string; granted: boolean }
  plugin_status: NoCommandArgs
  plugin_storage_delete: { pluginId: string; scope: string; key: string }
  plugin_storage_get: { pluginId: string; scope: string; key: string }
  plugin_settings: { pluginId: string }
  plugin_storage_set: { pluginId: string; scope: string; key: string; value: JsonInput }
  plugin_task: { pluginId: string; taskId: string }
  plugin_themes: NoCommandArgs
  plugin_toolbar_invoke: { pluginId: string; directory: string; paths: string[] }
  plugin_uninstall: { pluginId: string }
  plugin_view_asset: { pluginId: string; entry: string }
  plugins_list: NoCommandArgs
  preview_candidates: { path: string }
  preview_loading_asset: { pluginId: string }
  preview_open: { path: string; providerId?: string | null; providerOrder?: string[] | null }
  provider_catalog: NoCommandArgs
  provider_preferences: NoCommandArgs
  read_preview: { path: string }
  recent_navigation_target: { alias: string }
  recycle: { paths: string[]; onProgress?: Channel<OperationProgress> }
  register_drag: { paths: string[]; copyRequested: boolean }
  remote_cache: { paths: string[]; onProgress: Channel<OperationProgress> }
  remote_cached: { path: string }
  remote_connection_config: { id: string }
  remote_connections: NoCommandArgs
  remote_download: { path: string; onProgress: Channel<OperationProgress> }
  remote_download_dir: { path: string }
  remote_open: { path: string; onProgress: Channel<OperationProgress> }
  remote_resolve: { path: string; keepLocal: boolean }
  remote_set_sync_interval: { seconds: number }
  remote_sync: { paths: string[]; onProgress: Channel<OperationProgress> }
  remote_unpin: { paths: string[] }
  remote_uncache: { paths: string[] }
  remove_remote_connection: { id: string }
  relaunch_elevated: NoCommandArgs
  rename_entry: { path: string; name: string }
  reset_provider_preference: { feature: string }
  restore_from_trash: { paths: string[]; fallback?: string | null }
  save_remote_connection: { input: RemoteConnectionInput }
  search_files: { request: SearchCommandInput; onBatch: Channel<unknown> }
  search_providers: { root: string; mode?: string | null }
  set_provider_preference: { feature: string; order: string[]; disabled: string[] }
  set_tags: { paths: string[]; tags: Tag[] }
  state_get: StateGetArgs
  state_put: { namespace: string; key: string; value: JsonInput }
  state_remove: { namespace: string; key: string }
  system_fonts: NoCommandArgs
  system_proxy_info: NoCommandArgs
  terminal_close: { sessionId: string }
  terminal_resize: { sessionId: string; cols: number; rows: number }
  terminal_write: { sessionId: string; data: number[] }
  transfer: {
    paths: string[]
    destination: string
    moving: boolean
    keepBoth: boolean
    ignores: string[]
    resolutions?: ConflictResolution[] | null
    onProgress: Channel<OperationProgress>
  }
  trash_path: NoCommandArgs
  trash_restore_needs_folder: { paths: string[] }
  ui_error_report: {
    message: string
    detail?: string | null
    pluginId?: string | null
    source?: string | null
  }
  undo: NoCommandArgs
  unwatch_directory: { path: string; pane: string }
  update_connection_session: { id: string; saved?: boolean | null; autostart?: boolean | null }
  vfs_retain_views: { paths: string[]; durable: string[] }
  watch_directory: { path: string; pane: string }
}

export type TauriCommandArgument<C extends keyof TauriCommandArgs> = TauriCommandArgs[C]
