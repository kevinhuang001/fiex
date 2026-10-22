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

## Search

- Core merges local filename search with plugin search providers.
- Everything plugin queries the existing Windows Everything index; indexing stays owned by Everything.

## Compatibility

- Local file browsing, file operations, tabs, tags, Git source control, WebDAV/SMB and archive/checksum UI match the 0.2.7 behavior when the corresponding plugin is installed.
- Plugin protocol is 2.0 and manifest schema is 1. Both remain experimental until the Windows sandbox and task-progress work is complete.
