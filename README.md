<p align="center"><img src="assets/icon.png" width="88" alt="Fiex"></p>
<h1 align="center">Fiex</h1>
<p align="center">A calm, fast file explorer for macOS, Windows and Linux.</p>
<p align="center"><a href="https://github.com/kevinhuang001/fiex/releases/latest">Download</a> · <a href="README.zh-CN.md">简体中文</a></p>

Fiex gives your files room to breathe: a translucent sidebar, compact controls, colored file-type icons, and four ways to explore.

### Find your way

The address bar opens an inline path field with the full path selected for copying. Explore with list, icon, column and gallery views. Removable USB drives are detected automatically and shown under Devices with a USB icon, filesystem label, available/total capacity, a per-drive colored usage bar and an eject button. Volumes the OS has not mounted appear dimmed; clicking one uses the system authorization prompt. NTFS and ext2/3/4 volumes are read and written in-process through bundled pure-Rust drivers under `~/.fiex/usbfs`, without FUSE or an OS mount. APFS, HFS+ and btrfs are also opened in-process, read-only. Encrypted APFS volumes are not supported by the reader. The icon view uses a Finder-style fixed grid with two-line labels and snap-to-grid cells. Group by kind, extension, modification date or size. Sort, open tabs, pin favorite folders, browse mounted volumes and jump directly to a path. Folder changes appear automatically.

macOS Finder tags appear as colored dots in list, icon and column views. Tags can be applied from the context menu, using the standard colors or custom names. File icons are colored by type and language using the MIT-licensed [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme), with dedicated icons for 1,300+ extensions and 2,100+ filenames. Application bundles still use their native system logos.

### Find it quickly

Search is a live filesystem walk backed by ripgrep's [`ignore`](https://crates.io/crates/ignore) walker. There is no persistent index and no background reindexing: every query sees the current directory state, respects the **display-ignore** rules from Settings, and stops at 500 results. Application bundles (`.app`, `.framework` and friends) are returned as results but never traversed. Advanced indexed or semantic search is supplied by optional plugins through `extensions.search`.

### Work with your files

Create files and folders, rename, copy, cut, paste, duplicate, move to Trash, and undo file operations within the session. The Trash keyboard shortcut deletes immediately; the context-menu action still asks for confirmation. Renaming happens inline on the item, Finder-style; a new folder is created immediately with a unique name and its name is selected for editing. Drag files and folders between Fiex folders to move them; hold Option/Alt (or Ctrl) to copy. Drop a folder on the Favorites section to bookmark it. Drag files out to Finder, Explorer or the desktop, and drop files from another app into Fiex to copy them into the current folder. Copy and move run immediately unless names conflict. Conflicts offer replace, skip and per-file comparison; selecting both versions keeps a numbered copy. Native replacements can be undone during the session.

When Git is installed, the status bar shows the current branch and number of changes, changed filenames are tinted by state, and the toolbar Git button opens Source Control with repository initialization, file staging/unstaging, and commits with a message and author identity. The branch + commit button opens a searchable list of commits from every branch, sorted newest first. Each commit opens as a read-only snapshot without changing HEAD or uncommitted files; historical filenames use the normal theme color. Source Control shows changes by default. A separate commit dialog prefills the effective project/global Git identity and saves the chosen name and email to the project Git config. Missing identity must be filled before committing. Drags between Fiex windows move by default. External drags always copy, including browser upload targets, and preserve the source file; in-app folder drops still move unless a copy modifier is held. Favorites use the same file-type icons as the main view. Disabled commit buttons display the reason in the button. Remote repositories support push and fast-forward-only pull, with project-level remote configuration. Toolbar icons use Phosphor Icons. Only staged files are committed; Git errors remain visible and the commit draft is retained on failure. Show Info includes the file's Git state. Open the selected file or folder in VS Code from the toolbar or the context menu; Tabby is detected alongside the other terminals. The sidebar has one **Applications** entry on every platform. macOS browses `/Applications` directly; Windows and Linux have no equivalent folder, so the entry only opens the platform app list. The **Trash** opens inside Fiex on macOS and Linux, so deleted files can be browsed and moved back without launching Finder. macOS protects `~/.Trash`; if access is denied, Fiex offers to open Full Disk Access settings or Finder.

SSH terminals are built in (see [docs/ssh.md](docs/ssh.md)); Git, WebDAV, SMB, archive, checksum and the Windows Everything search are optional plugins. Install them from the plugin button in the toolbar (the gear at the top-right of its menu); nothing is preinstalled. Each plugin runs as a separate process, requires explicit permission approval, and is constrained by the Arapuca OS sandbox on Linux, macOS and Windows.

SSH connections open a full-height xterm.js terminal backed by the system OpenSSH client in a real PTY (passwords, keys, agent, ProxyJump and `~/.ssh/config` are handled by OpenSSH). WebDAV (HTTP/HTTPS) and SMB connections appear under **Devices** and live in `~/.fiex/drives`. Directory listings are rendered virtually from plugin metadata; remote files download on first open and stay available offline. **Download for offline use** fetches a whole folder. Connections are read-only; closing one from Devices leaves its cache in place.

Press **Space** for Quick Look. Preview images, video, audio, UTF-8 text/code and PDF through the built-in PDF.js renderer. Applications show their native logos. **Open in Terminal** detects installed terminals and lets you choose a default.

The Archive plugin supports ZIP, RAR, 7Z, TAR, TAR.GZ, TAR.BZ2 and TAR.XZ compression, and extraction of ZIP, RAR, 7z, TAR, GZ, BZ2 and XZ. RAR uses the pure-Rust `rars` codec; no command-line tools are needed. Extraction is staged, rejects links and unsafe paths, and has entry/size limits. The Checksum plugin renders its React panel in a sandboxed iframe and computes SHA-256, SHA-512, SHA-1, MD5 and CRC32. Settings has editable display and copy/move ignore lists. Display presets hide other-platform metadata such as `.DS_Store`, `Thumbs.db` and `desktop.ini`; the copy/move preset also skips build output for common toolchains.

### Keyboard

Settings use a System Settings-style window with a searchable category sidebar and grouped cards. The Sidebar page reorders Common Locations, Favorites, Tags and Devices, and hides individual common locations. The font-size slider scales both labels and icon-view icons. Finder-style tags are listed in the sidebar; selecting one filters the current folder, and selecting it again clears the filter. Shortcuts are editable under **Settings → Shortcuts**. Fiex ships with macOS, Windows and Linux presets; picking a style rewrites every binding, and any individual shortcut can be re-recorded. Defaults for the macOS style:

| Action                | Shortcut                   |
| --------------------- | -------------------------- |
| Quick Look            | Space                      |
| Search / Go to folder | Cmd+F / Cmd+Shift+G        |
| Copy / Cut / Paste    | Cmd+C / X / V              |
| Undo / Duplicate      | Cmd+Z / D                  |
| New / Close tab       | Cmd+T / W                  |
| New folder / file     | Cmd+Shift+N / Cmd+Option+N |
| Rename / Open         | Return or F2 / Cmd+Down    |
| Trash                 | Cmd+Backspace              |
| Back / Forward / Up   | Cmd+[ / ] / Up             |
| Refresh               | Cmd+R or F5                |
| Settings              | Cmd+,                      |

Windows and Linux presets use Ctrl/Alt with Explorer/Nautilus-style Rename (F2), Open (Enter), Trash (Delete) and Back/Forward (Alt+Left/Right).

### Development

Node 22+, Rust stable (1.90+) and [Tauri platform dependencies](https://v2.tauri.app/start/prerequisites/) are required.

```sh
npm ci
npm run icons
npm run desktop:dev
```

```sh
npm test
npm run e2e
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --lib
npm run desktop:build
```

`npm run dev` with `/?demo` runs an isolated UI fixture for browser testing. The desktop app always uses the real Rust filesystem backend.

Native integration videos are recorded in GitHub Actions on Windows, Linux and macOS Intel. The separate `ci-driver` binary locates real UI controls; OS input drives actual drag-and-drop. Tests verify folder moves, moves between Fiex windows, copies to the OS file manager, Chromium uploads, Git push/pull, live WebDAV/SMB previews and five populated disk images (NTFS, ext4, btrfs, APFS, HFS+). Videos, screenshots, application logs and byte-level results are uploaded as `native-integrations-*` artifacts, including on failure. Release installers are built and uploaded before the test-only feature is compiled. No recordings are made locally.

Modules: `src/components/browser` (views), `src/hooks` (navigation/preferences), `src/lib` (models/IPC), `src-tauri/src/core` (files, operations, search, tags, terminals), `plugins/host.rs` (process lifecycle and IPC), `plugins/installer.rs` (GitHub marketplace and install), `plugins/permissions.rs`, `plugins/sandbox.rs`, `plugins/vfs.rs` (generic plugin VFS), `remote.rs` (remote metadata and controlled streaming), `desktop.rs` (Tauri commands). Git, WebDAV, SMB, archive, checksum and Everything search implementations live in the separate `fiex-official-plugins` repository.

Archive and network codecs ship inside the optional plugin processes, not the Core binary. Core still uses rustls for the GitHub plugin installer. The release pipeline runs `npm run audit:native` to reject any app binary that points at `/opt/homebrew`, `/usr/local` or a build-machine path. Remote connection passwords are stored in the system credential store; `~/.fiex/connections.json` never serializes them.

GitHub Actions runs UI workflows, model tests, Rust tests, formatting, Clippy, the native dependency audit and native builds for macOS arm64/x64, Windows x64 and Linux x64. Releases publish only after every check and build passes.

### First-release scope

Fiex is not yet a complete Finder replacement. OS-level network mounts, Quick Look plugins and whole-disk indexing are not included. RAR extraction and creation are provided by the Archive plugin. Plugin permissions cover file read/write and network domains; OS-level sandboxing uses Arapuca on Linux, macOS and Windows. WebDAV/SMB are read-only virtual mirrors under `~/.fiex`, not operating-system mounts. Bundled raw filesystem drivers support NTFS/ext4 read-write and APFS/HFS+/btrfs read-only, without FUSE or an OS mount. Cross-volume moves copy successfully before removing the source. Clipboard and undo are internal to the session. Trash opens in Fiex where the OS allows it, with Finder and Full Disk Access shortcuts when macOS blocks access. Linux blur requires compositor support. Media preview depends on WebView codecs; other types open with the default application. Builds are not signed with commercial code-signing certificates.

GPL-3.0-or-later. See [third-party notices](THIRD-PARTY.md).
