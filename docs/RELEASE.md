Fiex is a native file explorer built with Tauri, React and shadcn/ui.

- Frosted sidebar inspired by Pantodo, with light and dark appearances.
- System Settings-style preferences with a searchable sidebar and grouped cards.
- macOS Finder tags with color dots in every view, preset colors and custom tag names.
- Move files and folders by dragging; hold Option/Alt or Ctrl to copy. Drop folders on Favorites to bookmark them.
- Open the selected file or folder in VS Code from the toolbar or context menu.
- Tabby is detected alongside Terminal, iTerm2, Warp, Alacritty, kitty, Ghostty and WezTerm.
- The whole address bar opens an inline path field with all text selected; no separate copy button is needed.
- Link targets are shown for symlinks and Windows shortcuts in Show Info.
- Adjustable font size scales both filenames and icon-view icons.
- A unified Applications entry on macOS, Windows and Linux: macOS browses /Applications, while Windows and Linux open the platform app list.
- The Trash opens inside Fiex on macOS and Linux; Windows keeps the system Recycle Bin.
- Crisp native application logos rendered at the display's pixel ratio, including large icon view.
- One deduplicated Macintosh HD entry on macOS instead of separate root and /System/Volumes/Data volumes.
- List, icons, columns and gallery views; grouping and sorting.
- Finder-style icon grid with fixed cells, two-line labels and adjustable icon size.
- Colored file-type icons from the MIT-licensed Material Icon Theme, covering 1,300+ extensions and 2,100+ filenames, plus native application logos.
- Inline rename directly on the file item. New folders are created immediately with a unique name and are ready to rename.
- Editable keyboard shortcuts with macOS, Windows and Linux presets, per-action recording and conflict handling.
- Parallel, cancellable filename search powered by ripgrep's `ignore` library; entering a folder focuses search, and app bundles are never traversed.
- Tabs, favorites, mounted volumes, breadcrumbs and directory watching.
- Copy, cut, paste, duplicate, create, Trash and session undo.
- Space to preview images, audio, video, UTF-8 text/code and PDF with built-in PDF.js rendering.
- Built-in ZIP creation and ZIP/7z/TAR/GZ/BZ2/XZ extraction.
- Detected terminal picker and a configurable default terminal.
- Self-contained native codecs: liblzma is linked statically, and release builds are audited for Homebrew/build-machine library paths.
- Fixed startup on macOS causing `Library not loaded: /opt/homebrew/opt/xz/lib/liblzma.5.dylib`.
- Group headings are bold and use the primary text color.
- The app icon is the cropped Glass Blue Folder artwork.

Downloads: macOS Apple Silicon / Intel DMG, Windows EXE / MSI, Linux DEB / RPM / AppImage.

The app is unsigned by an Apple Developer / Microsoft signing certificate. macOS uses ad-hoc signing. Linux background translucency depends on the compositor. RAR, encrypted archives and network-share mounting are not included in this release. Clipboard and undo are within the Fiex session; Trash opens in Fiex when permitted, otherwise Fiex offers Finder or Full Disk Access settings. Cross-volume moves report an error and direct you to copy first. Archive extraction limits: 100,000 entries / 20 GiB; links and unsafe paths are rejected.
