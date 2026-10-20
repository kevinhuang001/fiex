# Fiex

基于 Tauri + React + shadcn/ui 的三平台资源管理器，参考 Pantodo 的磨砂侧栏与简洁圆角界面。

[下载安装包](https://github.com/kevinhuang001/fiex/releases/latest)

- 列表、图标、分栏、画廊四种视图；按类型、扩展名、修改时间或大小分组。
- 内置 ripgrep `ignore` 引擎进行多线程文件名查找，流式返回结果，支持取消、隐藏文件和 .gitignore 过滤。
- 标签页、收藏夹、设备列表、路径导航与自动刷新。
- 创建、重命名、复制、剪切、粘贴、副本、移入废纸篓及会话内撤销；同名项目保留两者。
- 拖放文件，空格预览图片、音视频、UTF-8 文本/代码和 PDF（取决于系统 WebView）。
- 系统原生文件图标和应用 Logo。
- 内置 ZIP 压缩和 ZIP / 7z / TAR / GZ / BZ2 / XZ 解压，无需额外安装工具。
- Open in Terminal 按钮，自动检测已安装终端，可选择默认终端。
- 浅色、深色、跟随系统外观。

当前版本限制、构建方法与快捷键参见 [README](README.md)。跨磁盘移动暂需先复制，再核对并删除原件；恢复删除项目请在系统废纸篓中操作。RAR、加密压缩包、Finder 标签和整盘索引暂不包含。Linux 背景透光由桌面合成器决定。
