Fiex is a native file explorer built with Tauri, React and shadcn/ui.

- Frosted sidebar inspired by Pantodo, with light and dark appearances.
- List, icons, columns and gallery views; grouping and sorting.
- Finder-style icon grid with fixed cells, two-line labels and adjustable icon size.
- Colored file-type icons from the MIT-licensed Material Icon Theme, covering 1,300+ extensions and 2,100+ filenames, plus native application logos.
- Inline rename directly on the file item. New folders are created immediately with a unique name and are ready to rename.
- Editable keyboard shortcuts with macOS, Windows and Linux presets, per-action recording and conflict handling.
- Parallel, cancellable filename search powered by ripgrep's `ignore` library.
- Tabs, favorites, mounted volumes, breadcrumbs and directory watching.
- Copy, cut, paste, duplicate, create, Trash and session undo.
- Space to preview images, audio, video, UTF-8 text/code and PDF with built-in PDF.js rendering.
- Built-in ZIP creation and ZIP/7z/TAR/GZ/BZ2/XZ extraction.
- Detected terminal picker and a configurable default terminal.
- Self-contained native codecs: liblzma is linked statically, and release builds are audited for Homebrew/build-machine library paths.
- Fixed startup on macOS causing `Library not loaded: /opt/homebrew/opt/xz/lib/liblzma.5.dylib`.

Downloads: macOS Apple Silicon / Intel DMG, Windows EXE / MSI, Linux DEB / RPM / AppImage.

The app is unsigned by an Apple Developer / Microsoft signing certificate. macOS uses ad-hoc signing. Linux background translucency depends on the compositor. RAR, encrypted archives, network-share mounting and Finder tags are not included in this release. File clipboard and undo are currently within the Fiex session; restore deleted files from the system Trash. Cross-volume moves report an error and direct you to copy first. Archive extraction limits: 100,000 entries / 20 GiB; links and unsafe paths are rejected.
