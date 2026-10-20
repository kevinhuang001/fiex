# Fiex

基于 Tauri + React + shadcn/ui 的三平台资源管理器，参考 Pantodo 的磨砂侧栏与简洁圆角界面。

[下载安装包](https://github.com/kevinhuang001/fiex/releases/latest)

- 列表、图标、分栏、画廊四种视图；图标视图采用类似 Finder 的固定网格、两行文件名与对齐排列；可按类型、扩展名、修改时间或大小分组。
- 文件图标来自 MIT 许可的 [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme)，覆盖 1300+ 扩展名与 2100+ 文件名，按语言和类型着色；应用仍显示系统原生 Logo。
- 内置 ripgrep `ignore` 引擎进行多线程文件名查找，流式返回结果，支持取消、隐藏文件和 .gitignore 过滤。
- 标签页、收藏夹、设备列表、路径导航与自动刷新。
- 创建、重命名、复制、剪切、粘贴、副本、移入废纸篓及会话内撤销；重命名直接在文件项上完成，新建文件夹立即创建并选中名称；同名项目保留两者。
- 拖放文件，空格预览图片、音视频、UTF-8 文本/代码和 PDF（取决于系统 WebView）。
- 应用使用系统原生 Logo；普通文件按类型和语言显示彩色图标。
- 内置 ZIP 压缩和 ZIP / 7z / TAR / GZ / BZ2 / XZ 解压，无需额外安装工具。
- Open in Terminal 按钮，自动检测已安装终端，可选择默认终端。
- 浅色、深色、跟随系统外观。
- 设置中可编辑快捷键，支持 macOS / Windows / Linux 整套预设切换与单项重录。

当前版本限制、构建方法与快捷键参见 [README](README.md)。原生压缩库随应用静态链接（包括 liblzma），发布前会审计并拒绝依赖构建机 `/opt/homebrew` 等路径的安装包。跨磁盘移动暂需先复制，再核对并删除原件；恢复删除项目请在系统废纸篓中操作。RAR、加密压缩包、Finder 标签和整盘索引暂不包含。Linux 背景透光由桌面合成器决定。
