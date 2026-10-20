<p align="center"><img src="assets/icon.svg" width="88" alt="Fiex"></p>
<h1 align="center">Fiex</h1>
<p align="center">A calm, fast file explorer for macOS, Windows and Linux.</p>
<p align="center"><a href="https://github.com/kevinhuang001/fiex/releases/latest">Download</a> · <a href="README.zh-CN.md">简体中文</a></p>

Fiex gives your files room to breathe: a translucent sidebar, compact controls, native icons, and four ways to explore.

### Find your way

List, icons, columns and gallery views. Group by kind, extension, modification date or size. Sort, open tabs, pin favorite folders, browse mounted volumes and jump directly to a path. Folder changes appear automatically.

### Find it quickly

Built-in parallel filename search uses the `ignore` engine from ripgrep. Results stream as they are found. New queries cancel old searches. Hidden files and `.gitignore` filtering are configurable. The list virtualizes its rows, including search results. Searches are scoped to the current folder and its descendants; there is no persistent whole-disk index.

### Work with your files

Create files and folders, rename, copy, cut, paste, duplicate, move to Trash, and undo file operations within the session. Drag between Fiex folders to copy; hold Shift to move. Drop files from another app to copy them into the current folder. Existing names are preserved by keeping both files.

Press **Space** for Quick Look. Preview images, video, audio, UTF-8 text/code and PDF through the built-in PDF.js renderer. Applications show their native logos. **Open in Terminal** detects installed terminals and lets you choose a default.

ZIP creation and ZIP, 7z, TAR, GZ, BZ2 and XZ extraction are built in; no command-line tools need to be installed. Extraction is staged, rejects links and unsafe paths, and has entry/size limits.

### Keyboard

| Action                | Shortcut                  |
| --------------------- | ------------------------- |
| Quick Look            | Space                     |
| Search / Go to folder | Cmd/Ctrl+F / Cmd/Ctrl+L   |
| Copy / Cut / Paste    | Cmd/Ctrl+C / X / V        |
| Undo / Duplicate      | Cmd/Ctrl+Z / D            |
| New / Close tab       | Cmd/Ctrl+T / W            |
| New folder            | Cmd/Ctrl+Shift+N          |
| Rename                | F2 (or Return on macOS)   |
| Trash                 | Delete (or Cmd+Backspace) |
| Back / Forward / Up   | Alt+Left / Right / Up     |
| Refresh               | F5                        |

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

GitHub Actions runs UI workflows, model tests, Rust tests, formatting, Clippy and native builds for macOS arm64/x64, Windows x64 and Linux x64. Releases publish only after every check and build passes.

### First-release scope

Fiex is not yet a complete Finder replacement. RAR/encrypted archives, Finder tags, share mounting, permissions editing, Quick Look plugins and whole-disk indexing are not included. Cross-volume moves currently ask you to copy first; verify the copy before trashing the original. Clipboard and undo are internal to the session. Trash is opened in the system file manager for restore. Linux blur requires compositor support. Media preview depends on WebView codecs; other types open with the default application. Builds are not signed with commercial code-signing certificates.

MIT. See [third-party notices](THIRD-PARTY.md).
