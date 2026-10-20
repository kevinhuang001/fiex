<p align="center"><img src="assets/icon.png" width="88" alt="Fiex"></p>
<h1 align="center">Fiex</h1>
<p align="center">A calm, fast file explorer for macOS, Windows and Linux.</p>
<p align="center"><a href="https://github.com/kevinhuang001/fiex/releases/latest">Download</a> · <a href="README.zh-CN.md">简体中文</a></p>

Fiex gives your files room to breathe: a translucent sidebar, compact controls, colored file-type icons, and four ways to explore.

### Find your way

The address bar opens an inline path field with the full path selected for copying. Explore with list, icon, column and gallery views. The icon view uses a Finder-style fixed grid with two-line labels and snap-to-grid cells. Group by kind, extension, modification date or size. Sort, open tabs, pin favorite folders, browse mounted volumes and jump directly to a path. Folder changes appear automatically.

macOS Finder tags appear as colored dots in list, icon and column views. Tags can be applied from the context menu, using the standard colors or custom names. File icons are colored by type and language using the MIT-licensed [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme), with dedicated icons for 1,300+ extensions and 2,100+ filenames. Application bundles still use their native system logos.

### Find it quickly

Search is backed by a persistent [Tantivy](https://github.com/quickwit-oss/tantivy) index under `~/.fiex/index`. The current folder is indexed in the background as you browse, and changes are picked up by a recursive watcher; queries then read the index instead of walking the filesystem. Search uses the **display-ignore** rules from Settings, so hidden system files stay out of results. Entering a folder focuses the search field. Application bundles (`.app`, `.framework` and friends) are returned as results but never traversed. Searches are scoped to the current folder and its descendants; the index is per folder, not a whole-disk index.

### Work with your files

Create files and folders, rename, copy, cut, paste, duplicate, move to Trash, and undo file operations within the session. Renaming happens inline on the item, Finder-style; a new folder is created immediately with a unique name and its name is selected for editing. Drag between Fiex folders to move; hold Option/Alt (or Ctrl) to copy. Drop a folder on the Favorites section to bookmark it. Drop files from another app to copy them into the current folder. Existing names are preserved by keeping both files.

When Git is installed, the status bar shows the current branch and number of changes, changed filenames are tinted by state, and the toolbar Git button opens a read-only Source Control panel with staged, modified and untracked groups. Show Info includes the file's Git state. Open the selected file or folder in VS Code from the toolbar or the context menu; Tabby is detected alongside the other terminals. The sidebar has one **Applications** entry on every platform. macOS browses `/Applications` directly; Windows and Linux have no equivalent folder, so the entry only opens the platform app list. The **Trash** opens inside Fiex on macOS and Linux, so deleted files can be browsed and moved back without launching Finder. macOS protects `~/.Trash`; if access is denied, Fiex offers to open Full Disk Access settings or Finder.

WebDAV (HTTP/HTTPS) and SMB connections appear under **Devices** and live in `~/.fiex/drives`. Adding a connection creates a Google Drive-style virtual folder: directory listings are synchronized into it and remote files appear as placeholders, downloaded on first open. Downloaded files stay available offline, and **Download for offline use** fetches a whole folder. Connections are read-only; closing one from Devices leaves its cache in place. Drives have an eject button, and virtual connections close with the same control.

Press **Space** for Quick Look. Preview images, video, audio, UTF-8 text/code and PDF through the built-in PDF.js renderer. Applications show their native logos. **Open in Terminal** detects installed terminals and lets you choose a default.

Compression supports ZIP, 7Z, TAR, TAR.GZ, TAR.BZ2 and TAR.XZ from a context-menu submenu. Extraction covers ZIP, 7z, TAR, GZ, BZ2 and XZ. Everything is built in; no command-line tools are needed. Extraction is staged, rejects links and unsafe paths, and has entry/size limits. Settings has three editable ignore lists: display, compression and copy/move. Display presets hide other-platform metadata such as `.DS_Store`, `Thumbs.db` and `desktop.ini`; compression and copy/move presets also skip build output for JavaScript, Rust, Python, Java, C/C++, Dart, iOS, Go and other common toolchains (`node_modules`, `target`, `__pycache__`, `.gradle`, `Pods`, `vendor` and more).

### Keyboard

Settings use a System Settings-style window with a searchable category sidebar and grouped cards. The font-size slider scales both labels and icon-view icons. Finder-style tags are listed in the sidebar; selecting one filters the current folder, and selecting it again clears the filter. Shortcuts are editable under **Settings → Shortcuts**. Fiex ships with macOS, Windows and Linux presets; picking a style rewrites every binding, and any individual shortcut can be re-recorded. Defaults for the macOS style:

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

Modules: `src/components/browser` (views), `src/hooks` (navigation/preferences), `src/lib` (models/IPC), `src-tauri/src/files.rs` (listing/metadata), `operations.rs` (mutations/undo), `search.rs` (Tantivy index/search), `archives.rs` (archives), `remote.rs` (WebDAV/SMB virtual drives), `ignores.rs` (ignore matching), `git.rs` (Git status), `terminals.rs` (OS integration), `desktop.rs` (Tauri commands).

Native codecs and network clients are bundled: liblzma is compiled statically, WebDAV uses rustls instead of system OpenSSL, and SMB is implemented by the pure-Rust `smb` crate. The release pipeline runs `npm run audit:native` to reject any app binary that still points at `/opt/homebrew`, `/usr/local` or a build-machine path. Credentials for remote connections are stored in `~/.fiex/connections.json` with owner-only permissions.

GitHub Actions runs UI workflows, model tests, Rust tests, formatting, Clippy, the native dependency audit and native builds for macOS arm64/x64, Windows x64 and Linux x64. Releases publish only after every check and build passes.

### First-release scope

Fiex is not yet a complete Finder replacement. RAR/encrypted archives, OS-level network mounts, permissions editing, Quick Look plugins and whole-disk indexing are not included. WebDAV/SMB are read-only virtual mirrors under `~/.fiex`, not operating-system mounts. Cross-volume moves currently ask you to copy first; verify the copy before trashing the original. Clipboard and undo are internal to the session. Trash opens in Fiex where the OS allows it, with Finder and Full Disk Access shortcuts when macOS blocks access. Linux blur requires compositor support. Media preview depends on WebView codecs; other types open with the default application. Builds are not signed with commercial code-signing certificates.

MIT. See [third-party notices](THIRD-PARTY.md).
