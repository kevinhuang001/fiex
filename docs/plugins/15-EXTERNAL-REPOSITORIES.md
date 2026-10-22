# 15 · 独立仓库、CI 所有权及插件安装

- [Fiex Core](https://github.com/kevinhuang001/fiex)：主应用、基本文件管理、插件宿主/授权/安装/UI、通用模拟插件 IPC 测试。
- [Fiex SDK](https://github.com/kevinhuang001/fiex-sdk)：原生 Rust 和 iframe UI SDK、协议与测试。
- [Fiex Official Plugins](https://github.com/kevinhuang001/fiex-official-plugins)：Git、WebDAV、SMB、Archive、Checksum 独立插件源码、构建、集成测试、插件包 Release。

Core **只依赖 `sdk/` git 子模块**；不包含 `plugins/` 子模块、不构建、不预装也不发布官方插件。官方插件仓库单独依赖其固定版本的 `sdk/` 子模块。双方更新 SDK SHA 时应明确审查 ABI / API 变更。

本地开发：

```sh
git clone https://github.com/kevinhuang001/fiex.git
cd fiex
git submodule update --init sdk
```

**CI 归属**：Core CI 只做前端/桌面应用构建、通用本地文件操作、权限/安装器和**模拟子进程的 IPC**，禁止在 Core CI 中启动 WebDAV/SMB fixture；插件仓库单独在 Linux/macOS/Windows 构建并打包，通过真实 WebDAV/Samba 服务验证校验、列目录、游标、路径限制、预览和文件下载；再调用真实的 Fiex Core `PluginHost` 验证五个插件的安装批准、进程握手、启停与 IPC。SDK 自有单独 CI。只有插件仓库的 `plugins-v*` Tag 有资格在**所有插件测试通过后**发布 `.fiexplugin`。

**运行时现状**：Core 的 WebDAV/SMB 目录获取、连接验证、下载现在走经授权的 Native 插件 RPC，目录显示不制造空占位文件；但 Core 仍管理旧连接和缓存，也仍包含旧 Git 后端。暂存下载仍向插件传本地绝对路径，必须迁移到受控二进制流才可视为最终解耦。

**授权/隔离界线**：官方和第三方插件同权，每次安装需要用户同意；作者或来源**不自动放行权限**。当前权限主要在宿主 API 层执行，插件 OS 进程尚无强制沙箱，不能将宿主授权称为完全文件/网络/命令隔离。用户只能从 GitHub Releases 选择包，不直接运行源码。

最新实现约束和未完成阻断项见 [实现进度](12-IMPLEMENTATION-STATUS.md)。
