# Code structure

Fiex has one React frontend and one Rust library. Tauri commands form the boundary between them; command names and serialized payloads remain stable when implementation modules move.

## Frontend

- `App.tsx` composes the sidebar, explorer chrome, browser, Git panel and overlays.
- `components/explorer/` owns toolbar, tabs, address/search bar, status bar, context menu and dialog composition. Their props select the controller capabilities each component needs.
- `hooks/useAppController.tsx` coordinates application state, device/remote actions, tags, bookmarks and previews.
- `hooks/useFileOperations.ts` owns the operation runner, clipboard, creation, recycling, duplication and archives.
- `hooks/usePathEditor.ts` owns address editing, validation and breadcrumbs.
- `hooks/useExplorerShortcuts.ts` owns focus-aware keyboard dispatch; the controller supplies an exhaustive action map.
- `hooks/useDesktopEnvironment.ts` detects native platform integrations.
- `hooks/useExplorer.ts` owns directory listing, search, selection, navigation, tabs and filesystem events.
- `components/settings/` contains independent settings sections, including local shortcut recording and sidebar reordering state. `PreferencesDialog.tsx` only hosts category navigation and the integration help dialog.
- `lib/archives.ts` defines archive formats once for both commands and menus. `lib/dialogs.ts` defines the operation dialog contract.
- `lib/api.ts` selects native IPC or the explicit `?demo` fixture adapter. Demo fixtures are used by browser regression tests and recordings; they do not replace native integration tests.

## Native library

- `files.rs`: entry metadata, directory listing, filename validation and text previews.
- `tags.rs`: Finder tag data and macOS extended attributes.
- `volumes.rs`: common locations, mounted/unmounted disks, device mounting and Trash discovery, with platform-specific implementations guarded by `cfg`.
- `operations.rs`, `archives.rs`, `search.rs`, `remote.rs`, `usbfs.rs`, `git.rs` and `terminals.rs` retain their focused domains.
- `desktop.rs`: Tauri command adapters and application startup. Callers reference the owning native module directly; moved APIs have no compatibility re-exports.

The obsolete icon-size preference migration has been removed. PDF.js's `legacy/build` import is the upstream distribution used for WebView compatibility, not an old Fiex implementation.

## Validation

Run `npm run format:check`, `npm test`, `npm run build` and `npm run e2e`. CI additionally formats, tests, lints and packages the native library on Windows, Linux and both macOS architectures, including WebDAV/SMB and filesystem-driver tests.

To record the fixture-backed UI, run `npm run demo:record` (it starts its own local Vite server). Set `FIEX_TEST_CHROMIUM` for a custom browser executable. The resulting WebM is written to `artifacts/demo/` and covers navigation, views, search, file creation/renaming, previews, Git, remote connection settings and preferences.
