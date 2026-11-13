import { isUnsignedInteger } from './json'

type SimpleResultKind =
  'void' | 'emptyObject' | 'string' | 'nullableString' | 'count' | 'boolean' | 'platform'

const SIMPLE_RESULT_KINDS: Record<string, SimpleResultKind> = {
  begin_operation: 'void',
  cancel_operation: 'void',
  cancel_search: 'void',
  ci_log: 'void',
  ci_reply: 'void',
  create_entry: 'string',
  create_file_entry: 'string',
  credential_backend: 'string',
  credential_backend_set: 'void',
  delete_permanently: 'count',
  directory_open: 'void',
  drag_files: 'void',
  eject_device: 'void',
  eject_usb: 'void',
  empty_trash: 'count',
  file_info_cancel: 'void',
  file_info_session_close: 'void',
  file_parent: 'string',
  open_full_disk_access: 'void',
  pick_folder: 'nullableString',
  finish_drag: 'void',
  initial_path: 'nullableString',
  launch_path: 'nullableString',
  local_terminal_open: 'string',
  locale_set: 'void',
  mount_device: 'string',
  native_icon: 'string',
  new_window: 'void',
  open_external_url: 'void',
  open_in_ide: 'void',
  open_path: 'void',
  platform: 'platform',
  plugin_toolbar_invoke: 'emptyObject',
  open_terminal: 'void',
  open_with_picker_execute: 'void',
  plugin_activate: 'void',
  plugin_approve_install: 'void',
  plugin_cancel_install: 'void',
  plugin_cancel_task: 'void',
  plugin_confirm_install: 'string',
  plugin_context_directory: 'string',
  plugin_file_dialog: 'nullableString',
  plugin_logs_clear: 'void',
  plugin_panel_asset: 'string',
  plugin_panel_html: 'string',
  plugin_permission_respond: 'void',
  plugin_permission_allowed: 'boolean',
  plugin_secret_available: 'boolean',
  plugin_secret_set: 'void',
  plugin_set_command: 'void',
  plugin_set_enabled: 'void',
  plugin_set_network_all: 'void',
  plugin_set_network_domain: 'void',
  plugin_set_permission: 'void',
  plugin_storage_set: 'void',
  plugin_storage_delete: 'void',
  plugin_uninstall: 'void',
  plugin_view_asset: 'string',
  preview_loading_asset: 'nullableString',
  read_preview: 'string',
  remote_cached: 'boolean',
  remote_cache: 'void',
  remote_download: 'string',
  remote_download_dir: 'void',
  remote_open: 'void',
  remote_resolve: 'void',
  remote_set_sync_interval: 'void',
  remote_sync: 'void',
  remote_uncache: 'void',
  remote_unpin: 'void',
  remove_remote_connection: 'void',
  rename_entry: 'string',
  restore_from_trash: 'count',
  search_files: 'void',
  set_tags: 'void',
  state_put: 'void',
  state_remove: 'void',
  terminal_close: 'void',
  terminal_resize: 'void',
  terminal_write: 'void',
  trash_path: 'nullableString',
  trash_restore_needs_folder: 'boolean',
  ui_error_report: 'void',
  undo: 'void',
  unwatch_directory: 'void',
  vfs_retain_views: 'void',
  watch_directory: 'void',
  relaunch_elevated: 'void',
}

export function decodeSimpleIpcResult(command: string, value: unknown): { value: unknown } | null {
  const kind = SIMPLE_RESULT_KINDS[command]
  if (!kind) return null
  const invalid = () => new TypeError(`Invalid ${command} IPC result`)

  if (kind === 'void') {
    if (value !== null && value !== undefined) throw invalid()
    return { value: undefined }
  }
  if (kind === 'emptyObject') {
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).length)
      throw invalid()
    return { value: undefined }
  }
  if (kind === 'string') {
    if (typeof value !== 'string') throw invalid()
    return { value }
  }
  if (kind === 'nullableString') {
    if (value !== null && typeof value !== 'string') throw invalid()
    return { value }
  }
  if (kind === 'count') {
    if (!isUnsignedInteger(value) || !Number.isSafeInteger(value)) throw invalid()
    return { value }
  }
  if (kind === 'platform') {
    if (value !== 'macos' && value !== 'windows' && value !== 'linux') throw invalid()
    return { value }
  }
  if (typeof value !== 'boolean') throw invalid()
  return { value }
}
