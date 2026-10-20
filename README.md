<p align="center"><img src="assets/icon.svg" width="88" alt="Fiex"></p>
<h1 align="center">Fiex</h1>
<p align="center">A calm, fast file explorer for macOS, Windows and Linux.</p>
<p align="center"><a href="https://github.com/kevinhuang001/fiex/releases/latest">Download</a> · <a href="README.zh-CN.md">简体中文</a></p>

Fiex gives your files room to breathe: a translucent sidebar, compact controls, colored file-type icons, and four ways to explore.

### Find your way

The path bar edits the current location inline and copies it with one click. Explore with list, icon, column and gallery views. The icon view uses a Finder-style fixed grid with two-line labels and snap-to-grid cells. Group by kind, extension, modification date or size. Sort, open tabs, pin favorite folders, browse mounted volumes and jump directly to a path. Folder changes appear automatically.

File icons are colored by type and language using the MIT-licensed [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme), with dedicated icons for 1,300+ extensions and 2,100+ filenames. Application bundles still use their native system logos.

### Find it quickly

Built-in parallel filename search uses the `ignore` engine from ripgrep. Results stream as they are found. New queries cancel old searches. Entering a folder focuses the search field, so typing immediately searches that location. Hidden files and `.gitignore` filtering are configurable. Application bundles (`.app`, `.framework` and friends) are returned as results but never traversed, so searching `/Applications` finds the app instead of files inside it. The list virtualizes its rows, including search results. Searches are scoped to the current folder and its descendants; there is no persistent whole-disk index.

### Work with your files

Create files and folders, rename, copy, cut, paste, duplicate, move to Trash, and undo file operations within the session. Renaming happens inline on the item, Finder-style; a new folder is created immediately with a unique name and its name is selected for editing. Drag between Fiex folders to copy; hold Shift to move. Drop files from another app to copy them into the current folder. Existing names are preserved by keeping both files.

The sidebar has one **Applications** entry on every platform. macOS browses `/Applications` directly; Windows and Linux have no equivalent folder, so the entry only opens the platform app list. The **Trash** opens inside Fiex on macOS and Linux, so deleted files can be browsed and moved back without launching Finder.

Press **Space** for Quick Look. Preview images, video, audio, UTF-8 text/code and PDF through the built-in PDF.js renderer. Applications show their native logos. **Open in Terminal** detects installed terminals and lets you choose a default.

ZIP creation and ZIP, 7z, TAR, GZ, BZ2 and XZ extraction are built in; no command-line tools need to be installed. Extraction is staged, rejects links and unsafe paths, and has entry/size limits.

### Keyboard

Settings use a System Settings-style window with a searchable category sidebar and grouped cards. Shortcuts are editable under **Settings → Shortcuts**. Fiex ships with macOS, Windows and Linux presets; picking a style rewrites every binding, and any individual shortcut can be re-recorded. Defaults for the macOS style:

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

Modules: `src/components/browser` (views), `src/hooks` (navigation/preferences), `src/lib` (models/IPC), `src-tauri/src/files.rs` (listing/metadata), `operations.rs` (mutations/undo), `search.rs` (parallel search), `archives.rs` (archives), `terminals.rs` (OS integration), `desktop.rs` (Tauri commands).

Native compression codecs are bundled: liblzma is compiled statically instead of linking Homebrew's `xz`, and the release pipeline runs `npm run audit:native` to reject any app binary that still points at `/opt/homebrew`, `/usr/local` or a build-machine path.

GitHub Actions runs UI workflows, model tests, Rust tests, formatting, Clippy, the native dependency audit and native builds for macOS arm64/x64, Windows x64 and Linux x64. Releases publish only after every check and build passes.

### First-release scope

Fiex is not yet a complete Finder replacement. RAR/encrypted archives, Finder tags, share mounting, permissions editing, Quick Look plugins and whole-disk indexing are not included. Cross-volume moves currently ask you to copy first; verify the copy before trashing the original. Clipboard and undo are internal to the session. Trash is opened in the system file manager for restore. Linux blur requires compositor support. Media preview depends on WebView codecs; other types open with the default application. Builds are not signed with commercial code-signing certificates.

MIT. See [third-party notices](THIRD-PARTY.md).
