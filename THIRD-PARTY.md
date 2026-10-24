> Fiex is licensed under GPL-3.0-or-later. The dependencies below keep their own licenses; GPL-compatible terms apply when distributing binaries.

# Third-party software

Fiex incorporates these open-source projects through package managers. Exact resolved versions are recorded in package-lock.json and src-tauri/Cargo.lock; upstream licenses apply.

| Project              | Purpose                                                        | License / source                                                               |
| -------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Tauri                | Native application runtime                                     | MIT / Apache-2.0 — https://github.com/tauri-apps/tauri                         |
| React                | Interface                                                      | MIT — https://github.com/facebook/react                                        |
| shadcn/ui / Radix UI | Accessible interface primitives                                | MIT — https://github.com/shadcn-ui/ui / https://github.com/radix-ui/primitives |
| Tailwind CSS         | Styling                                                        | MIT — https://github.com/tailwindlabs/tailwindcss                              |
| Phosphor Icons       | Toolbar icons                                                  | MIT — https://github.com/phosphor-icons/react                                  |
| Lucide               | Interface icons                                                | ISC — https://github.com/lucide-icons/lucide                                   |
| Material Icon Theme  | Colored file-type and folder icons                             | MIT — https://github.com/material-extensions/vscode-material-icon-theme        |
| TanStack Virtual     | Large-list rendering                                           | MIT — https://github.com/TanStack/virtual                                      |
| ignore (ripgrep)     | Filesystem walking and search filters                          | MIT / Unlicense — https://github.com/BurntSushi/ripgrep                        |
| zip                  | Plugin package verification and extraction                     | MIT — https://github.com/zip-rs/zip2                                           |
| file_icon_provider   | Native file/application icons                                  | MIT — https://github.com/IohannRabeson/file_icon_provider                      |
| Noto Sans SC         | Chinese font fallback                                          | SIL OFL-1.1 — https://github.com/notofonts/noto-cjk                            |
| PDF.js               | PDF rendering                                                  | Apache-2.0 — https://github.com/mozilla/pdf.js                                 |
| Tantivy              | Persistent filename search index                               | MIT — https://github.com/quickwit-oss/tantivy                                  |
| reqwest              | GitHub plugin installer (rustls)                               | MIT / Apache-2.0 — https://github.com/seanmonstar/reqwest                      |
| globset              | Ignore-rule matching                                           | Unlicense / MIT — https://github.com/BurntSushi/ripgrep                        |
| tauri-plugin-drag    | Native drag-out to other applications                          | Apache-2.0 / MIT — https://github.com/crabnebula-dev/tauri-plugin-drag         |
| apfs / hfsplus       | Read-only APFS and HFS+ parsers                                | MIT — https://github.com/Dil4rd/dpp                                            |
| notify               | Filesystem watching                                            | CC0-1.0 — https://github.com/notify-rs/notify                                  |
| trash                | System Trash                                                   | MIT — https://github.com/Byron/trash-rs                                        |
| Pantodo              | Native glass configuration and shadcn component starting point | Same author — https://github.com/kevinhuang001/pantodo                         |
| Lofty                | Audio tag and duration metadata                                | MIT — https://github.com/Serial-ATA/lofty-rs                                   |
| mp4                  | MP4/MOV container metadata                                     | MIT — https://github.com/alfg/mp4-rust                                         |
| matroska             | Matroska/WebM container metadata                               | MIT — https://github.com/rust-av/matroska                                      |

The optional Archive, Checksum, Git, WebDAV and SMB plugins live in a separate repository and have their own dependency notices. Core dependencies include serde, serde_json, dirs, open, which, base64, mime_guess, tempfile, sysinfo, image, lofty, mp4, matroska, objc2, clsx, class-variance-authority and tailwind-merge. Development tooling includes Vite, TypeScript, Vitest, Playwright and Prettier.
