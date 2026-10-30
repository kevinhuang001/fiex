# Fiex 0.7.8

- Drop files onto any breadcrumb segment to copy or move them into that ancestor folder (including virtual roots and the ellipsis ancestor).
- Transfer preflight now runs only across VFS boundaries. Moves, renames and app imports inside the same VFS keep Core capability checks without invoking provider review again.
- The secondary pane address bar uses the same virtual-path normalization, decoded Chinese display and cached directory stream as the main pane.
- Saving or validating a connection no longer blocks the window while the browser OAuth page waits. A loading overlay explains that Fiex is waiting for authorization and continues automatically after the callback.

# Fiex 0.7.7

- Applications creates user folders directly under the view root (`applications://~…`). Breadcrumbs, the address bar and File Info show the real Chinese folder name instead of base64, and existing `applications://user/...` links plus mapped folders migrate automatically.
- The address bar accepts every built-in virtual root (`recent`, `applications`, `trash`, `recyclebin`) with either URL or Windows-backslash separators. Breadcrumb clicks stay on `applications://` instead of degrading to `applications:\`.
- Ejecting a Windows drive now resolves `E:\` through the Shell eject verb and leaves panes that are inside the volume before the location list refreshes; Windows USB ejection uses the same helper.
- Plugin RPC logs record the important request parameters (node id, path, name, query, mode, limits, stream offsets) and result counts. Credentials, opaque identities and payload bodies remain redacted, and duplicate payload-free SDK lifecycle lines are suppressed.
- Address-bar navigation now uses the same cached directory stream as the sidebar and history, so a previously opened path paints immediately.
- Provider settings use a compact feature tab grid and a cleaner execution-order card with enable counts and provider summaries.

# Fiex 0.7.6

- Remove the Core-owned Office PDF converter and its PowerShell/COM execution. Core no longer knows DOCX/PPTX, Office or WPS.
- The Office Preview plugin now renders Word/PowerPoint entirely in pure Rust (`office_oxide` IR + `printpdf` + `hayro`) with a bundled CJK font. It lays out only the requested first pages and returns plugin-owned PNG pages, so Core never executes Office or renders a generated PDF.
- Move official-plugin demo/i18n copy out of Core's production language catalogs.

# Fiex 0.7.5

- Load plugin previews from the asset origin instead of `srcDoc`, so the app shell CSP no longer blocks the plugin's inline bundle. Archive, Office and Data previews now render instead of timing out.
- Windows Office documents are converted to PDF through the installed Office automation server before preview; the built-in PDF viewer shows the first 20 pages. The office plugin remains the fallback when conversion is unavailable.
- XLSX/XLS previews expose every workbook sheet in the same left-hand dataset list as SQLite tables.
- Plugin panels and PluginViews receive `settings` through `context.get`; manifest settings support system-font (`font`) and color (`color`) pickers.
- SSH terminal settings add font family, font size and foreground color; the terminal background and chrome follow light/dark theme tokens.
- PluginViews receive `theme.changed` events, giving connection UIs the same live theme contract as panels and previews.
- Picker startup no longer lists an empty path; sidebar connection projection avoids keyring reads and local directory metadata loads in parallel.

# Fiex 0.7.4

- Separate downloaded connection cache from pinned offline copies, with upload indicators, configurable synchronization intervals and activity records.
- Route builtin context menus and decorations through the shared event pipeline; keep asynchronous menus inside the viewport.
- Use plugin protocol 6 for unified ref-based filesystem operations and provider-declared operation support. Plugins must use a matching protocol-6 SDK.
- Reorganize native Core into domain modules and grouped resolvers; remove compatibility forwarding modules and unused entry points.
- Fix large and partial stream writes, cleanup after failed commits, pagination cursor cycles and malformed node permission restrictions.
- Prevent disabled lazy submenu actions from executing; fix Windows Clippy and static toolbar panel CI coverage.

# Fiex 0.7.3

- Terminal settings now choose a system font (including Nerd Font families) and font size.
- Plugin host no longer holds its global lock while spawning a plugin process, so a slow Windows process start cannot block Info, hooks or other plugins.
- File Info has a 15-second loading timeout instead of waiting forever on a plugin.

# Fiex 0.7.2

- Fiex is now single-instance: launching it again focuses the existing window instead of starting a second Core and a second plugin process tree.

# Fiex 0.7.1

- Approved, enabled native plugins auto-start after the app opens instead of waiting for the first invocation.
- Closing Fiex explicitly stops every resident plugin process, so plugin children do not outlive the window.
- Plugin activation and disable still start/stop their backend immediately.

# Fiex 0.7.0

- Plugin platform: schema 2 manifests, scoped provider events, unified preview/info providers, per-operation grants and grouped activation with per-capability toggles.
- Plugin previews can declare a package-local loading page. Core keeps it visible until the preview calls `preview.ready` after its first data render, so there is no empty-shell flash.
- New GPG keychain, Email, SSH and Google Drive official plugins; Archive, Office preview and Data preview gain branded loading pages.
- Plugin panels and menus open at the final size and side, with no delay or small-to-large jump; nested menus close immediately and grow in the correct direction.
- Integrated terminal: steady cursor, draggable tabs with the same animation as workspace tabs, stable tab numbers and a tighter tab strip.
- Dialogs and plugin sheets adapt to the current window size.
- Legacy manifest and grant compatibility removed; old records require reactivation.

# Fiex 0.5.1

0.5.1 is a structure-only release: the Rust host, desktop commands, hooks, settings and SDK are split into modules, with no behavior changes.

- New `hooks` permission and `contributions.hooks`: a plugin can run before Core opens a file, in priority order. A hook may pass, consume the open, or hand off to the preview sheet. Failures are logged and never block the file.
- The archive plugin uses it: opening an archive extracts it next to the archive (conflict-safe names) instead of launching the system app; password-protected archives open the preview for the password.
- Plugin file dialogs now use the app's own compact file browser (sidebar places, favorites, connections, devices, breadcrumbs, back/forward/up, search, list/grid, sorting, grouping, new folder, context menu, centered save name, replace dialog) instead of the system picker.
- Context menus (picker and main window) measure themselves and stay inside the viewport, flipping near the edges.
- Breadcrumbs show at most the last three levels, with an ellipsis to climb one level, in both the main window and the picker.
- The plugin store takes each plugin's introduction from the release's `plugin-descriptions.json`, and only lists plugins whose latest release has a package for this platform.
- Internal cleanup: the Rust plugin host, demo data, panel host, plugin settings, file picker and both SDKs are split into focused modules instead of monolithic files.

# Fiex 0.4.11

- The preview sheet header is compact (44 px), leaving more room for the content.
- Office preview now parses DOCX/XLSX/PPTX with a real Office library and renders headings, lists and tables (plugin 0.2.10).

# Fiex 0.4.10

- Plugin previewers render inside the Quick Look sheet itself, with the same sandbox and bridge as panels; the built-in text, image, media and PDF previewers are unchanged.
- The dev server serves a plugin's unbuilt `ui/index.html` when there is no `ui/dist`, so self-contained plugin panels can be developed without a bundler.
- New official plugins (published separately): SQLite viewer and Office preview.

# Fiex 0.4.9

- Plugins run behind scoped Host services and require explicit activation consent: folder, file, network and external-command grants are requested one at a time and can be revoked.
- Panels can be declared on toolbar tools and inside context submenus; “高级压缩” opens again instead of failing with “plugin declares no such panel”.
- Plugin panels keep a cached session per window, use the size the plugin declares, anchor to the toolbar button or menu item, and only appear once their iframe has settled.
- Theme packages are declarative: no executable, no iframe, no permissions. Tokens now cover tabs, rows, menus, bars, inputs and controls, so a theme restyles the chrome without CSS while the built-in palette stays unchanged.
- Tab and pinned-tool reordering follows the pointer, and neighbouring items move aside at half overlap.
- Appearance settings right-align their controls and centre the theme picker menu.
- File decorations and plugin-provided file icons.
- Plugins can declare file previewers (`contributions.preview`) with extensions and a priority, overriding built-in previewers such as PDF.
- Plugin diagnostics: requested commands missing from PATH are marked in red, and recent plugin, panel, bridge and activation failures are listed in the plugin's details.
- Plugin settings support list fields, shown as add/remove rows.
- The toolbar plugin switcher no longer flashes the outgoing panel twice, and a reopened cached panel settles before it becomes visible.

# Fiex 0.4.8

- Plugin iframes stay invisible until their HTML and bridge are ready, eliminating the pop-then-load flash.
- Plugin empty-selection hints wait for bridge context.
- The dev plugin catalog now returns a fresh list after installation, so WebDAV appears immediately in “添加连接”.

# Fiex 0.4.7

- Plugin panels are centered on screen, use the panel size declared by the plugin, and only clamp to the viewport maximum.
- Large plugin panels no longer force a fixed Core width or height; plugins can also resize through `panel.resize`.
- Archive’s advanced-compression panel uses a compact plugin-declared size.

# Fiex 0.4.6

- Decoration badges render after the real filename so text selectors and accessibility keep the exact name.
- Git status decorations also color the filename; modified files are orange, additions green, deletions red, renames blue, untracked gray.
- Toolbar plugin panels anchor to the toolbar button; context-menu panels anchor to the menu item position.

# Fiex 0.4.5

- Nested context submenus automatically flip upward when they would cross the window bottom, so long Archive format lists stay visible.
- Core owns the plugin panel icon, title and close button. Archive/Checksum/Git panels no longer render duplicate headers.
- Archive uses separate first-level context entries: “压缩” for non-archive selections, “解压缩” for archive files, and “高级” containing “压缩到…” and advanced compression. Compression defaults to a unique name in the current folder; extraction defaults to the archive folder. Long nested menus flip upward near the viewport bottom.

# Fiex 0.4.4

- Official plugin React panels can be tested in the dev web environment at `http://127.0.0.1:1421/`; the Git and Archive tools are pinned automatically in development.
- Archive actions are context-menu only; the Archive toolbar entry was removed.
- The plugin iframe bridge retries the MessageChannel handshake when a module script starts after the iframe load event, fixing blank official panels.
- The Core iframe header owns the plugin icon, title and close button; official panels no longer render their own duplicate header.
- Demo mode is enabled automatically in development, so opening the dev URL no longer raises "请在 Fiex 桌面应用中浏览本地文件".

# Fiex 0.4.3

- Status Center progress counts every file, including files inside folders. A folder with 8 files contributes 8 to total/done, not 1.
- Per-file progress is reported while copying directories, including parallel copies.

# Fiex 0.4.2

- Plugin manifests are signed with Ed25519 publisher keys. `manifest.files` covers every package file; Core verifies the signature and digests at install and again before spawning the plugin.
- There is no special official trust. Official and third-party packages use the same first-use publisher fingerprint confirmation and key pinning.
- `Settings → Plugins` supports importing a local `.fiexplugin` package with signature/fingerprint review.
- Detects Windows/macOS system proxies and Linux proxy environment variables, shows the detected endpoint in plugin settings, and uses it for marketplace requests.
- Bottom bar no longer shows current-operation text. Important operations are recorded in Status Center.
- The plugin UI SDK now supports `tasks.create/update/finish/cancel`; Git init/commit/push/pull and Archive compression report name, detail, step and progress to Status Center.
- Removed the binary-embedded manifest handshake; the package manifest is the single source of truth.

# Fiex 0.4.1

- Fix GitHub release listing on Windows/macOS by enabling reqwest's system-proxy support. The marketplace now follows the OS proxy instead of failing with "error sending request".
- Hide every PowerShell helper window used for removable-volume polling, Recycle Bin clearing, device ejection and elevation.
- Add **Settings → Plugins → Import plugin** for a local `.fiexplugin` package, with the same manifest validation and permission preview as the store.
- Plugins now embed their manifest at build time and return it during `plugin.initialize`; Core verifies it matches the installed package before serving requests. Old protocol-2.0 plugins without the handshake remain loadable.
- Keep the 0.4.0 decoupling changes and plugin versions.

# Fiex 0.4.0

## Decoupled plugins

- Core contains no Git, archive or checksum commands, dialogs, status tinting or official plugin IDs. Those features are delivered entirely by installed plugins.
- Git, Archive and Checksum ship React/Vite panels built to single-file HTML and loaded in the sandboxed iframe host. Core owns only the generic panel host, connection forms, permissions and VFS routing.
- Connections are generic `{id,name,kind,pluginId,settings}` records. Core no longer has WebDAV/SMB-specific fields, derives plugin IDs, or auto-generates `org.fiex.*` IDs.
- Repository connection secrets are stored in the OS keyring and removed from `connections.json`; providers receive them only through the explicit `connection.validate` invocation.

## Git

- Git links libgit2 through `git2`; no local Git executable and no legacy `host.exec` entry.
- The plugin owns status, refs, staging, commits, identity, history, remote save/push/fast-forward pull and read-only commit VFS browsing.
- Divergent pulls stop before modifying the worktree.

## Archive and checksum

- Archive supports RAR extraction and creation with the pure-Rust `rars` codec, alongside ZIP, 7Z, TAR, GZ, BZ2 and XZ.
- RAR extraction keeps the total-size, entry-count, path-traversal and cancellation guards.
- Checksum and archive advanced options are React plugin panels.

## Search and preview

- Built-in search is a live ripgrep walker: no Tantivy index, no background reindexing, no `~/.fiex/index`.
- Core merges local filename search with plugin search providers and renders all result rows and icons.
- Text preview highlights source code with a bundled highlight.js core.

## Performance and size

- File copies use copy-on-write reflinks when supported, then a parallel chunked fallback on a bounded Rayon pool.
- Release builds use `strip`, fat LTO and one codegen unit.
