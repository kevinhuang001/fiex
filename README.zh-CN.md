# Fiex

基于 Tauri + React + shadcn/ui 的三平台资源管理器，参考 Pantodo 的磨砂侧栏与简洁圆角界面。

[下载安装包](https://github.com/kevinhuang001/fiex/releases/latest)

- 地址栏可点击编辑当前路径，地址会自动全选，直接复制即可。
- 列表、图标、分栏、画廊四种视图；图标视图采用类似 Finder 的固定网格、两行文件名与对齐排列；可按类型、扩展名、修改时间或大小分组。
- 文件图标来自 MIT 许可的 [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme)，覆盖 1300+ 扩展名与 2100+ 文件名，按语言和类型着色；应用仍显示系统原生 Logo。
- 内置 ripgrep `ignore` 引擎进行多线程文件名查找，流式返回结果，支持取消、隐藏文件和 .gitignore 过滤；进入文件夹后搜索框自动聚焦，应用包（.app/.framework）只作为结果出现，不会深入其内部搜索。
- 标签页、收藏夹、设备列表、路径导航与自动刷新。
- 创建、重命名、复制、剪切、粘贴、副本、移入废纸篓及会话内撤销；重命名直接在文件项上完成，新建文件夹立即创建并选中名称；同名项目保留两者。
- 拖放文件或文件夹到文件夹默认移动，按住 Option/Alt（或 Ctrl）复制；把文件夹拖到“收藏”区域即可加入书签。
- 空格预览图片、音视频、UTF-8 文本/代码和 PDF（取决于系统 WebView）。
- 设置采用类似 macOS 系统设置的侧栏 + 分组卡片布局，可搜索分类。
- 三个平台都提供“应用程序”入口：macOS 直接浏览 /Applications，Windows / Linux 打开系统应用列表。
- 支持 WebDAV（HTTP/HTTPS）和 SMB 虚拟设备：连接挂载到 `~/.fiex/drives`，目录按需同步，文件首次打开时下载，已下载文件可离线使用，并可右键整目录离线下载。设备旁的按钮可弹出磁盘或关闭连接。
- 若系统安装了 Git，状态栏显示当前分支和更改数量，文件行显示 Git 状态，简介中包含 Git 信息。
- 搜索使用 `~/.fiex/index` 下的 Tantivy 持久索引，浏览时后台建立并自动监听更新；搜索使用“显示忽略”规则。
- 支持 macOS Finder 标签：列表、图标和分栏视图显示彩色圆点，右键可设置预设颜色或自定义标签名；侧边栏列出标准标签，点击即可筛选当前文件夹。
- 支持从工具栏或右键菜单用 VS Code 打开文件/文件夹；终端检测新增 Tabby。
- 分组标题加粗并使用正文色；字体大小设置会同时缩放文件名和图标视图图标。
- 设置中可分别编辑显示、压缩、复制/移动三组忽略规则；预设覆盖各平台系统文件以及 node_modules、target、`__pycache__`、.gradle、Pods、vendor 等主流构建目录。
- 右键压缩支持 ZIP、7Z、TAR、TAR.GZ、TAR.BZ2、TAR.XZ 二级菜单。
- macOS / Linux 的“废纸篓”直接在 Fiex 内打开；macOS 受系统保护时，会引导开启“完全磁盘访问权限”或改用 Finder。
- 应用 Logo 按屏幕像素比渲染，图标视图下保持清晰。
- macOS 只显示一个 Macintosh HD，不再重复显示 /System/Volumes/Data。
- 应用使用系统原生 Logo；普通文件按类型和语言显示彩色图标。
- 内置 ZIP 压缩和 ZIP / 7z / TAR / GZ / BZ2 / XZ 解压，无需额外安装工具。
- Open in Terminal 按钮，自动检测已安装终端，可选择默认终端。
- 浅色、深色、跟随系统外观。
- 设置中可编辑快捷键，支持 macOS / Windows / Linux 整套预设切换与单项重录。

当前版本限制、构建方法与快捷键参见 [README](README.md)。原生压缩库随应用静态链接（包括 liblzma），发布前会审计并拒绝依赖构建机 `/opt/homebrew` 等路径的安装包。跨磁盘移动暂需先复制，再核对并删除原件；恢复删除项目请在系统废纸篓中操作。RAR、加密压缩包、操作系统级网络挂载和整盘索引暂不包含；WebDAV/SMB 是 `~/.fiex` 下的只读虚拟镜像。Linux 背景透光由桌面合成器决定。
