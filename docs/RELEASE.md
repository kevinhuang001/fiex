# Fiex 0.4.5

- Nested context submenus automatically flip upward when they would cross the window bottom, so long Archive format lists stay visible.
- Core owns the plugin panel icon, title and close button. Archive/Checksum/Git panels no longer render duplicate headers.
- Archive is one context-menu entry with all compression formats nested, extraction only for archive files, and compression only for non-archive selections.

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
