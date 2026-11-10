---
title: 第三方软件与许可
---

# 第三方软件与许可

Fiex 使用 GPL-3.0-or-later 许可证。依赖组件保留各自许可证；具体解析版本以 package-lock.json 和 src-tauri/Cargo.lock 为准。分发安装包时请同时遵守 Fiex 与各依赖的许可证条件。

## 主要第三方组件

| Project                      | Purpose                                                        | License / source                                                               |
| ---------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Tauri                        | Native application runtime                                     | MIT / Apache-2.0 — https://github.com/tauri-apps/tauri                         |
| React                        | Interface                                                      | MIT — https://github.com/facebook/react                                        |
| shadcn/ui / Radix UI         | Accessible interface primitives                                | MIT — https://github.com/shadcn-ui/ui / https://github.com/radix-ui/primitives |
| Tailwind CSS                 | Styling                                                        | MIT — https://github.com/tailwindlabs/tailwindcss                              |
| Phosphor Icons               | Toolbar icons                                                  | MIT — https://github.com/phosphor-icons/react                                  |
| Lucide                       | Interface icons                                                | ISC — https://github.com/lucide-icons/lucide                                   |
| Material Icon Theme          | Colored file-type and folder icons                             | MIT — https://github.com/material-extensions/vscode-material-icon-theme        |
| TanStack Virtual             | Large-list rendering                                           | MIT — https://github.com/TanStack/virtual                                      |
| ignore (ripgrep)             | Filesystem walking and search filters                          | MIT / Unlicense — https://github.com/BurntSushi/ripgrep                        |
| zip                          | Plugin package verification and extraction                     | MIT — https://github.com/zip-rs/zip2                                           |
| file_icon_provider           | Native file/application icons                                  | MIT — https://github.com/IohannRabeson/file_icon_provider                      |
| Noto Sans SC                 | Chinese font fallback                                          | SIL OFL-1.1 — https://github.com/notofonts/noto-cjk                            |
| JetBrainsMono Nerd Font Mono | Bundled terminal font and patched terminal glyphs              | SIL OFL-1.1 — https://github.com/ryanoasis/nerd-fonts                          |
| PDF.js                       | PDF rendering                                                  | Apache-2.0 — https://github.com/mozilla/pdf.js                                 |
| Tantivy                      | Persistent filename search index                               | MIT — https://github.com/quickwit-oss/tantivy                                  |
| reqwest                      | GitHub plugin installer (rustls)                               | MIT / Apache-2.0 — https://github.com/seanmonstar/reqwest                      |
| globset                      | Ignore-rule matching                                           | Unlicense / MIT — https://github.com/BurntSushi/ripgrep                        |
| tauri-plugin-drag            | Native drag-out to other applications                          | Apache-2.0 / MIT — https://github.com/crabnebula-dev/tauri-plugin-drag         |
| apfs / hfsplus               | Read-only APFS and HFS+ parsers                                | MIT — https://github.com/Dil4rd/dpp                                            |
| notify                       | Filesystem watching                                            | CC0-1.0 — https://github.com/notify-rs/notify                                  |
| trash                        | System Trash                                                   | MIT — https://github.com/Byron/trash-rs                                        |
| Pantodo                      | Native glass configuration and shadcn component starting point | Same author — https://github.com/kevinhuang001/pantodo                         |
| Lofty                        | Audio tag and duration metadata                                | MIT — https://github.com/Serial-ATA/lofty-rs                                   |
| mp4                          | MP4/MOV container metadata                                     | MIT — https://github.com/alfg/mp4-rust                                         |
| matroska                     | Matroska/WebM container metadata                               | MIT — https://github.com/rust-av/matroska                                      |
| trash-core                   | Finder Trash `.DS_Store` parser and test fixture               | Apache-2.0 — see tests/data/LICENSE-APACHE                                     |

Fiex adapts the parser and regression fixture from [SecurityRonin/trash-forensic](https://github.com/SecurityRonin/trash-forensic), with a Fiex-specific lookup for individual Trash entries. The fixture's Apache-2.0 text remains beside the test data at src-tauri/crates/builtins/fiex-trash/tests/data/LICENSE-APACHE.

The optional Archive, Checksum, Git, WebDAV and SMB plugins live in a separate repository and have their own dependency notices. Core dependencies include serde, serde_json, dirs, open, which, base64, mime_guess, tempfile, sysinfo, image, lofty, mp4, matroska, objc2, clsx, class-variance-authority and tailwind-merge. Development tooling includes Vite, TypeScript, Vitest, Playwright and Prettier.

## 随资源分发的许可证文本

- [Noto Sans SC — SIL Open Font License 1.1](/licenses/noto-sans-sc.txt)
- [PDF.js — Apache License 2.0](/licenses/pdfjs.txt)
- JetBrainsMono Nerd Font Mono 的字体许可证随应用资源保存在 public/fonts/OFL.txt。
- Fiex 的 GPL-3.0-or-later 正文见仓库根目录 LICENSE；Arapuca 和其他 vendored 依赖保留其上游许可证文件。
