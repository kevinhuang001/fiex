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
