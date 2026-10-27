# Code structure

> **当前实现（插件化开发分支）**：可选能力通过独立进程插件提供。Git 业务实现已移出 Core；WebDAV/SMB 通过插件读取远程目录；Native IPC 为 2.0。未完成项与平台限制见 [实现状态](plugins/12-IMPLEMENTATION-STATUS.md)。

Fiex has one React frontend and one Rust library. Tauri commands form the boundary between them; command names and serialized payloads remain stable when implementation modules move.

## Frontend

- `App.tsx` composes the sidebar, explorer chrome, browser, Git panel and overlays.
- `components/explorer/` owns toolbar, tabs, address/search bar, status bar, context menu and dialog composition. Their props select the controller capabilities each component needs.
- `hooks/useAppController.tsx` coordinates application state, device/remote actions, tags, bookmarks and previews.
- `hooks/useFileOperations.ts` owns the local operation runner, clipboard, creation, recycling and duplication. Archive/checksum actions come from plugin contributions.
- `hooks/usePathEditor.ts` owns address editing, validation and breadcrumbs.
- `hooks/useExplorerShortcuts.ts` owns focus-aware keyboard dispatch; the controller supplies an exhaustive action map.
- `hooks/useDesktopEnvironment.ts` detects native platform integrations.
- `hooks/useExplorer.ts` owns directory listing, search, selection, navigation, tabs and filesystem events.
- `components/settings/` contains independent settings sections, including local shortcut recording and sidebar reordering state. `PreferencesDialog.tsx` only hosts category navigation and the integration help dialog.
- `lib/archives.ts` defines archive formats once for both commands and menus. `lib/dialogs.ts` defines the operation dialog contract.
- `lib/api.ts` selects native IPC or the explicit `?demo` fixture adapter. Demo fixtures are used by browser regression tests; they do not replace native integration tests.

## Native library

`src-tauri/src/lib.rs` declares real Rust module trees. It has no path-mounted modules, forwarding files or old-name compatibility exports. Native command names and serialized frontend payloads are independent of Rust module paths.

| Directory | Responsibility |
| --- | --- |
| `core/` | Local file operations, transfers, search, volumes, tags, recent items, terminals and drag sessions |
| `desktop/` | Tauri adapters, windows, startup and scheduling |
| `platform/` | OS proxy, drag and macOS window integration |
| `plugins/events/` | Event contracts, matching, resolver registry, dispatch, scope preparation and common validation |
| `plugins/resolvers/` | Info, Preview, Decorations, Context Menu, Search and filesystem resolution |
| `plugins/resource/` | Resource snapshots, ref projection, scoped file access, file capabilities and operation teardown |
| `plugins/security/` | Manifest/user permission decisions, prompts, grant storage and OS sandbox |
| `plugins/host/` | Discovery, manifest validation, process lifecycle, transport and host RPC dispatch |
| `plugins/net/` | HTTP request construction, response handles, bounded upload pipe, upload lifecycle and TCP |
| `remote/` | Connection configuration, remote cache, reconciliation and synchronization |
| `usbfs/` | Filesystem drivers and their transfer adapter |

Core calls its event candidates `Resolver`; SDK implementations remain Providers. The event contract table defines the extension and collection policy once: merge (Info, Decorations, menus, Search), first supported (Preview, Hooks), or owner only (Toolbar, menu execution, connections). The router isolates resolver panics and validates each result before the feature collector sees it. Custom business RPC stays outside this event registry.

Each feature resolver owns its business validation and collector. Info separates media probes from metadata formatting; Preview separates builtin rendering from plugin session lifecycle; filesystem resolution separates mount dispatch, entry validation and stream lifecycle. `resource/fs_access/` separates scope authorization, directory operations, read handles and transactional writes. One resource lifecycle function releases file, IPC and network handles on operation/task teardown.

Resource projection creates refs for Core selections. Arbitrary business fields such as `path`, `source`, `destination` and `root` are no longer rewritten as a compatibility mechanism. The controlled filesystem API accepts refs; the host never derives authorization from a business string.

Git, WebDAV, SMB, archive, checksum and Everything implementations live in `fiex-official-plugins`. Core contains the shared protocol, permission checks and IO brokers rather than provider-specific commands.

## Validation

Run `npm run format:check`, `npm test`, `npm run build` and `npm run e2e`. CI additionally formats, tests, lints and packages the native library on Windows, Linux and Apple Silicon macOS. Real WebDAV/SMB fixtures and official plugin packaging run in the `fiex-official-plugins` repository; Core CI uses the SDK mock plugin.

Native videos are recorded only by the independent `.github/workflows/recording.yml` workflow on Windows, Linux and Intel macOS. It runs on pull requests, main pushes and manual dispatch. A separate test binary built with `ci-driver,tauri/custom-protocol` embeds the frontend and exposes a token-protected loopback control locator; real OS mouse input performs drag-and-drop. `.github/workflows/ci.yml` owns tests, production installer builds and releases, without the driver feature or a dependency on recording jobs. `native-integrations-*` artifacts include videos, screenshots, logs and verified filesystem outcomes.

## Git workflow and responsiveness

Git is an independent official plugin with a React panel built to static HTML. Core has no Git commands, repository discovery, status tinting or Git-specific paths. The plugin uses `git2`/libgit2 for status, staging, commits, refs, history, trees and remotes; it never spawns a local Git process. It declares only `filesystem.*` and `network` permissions. Remote pull is fast-forward only and refuses divergent histories; errors preserve the working tree. The plugin registers the `history` virtual scheme for read-only commit browsing through the generic VFS.

The icon browser virtualizes rows above 200 files, including groups, keyboard navigation and inline rename. Large columns use virtual rows; list view remains virtualized. Selection membership uses sets. Native application-icon requests are limited to two concurrent requests and bounded caches evict one entry at a time. Refreshes retain the active directory contents; search result updates use React transitions. Settings load in a separate chunk, and theme listeners update only when the theme changes.

The Playwright suite includes a 10,000-file fixture (`?demo&count=10000`) that checks bounded DOM node counts, end-of-directory selection and inline rename, alongside Git initialization, staging and commit flows. This is a structural responsiveness check, not a claim of fixed latency on every disk or network connection.

History browsing uses Core-generated `fiex://mount/<id>/` views owned by an explicit VFS contribution through the generic `vfs.*` router; Core no longer contains Git tree/blob code. Branches, tags and commit IDs resolve to immutable commit hashes; listing and text preview are read directly, and only the selected media file is cached for preview. The live worktree and HEAD remain untouched. Historical entries are read-only and filesystem watchers are disabled for them.

Windows, Linux and macOS CI record folder moves, bookmark creation, moves between Fiex windows, copy-only drags into the OS file manager and Chromium uploads. The tests verify destination bytes and source removal/preservation, then exercise WebDAV/SMB previews and populated NTFS, ext4, btrfs, APFS and HFS+ images. Writable NTFS/ext4 images also exercise UI folder creation; read-only images disable writes. These are image-backed driver tests, not physical USB hardware tests.

`transfers.rs` plans native file operations and collects conflicts before mutation. Replace/skip/compare choices match Windows Explorer; selecting both versions retains a numbered filename. Replacement backups live until undo or session end. `virtual_transfers.rs` applies the same conflict contract to raw filesystem drivers. `drag_session.rs` recognizes active drags owned by this process so another Fiex window moves files while external destinations receive copy-only payloads. Each window has its own directory watcher.
