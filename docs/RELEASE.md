# Fiex 0.5.0

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

- Git links libgit2 through `git2`; no local Git executable and no `host.exec`.
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
