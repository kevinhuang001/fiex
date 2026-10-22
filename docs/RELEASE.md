# Fiex 0.3.0

## Plugins

- Optional Git, WebDAV, SMB, Archive, Checksum and Windows Everything search plugins.
- Nothing is preinstalled; install from **Settings → Plugins** and approve permissions.
- Permissions are limited to file read/write and network domains; loopback addresses are always allowed.
- Plugins run as independent processes in an Arapuca sandbox (Landlock + seccomp on Linux, Seatbelt on macOS, AppContainer on Windows).

## Plugin UI

- One row per installed plugin; hover actions for permissions and settings.
- Pin plugin actions into the toolbar, reorder them with the same animated drag used by tabs.
- Plugin dropdown shows manifest-provided icons.
- App-store style catalog reads `fiex-catalog.json` from the latest official plugin release.

## Search and preview

- Built-in search is a live ripgrep walker: no Tantivy index, no background reindexing, no `~/.fiex/index`.
- Core merges local filename search with plugin search providers.
- Everything plugin queries the existing Windows Everything index; indexing stays owned by Everything.
- Text preview highlights source code with a bundled highlight.js core and selected languages.
- Image, PDF, audio, video and text preview remain in Core.

## Performance and size

- File copies use copy-on-write reflinks when the filesystem supports them, then a parallel chunked fallback.
- Multi-file copies run on a bounded Rayon pool.
- Release builds use `strip`, fat LTO and a single codegen unit; the Linux binary is about 16 MiB.
- Tantivy and the unused `regex` dependency were removed.

## Compatibility

- Local file browsing, file operations, tabs, tags, Git source control, WebDAV/SMB and archive/checksum UI match the 0.2.7 behavior when the corresponding plugin is installed.
- Plugin protocol is 2.0 and manifest schema is 1. Both remain experimental until the Windows sandbox and task-progress work is complete.
