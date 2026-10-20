Fiex 0.2.4 release notes

Fiex is a native file explorer built with Tauri, React and shadcn/ui.

- Frosted sidebar inspired by Pantodo, with light and dark appearances.
- System Settings-style preferences with a searchable sidebar and grouped cards.
- Sidebar settings for section order and hiding individual common locations.
- macOS Finder tags with color dots in every view, preset colors, custom tag names, and a Finder-style sidebar tag list.
- Move files and folders by dragging; hold Option/Alt or Ctrl to copy. Drop folders on Favorites to bookmark them, and drag files out of Fiex to Finder/Explorer/the desktop.
- Open the selected file or folder in VS Code from the toolbar or context menu.
- Tabby is detected alongside Terminal, iTerm2, Warp, Alacritty, kitty, Ghostty and WezTerm.
- The whole address bar opens an inline path field with all text selected; no separate copy button is needed.
- Link targets are shown for symlinks and Windows shortcuts in Show Info.
- Adjustable font size scales both filenames and icon-view icons.
- A unified Applications entry on macOS, Windows and Linux: macOS browses /Applications, while Windows and Linux open the platform app list.
- Removable USB drives are detected with a USB icon, filesystem label, available/total capacity and per-drive colored usage bars. Unmounted NTFS and ext2/3/4 volumes request system authorization and are read/written in-process under `~/.fiex/usbfs`; APFS/HFS+/btrfs remain read-only.
- The Trash opens inside Fiex on macOS and Linux; Windows keeps the system Recycle Bin.
- Crisp native application logos rendered at the display's pixel ratio, including large icon view.
- One deduplicated Macintosh HD entry on macOS instead of separate root and /System/Volumes/Data volumes.
- List, icons, columns and gallery views; grouping and sorting.
- Finder-style icon grid with fixed cells, two-line labels and adjustable icon size.
- Colored file-type icons from the MIT-licensed Material Icon Theme, covering 1,300+ extensions and 2,100+ filenames, plus native application logos.
- Inline rename directly on the file item. New folders are created immediately with a unique name and are ready to rename.
- Editable keyboard shortcuts with macOS, Windows and Linux presets, per-action recording and conflict handling.
- Persistent Tantivy search index under `~/.fiex/index`, built in the background per folder and kept fresh with a recursive watcher; queries use display-ignore rules and never descend into app bundles.
- Tabs, favorites, mounted volumes, breadcrumbs and directory watching.
- Copy, cut, paste, duplicate, create, Trash and session undo; the Trash shortcut deletes without a confirmation dialog.
- Space to preview images, audio, video, UTF-8 text/code and PDF with built-in PDF.js rendering.
- ZIP/7Z/TAR/TAR.GZ/TAR.BZ2/TAR.XZ creation from a context-menu submenu, with editable compression ignore rules; extraction covers ZIP/7z/TAR/GZ/BZ2/XZ.
- Detected terminal picker and a configurable default terminal.
- Self-contained native codecs and network clients: liblzma is static, WebDAV uses rustls, SMB is pure Rust, and release builds are audited for Homebrew/build-machine library paths.
- Git branch, staged/unstaged/untracked status, colored filenames, and a read-only toolbar Source Control panel grouped by staged, modified and untracked changes.
- WebDAV (HTTP/HTTPS) and SMB virtual drives under `~/.fiex/drives`, with on-demand placeholders, offline cache, whole-folder offline downloads and no OS mount.
- Editable display, compression and copy/move ignore lists with presets for system files and mainstream toolchains.
- Finder-style tag list in the sidebar with click-to-filter.
- Device eject for disks and connection close for virtual drives.
- Fixed startup on macOS causing `Library not loaded: /opt/homebrew/opt/xz/lib/liblzma.5.dylib`.
- Group headings are bold and use the primary text color.
- The app icon is the cropped Glass Blue Folder artwork.

Downloads: macOS Apple Silicon / Intel DMG, Windows EXE / MSI, Linux DEB / RPM / AppImage.

The app is unsigned by an Apple Developer / Microsoft signing certificate. macOS uses ad-hoc signing. Linux background translucency depends on the compositor. RAR/encrypted archives and OS-level network mounts are not included in this release; WebDAV/SMB drives are read-only virtual mirrors under `~/.fiex`. Clipboard and undo are within the Fiex session; Trash opens in Fiex when permitted, otherwise Fiex offers Finder or Full Disk Access settings. Cross-volume moves report an error and direct you to copy first. Archive extraction limits: 100,000 entries / 20 GiB; links and unsafe paths are rejected.
